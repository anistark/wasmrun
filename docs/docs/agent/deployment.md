---
sidebar_position: 2
title: Deployment
---

# Production Deployment

Concrete recipes for running `wasmrun agent` as a service that other people depend on. The reasoning behind each constraint lives in the [overview](./index.md); this page is the configuration.

Everything here assumes the supported shape:

```
  clients ──TLS──> reverse proxy ──HTTP──> wasmrun agent (loopback)
                        │                        │
                   terminates TLS          --auth on regardless
                   /health, /ready         no TLS in-process
```

Three properties drive every recommendation below:

- **The exec endpoint runs arbitrary code.** Reaching it is equivalent to running code on the host, so the bind address and the auth file matter more than anything else on this page.
- **TLS is not terminated in-process.** A proxy is not optional for anything beyond loopback; see [Network exposure and TLS](./index.md#network-exposure-and-tls).
- **Sessions are pinned to one process and do not survive a restart.** That rules out round-robin load balancing and rolling restarts that expect to preserve work; see [Restarts and shutdown](./index.md#restarts-and-shutdown).

## Before you start

A checklist to run through once. Each item is expanded below.

| | Why it bites |
|---|---|
| A writable `$HOME` for the service user | The npm and runtime caches live in `~/.wasmrun`. Without one, every restart re-downloads and JavaScript execution fails outright |
| Outbound HTTPS from the host | Dependency vendoring and the language runtimes are fetched by the server. The sandbox itself has no network |
| `--auth` with per-tenant keys | The only thing between a reachable port and arbitrary code execution |
| A stop timeout larger than `--shutdown-timeout` | Otherwise the supervisor `SIGKILL`s mid-drain and leaks session directories |
| Proxy body size, read timeout and buffering | The three proxy defaults that silently break large uploads, long executions and streaming |
| `--max-concurrent-exec` sized to the host | The default of 100 is generous for a laptop and wrong for a small container |

## Host requirements

**A home directory.** Both caches are keyed off the service user's home:

| Path | Holds | If missing |
|---|---|---|
| `~/.wasmrun/runtimes/` | wasmhub language runtimes | JavaScript and TypeScript execution fails; the server cannot resolve a home directory to cache into |
| `~/.wasmrun/npm/` | Downloaded packages and their lowered forms | Vendoring still works, but every install re-downloads |

Systemd units that set `DynamicUser=yes`, and containers that run as a uid with no home, both hit this. Point `HOME` somewhere writable and persistent, as the unit and the Compose file below do.

**Outbound HTTPS.** The *server* needs egress even though the *sandbox* has none:

| Host | For | Configurable |
|---|---|---|
| `registry.npmjs.org` | npm metadata and tarballs | `--npm-registry` |
| `github.com`, redirecting to `release-assets.githubusercontent.com` | wasmhub language runtimes, fetched once per pinned release | `WASMRUN_WASMHUB_BASE_URL` |

Root certificates are compiled into the binary (rustls with bundled webpki roots), so a minimal image needs no system CA bundle.

**Disk.** Session working directories go in the temp directory, bounded per session by `--max-disk`. The shared caches go in `$HOME/.wasmrun`, bounded by `--max-cache-size` for npm plus roughly one artifact per language runtime. Budget `--max-sessions * --max-disk` for temp, and `--max-cache-size` plus a few hundred MB for the home directory.

## systemd

```ini
# /etc/systemd/system/wasmrun-agent.service
[Unit]
Description=wasmrun agent sandbox API
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=wasmrun
Group=wasmrun

# The caches live under $HOME/.wasmrun. StateDirectory creates and owns
# /var/lib/wasmrun; HOME points the caches there so they survive a restart
# instead of being re-downloaded.
StateDirectory=wasmrun
Environment=HOME=/var/lib/wasmrun

ExecStart=/usr/local/bin/wasmrun agent \
    --port 8430 \
    --auth /etc/wasmrun/auth.toml \
    --max-sessions 50 \
    --max-concurrent-exec 8 \
    --max-cache-size 4096 \
    --shutdown-timeout 30

Restart=on-failure
RestartSec=2

# Must exceed --shutdown-timeout, or systemd SIGKILLs the drain half-finished.
TimeoutStopSec=45

NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ProtectKernelTunables=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes

[Install]
WantedBy=multi-user.target
```

Notes on the hardening block, which is where these units usually go wrong:

- `ProtectHome=yes` is safe **only because** `HOME` is redirected to `/var/lib/wasmrun`. Leave `HOME` at a real home directory and this directive hides the caches.
- `ProtectSystem=strict` makes the filesystem read-only except for the `StateDirectory`, which is all the server writes to outside the temp directory.
- `PrivateTmp=yes` gives the service its own `/tmp`. Session directories land there, and the [orphan sweep](./index.md#orphaned-session-directories) then only ever sees trees from this service.
- `TimeoutStopSec` must exceed `--shutdown-timeout`. systemd sends `SIGTERM`, which the server handles cleanly, but it escalates to `SIGKILL` at `TimeoutStopSec` regardless of how the drain is going.

Then:

```sh
install -m 600 -o wasmrun -g wasmrun auth.toml /etc/wasmrun/auth.toml
systemctl daemon-reload
systemctl enable --now wasmrun-agent
systemctl status wasmrun-agent
```

Editing `/etc/wasmrun/auth.toml` needs no restart: the file is watched and [reloaded live](./index.md#live-config-reload).

## Containers

No image is published, so build one around the release binary. The server needs no runtime dependencies beyond libc:

```dockerfile
FROM debian:bookworm-slim

# curl is only here for the container healthcheck below. Root certificates are
# compiled into the binary, so nothing else is needed at runtime.
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home --home-dir /var/lib/wasmrun --shell /usr/sbin/nologin wasmrun

# A Linux release binary, from the build context. Cross-build or download it
# from the release page before `docker build`.
COPY wasmrun /usr/local/bin/wasmrun

USER wasmrun
ENV HOME=/var/lib/wasmrun
EXPOSE 8430
ENTRYPOINT ["wasmrun", "agent"]
CMD ["--host", "0.0.0.0", "--auth", "/etc/wasmrun/auth.toml"]
```

`--host 0.0.0.0` is required inside a container: the default loopback bind is reachable only from inside the container's own network namespace, so a published port would connect to nothing. That is exactly the case the [bind refusal](./index.md#network-exposure-and-tls) is guarding, which is why the `CMD` pairs it with `--auth`. Publish the port only to the proxy, not to the world.

```yaml
# compose.yaml
services:
  agent:
    build: .
    # Only the proxy reaches it; nothing is published to the host's interfaces.
    expose: ["8430"]
    volumes:
      - ./auth.toml:/etc/wasmrun/auth.toml:ro
      - wasmrun-cache:/var/lib/wasmrun
    command: >
      --host 0.0.0.0
      --auth /etc/wasmrun/auth.toml
      --max-sessions 50
      --max-concurrent-exec 8
      --shutdown-timeout 30
    # Longer than --shutdown-timeout, or `docker stop` kills the drain.
    stop_grace_period: 45s
    healthcheck:
      # /health needs no key, so the check works with --auth on.
      test: ["CMD", "curl", "-fsS", "http://localhost:8430/health"]
      interval: 30s

volumes:
  wasmrun-cache:
```

The `wasmrun-cache` volume is the part worth keeping. Without it, every container restart re-downloads the language runtimes before the first JavaScript execution can run, and every npm install starts cold. Docker seeds a fresh named volume from the image, so it inherits the `wasmrun` ownership that `useradd --create-home` established.

`docker stop` sends `SIGTERM`, which the server [drains on](./index.md#draining). Keep `stop_grace_period` above `--shutdown-timeout`.

## Kubernetes

The session model constrains the topology more than the manifest does. A session exists on exactly one pod, and any other pod returns 404 for it, so **replicas are not interchangeable**. Either run a single replica per service, or route by session id at an ingress that can read it. `sessionAffinity: ClientIP` is a weak approximation and breaks behind a shared NAT or a client-side connection pool; do not rely on it to keep a session reachable.

The Deployment spec, with the usual `metadata` and `selector` omitted:

```yaml
spec:
  # Sessions do not migrate. Scale by adding independent pools, not replicas.
  replicas: 1
  template:
    spec:
      # Comfortably above --shutdown-timeout, plus the preStop sleep.
      terminationGracePeriodSeconds: 60
      containers:
        - name: agent
          image: your-registry/wasmrun-agent:0.22.0
          args:
            - --host=0.0.0.0
            - --auth=/etc/wasmrun/auth.toml
            - --max-sessions=50
            - --max-concurrent-exec=8
            - --shutdown-timeout=30
          ports:
            - containerPort: 8430
          env:
            - name: HOME
              value: /var/lib/wasmrun
          # Both probes answer before the auth gate, so they need no key.
          livenessProbe:
            httpGet: { path: /health, port: 8430 }
            periodSeconds: 10
            failureThreshold: 3
          readinessProbe:
            httpGet: { path: /ready, port: 8430 }
            periodSeconds: 5
          lifecycle:
            preStop:
              # Let the endpoint removal propagate before the drain starts;
              # the server does not delay its own shutdown to wait for probes.
              exec: { command: ["sleep", "10"] }
          volumeMounts:
            - { name: cache, mountPath: /var/lib/wasmrun }
            - { name: auth, mountPath: /etc/wasmrun, readOnly: true }
      volumes:
        - name: cache
          persistentVolumeClaim: { claimName: wasmrun-cache }
        - name: auth
          secret: { secretName: wasmrun-auth }
```

Three things to get right:

- **`terminationGracePeriodSeconds` > `preStop` sleep + `--shutdown-timeout`.** The grace period covers both, and Kubernetes `SIGKILL`s at the end of it.
- **`/ready` is a routing signal, not a health signal.** It returns 503 for `at_session_capacity` and `at_exec_capacity`, which mean "send new work elsewhere", not "restart me". Never point the *liveness* probe at `/ready`, or a busy pod restarts itself and destroys the sessions it was serving. The full reason table is in [Observability](./usage/observability.md#health-and-readiness).
- **The cache volume should be persistent**, for the same reason as the Compose volume. An `emptyDir` still beats nothing: it survives container restarts within the pod.

## Reverse proxy

Any proxy works. What matters is the four settings that break the agent's less ordinary traffic.

### nginx

```nginx
server {
    listen 443 ssl;
    server_name agent.example.com;

    ssl_certificate     /etc/letsencrypt/live/agent.example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/agent.example.com/privkey.pem;

    location / {
        proxy_pass http://127.0.0.1:8430;
        proxy_http_version 1.1;

        # At least --max-body (default 32 MB), or large uploads are rejected
        # by nginx with its own 413 before the agent's limit applies.
        client_max_body_size 32m;

        # Longer than the longest execution `timeout` a client may request,
        # or a slow exec is cut off as a 504 while it is still running.
        proxy_read_timeout 300s;

        # Streaming executions are Server-Sent Events. With buffering on,
        # nginx holds every event until the response completes, which is
        # exactly what streaming exists to avoid.
        proxy_buffering off;
        proxy_cache off;
    }
}
```

### Caddy

```caddy
agent.example.com {
    reverse_proxy 127.0.0.1:8430 {
        # -1 disables response buffering, for streaming executions.
        flush_interval -1
        transport http {
            read_timeout 300s
        }
    }
    request_body {
        max_size 32MB
    }
}
```

### The four settings, whatever the proxy

| Setting | Set it to | Symptom when wrong |
|---|---|---|
| Max request body | At least `--max-body` (default 32 MB) | Large file writes and multi-file projects fail with the proxy's 413, not the agent's |
| Read / response timeout | Longer than the longest per-execution `timeout` a client may ask for (the default is 30s, and clients may raise it) | A long execution returns 504 while the sandbox keeps running to completion |
| Response buffering | Off | [Streaming executions](./usage/exec.md) deliver nothing until the run finishes |
| Forwarded headers | Pass through as usual | - |

Do not have the proxy strip `Authorization`: the agent authenticates every `/api/v1/*` request itself, and a proxy that authenticates instead leaves the port open to anything that reaches it directly.

Probe endpoints are unauthenticated by design and can be exposed to a load balancer without a key. They are also reachable under the API prefix (`/api/v1/health`) if a path-based routing rule is easier.

## Sizing

The defaults suit an interactive machine. On a shared server, the two that matter are `--max-concurrent-exec` and `--max-sessions`.

| Host | `--max-concurrent-exec` | `--max-sessions` | `--max-memory` | `--max-cache-size` |
|---|---|---|---|---|
| 1 vCPU, 2 GB | `2` | `20` | `256` | `1024` |
| 2 vCPU, 4 GB | `4` | `50` | `256` | `2048` |
| 4 vCPU, 8 GB | `8` | `100` | `256` | `4096` |
| 8 vCPU, 16 GB | `16` | `200` | `512` | `8192` |

Executions are CPU-bound interpretation, so concurrency above the core count buys throughput only while some executions are blocked on I/O. Starting from roughly two per core and watching `wasmrun_agent_exec_in_flight` against latency is a better guide than any table.

`--max-memory` is a ceiling on `memory.grow`, not a reservation, so a session that never allocates costs nothing. Peak memory is bounded by concurrent executions rather than by session count: plan for `--max-concurrent-exec * --max-memory` as the worst case you are willing to survive, not for `--max-sessions * --max-memory`.

`--workers` follows `--max-concurrent-exec` automatically (the cap plus 16 spare threads for requests that execute nothing). Set it explicitly only to bound thread memory on a small host, and see [Request concurrency](./index.md#request-concurrency) for why a pool smaller than the exec cap silently becomes the real exec ceiling.

Per-tenant ceilings are the other half of sizing. A single tenant can otherwise consume the whole server's budget, and `[tenants.rate]` is what stops it; see [Per-tenant limits and rate limits](./index.md#per-tenant-limits-and-rate-limits).

## Monitoring

Scrape `/api/v1/metrics` with a tenant key (it is auth-gated, unlike the probes). Full metric list in [Observability](./usage/observability.md).

```yaml
# prometheus.yml
scrape_configs:
  - job_name: wasmrun-agent
    metrics_path: /api/v1/metrics
    authorization:
      credentials_file: /etc/prometheus/wasmrun-key
    static_configs:
      - targets: ["agent.internal:8430"]
```

Worth alerting on:

| Signal | Reads as |
|---|---|
| `wasmrun_agent_exec_rejected_total{reason="concurrency"}` rising | The exec cap is the bottleneck. Raise it if the host has headroom, or add a pool |
| `wasmrun_agent_exec_rejected_total{reason="unauthorized"}` rising | Bad or revoked keys in circulation, or someone probing the port |
| `wasmrun_agent_exec_total{result="timeout"}` as a share of all executions | Clients asking for more than the timeout allows, or runaway code |
| `wasmrun_agent_sessions_active` near `--max-sessions` | Clients are not destroying sessions; they will start seeing 503 from `/ready` |
| `wasmrun_agent_sessions_disk_bytes` growing without bound | Sessions accumulating files faster than idle expiry reclaims them |

The access log goes to **stderr**, one structured `key=value` line per request. Both journald and the container log drivers capture it without extra configuration: `journalctl -u wasmrun-agent` or `docker logs`.

## Upgrading

There is no zero-downtime upgrade, because sessions cannot move between processes. An upgrade is:

1. Stop routing new sessions to the instance (deregister it, or let `/ready` fail after the signal)
2. Send `SIGTERM` and let the drain finish
3. Replace the binary and start it

Clients should already treat a 404 on a session as "create a new one", which is what makes this survivable; see [Sessions do not survive a restart](./usage/sessions.md#sessions-do-not-survive-a-restart). If in-flight work must not be lost, drain by waiting for `wasmrun_agent_sessions_active` to fall to zero before the signal, rather than by lengthening `--shutdown-timeout`, which only bounds requests already in flight.

Cached npm packages and language runtimes carry across an upgrade untouched. A wasmrun release that bumps its pinned wasmhub version re-downloads the language runtimes once and deletes the superseded artifacts on the next sweep.
