---
sidebar_position: 9
title: Agent Observability
---

# Observability

The agent server exposes runtime metrics for scraping and emits a structured access log for every request, so you can answer "is it healthy, how busy is it, what's failing" without attaching a debugger.

## Metrics

```
GET /api/v1/metrics
```

Returns server metrics in **Prometheus text exposition format** by default, or as JSON with `?format=json`.

```sh
# Prometheus (default) — point a Prometheus/Grafana scraper here
curl http://localhost:8430/api/v1/metrics

# JSON — convenient for ad-hoc inspection or a custom dashboard
curl "http://localhost:8430/api/v1/metrics?format=json"
```

**Prometheus response** (`Content-Type: text/plain; version=0.0.4`):

```
# HELP wasmrun_agent_exec_total Total code executions by terminal result.
# TYPE wasmrun_agent_exec_total counter
wasmrun_agent_exec_total{result="success"} 12
wasmrun_agent_exec_total{result="error"} 1
wasmrun_agent_exec_total{result="timeout"} 0
# HELP wasmrun_agent_exec_duration_ms_sum Sum of execution wall-clock durations in milliseconds.
# TYPE wasmrun_agent_exec_duration_ms_sum counter
wasmrun_agent_exec_duration_ms_sum 4200
...
# HELP wasmrun_agent_sessions_active Currently active (non-expired) sessions.
# TYPE wasmrun_agent_sessions_active gauge
wasmrun_agent_sessions_active 3
```

**JSON response** (`?format=json`):

```json
{
  "exec_total": { "success": 12, "error": 1, "timeout": 0 },
  "exec_duration_ms_sum": 4200,
  "exec_duration_ms_count": 13,
  "output_truncated_total": 0,
  "sessions_created_total": 20,
  "exec_rejected_total": { "concurrency": 0, "payload": 0, "unauthorized": 0, "rate": 0 },
  "sessions_active": 3,
  "sessions_total": 3,
  "exec_in_flight": 1,
  "sessions_disk_bytes": 16384
}
```

### Metrics reference

| Metric | Type | Meaning |
|--------|------|---------|
| `wasmrun_agent_exec_total{result}` | counter | Executions by terminal result: `success` (ran to completion), `error` (failed to run), `timeout` |
| `wasmrun_agent_exec_duration_ms_sum` | counter | Sum of execution wall-clock durations (ms) |
| `wasmrun_agent_exec_duration_ms_count` | counter | Number of executions in the sum — pair with `_sum` for the average |
| `wasmrun_agent_output_truncated_total` | counter | Executions whose captured output hit the `--max-output` cap |
| `wasmrun_agent_sessions_created_total` | counter | Sessions created since startup |
| `wasmrun_agent_exec_rejected_total{reason}` | counter | Requests rejected before doing work, by `reason`: `concurrency` (429), `payload` (413), `unauthorized` (401), `rate` (429, per-tenant rate limit) |
| `wasmrun_agent_sessions_active` | gauge | Active (non-expired) sessions right now |
| `wasmrun_agent_sessions_total` | gauge | Sessions tracked, including expired-but-not-yet-cleaned |
| `wasmrun_agent_exec_in_flight` | gauge | Exec workers currently running |
| `wasmrun_agent_sessions_disk_bytes` | gauge | Total on-disk footprint across active sessions (bytes) |

Average execution duration is `exec_duration_ms_sum / exec_duration_ms_count`.

### Authentication

When [`--auth`](../agent.md#authentication) is enabled, `/metrics` requires a valid API key like every other endpoint, and the scrape is limited to **global aggregates** — no per-tenant or per-session data — so one tenant cannot infer another's activity. A missing or invalid key returns **401** (and increments `exec_rejected_total{reason="unauthorized"}`).

### Per-session breakdown (open mode only)

In open mode (no `--auth`), the JSON format adds a `sessions` array with each active session's disk footprint and configured memory cap:

```json
{
  "...": "...",
  "sessions": [
    { "id": "a1b2c3...", "disk_bytes": 8192, "memory_cap_pages": 4096 }
  ]
}
```

`memory_cap_pages` is in WASM pages (64 KiB each); `null` means unlimited. This breakdown is **not** emitted in the Prometheus format (session ids would be unbounded-cardinality labels) and is withheld entirely when auth is enabled — leaving only the aggregate `sessions_disk_bytes` gauge.

## Access Log

The server writes one structured `key=value` line to **stderr** for every request, always on:

```
ts=2026-06-13T11:18:55.354+00:00 id=d5aafbe6c56c32ec method=POST path=/api/v1/sessions status=200 dur_ms=1 tenant=-
```

| Field | Meaning |
|-------|---------|
| `ts` | RFC 3339 timestamp |
| `id` | Request id — also returned as the `X-Request-Id` response header |
| `method` | HTTP method |
| `path` | Request path |
| `status` | HTTP status code |
| `dur_ms` | Handling duration in milliseconds |
| `tenant` | Authenticated tenant id, or `-` in open mode / before auth resolves |

Running with [`-v, --verbose`](../agent.md#starting-the-server) adds a request-received line (`→ METHOD url (id=...)`) ahead of each response line; the structured access line is emitted either way.

### Request correlation

Every response carries an `X-Request-Id` header matching the `id` in the access log, so a client can tie a response back to its server-side log line:

```sh
curl -i http://localhost:8430/api/v1/metrics | grep -i x-request-id
# X-Request-Id: 37dffe5b315dcd30
```
