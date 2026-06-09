---
sidebar_position: 5
title: Agent Sessions
---

# Session Management

Sessions are isolated WASM sandboxes. Each session has its own filesystem, environment, and output buffers.

## Create a Session

```
POST /api/v1/sessions
```

**Response** (200):
```json
{
  "session_id": "a1b2c3d4e5f6...",
  "created_at": "2025-01-15T10:30:00Z"
}
```

### Per-Session Limit Overrides

The body is optional. To override the server's default [resource limits](../agent.md#starting-the-server) for this session only, pass a `limits` object — any omitted field keeps the server default, and `0` disables that cap:

```json
{
  "limits": {
    "max_memory_mb": 128,
    "max_fuel": 5000000,
    "max_output_mb": 5,
    "max_file_size_mb": 10,
    "max_disk_mb": 50
  }
}
```

Body size and exec concurrency are server-wide and cannot be overridden per session.

## Get Session Status

```
GET /api/v1/sessions/:id
```

**Response** (200):
```json
{
  "session_id": "a1b2c3d4e5f6...",
  "state": "active",
  "created_at_elapsed_ms": 5000,
  "last_accessed_elapsed_ms": 1200,
  "timeout_secs": 300
}
```

## Destroy a Session

```
DELETE /api/v1/sessions/:id
```

Destroys the session and cleans up its filesystem.

**Response** (200):
```json
{
  "message": "Session a1b2c3d4e5f6... destroyed"
}
```

## Session Lifecycle

1. **Created** — `POST /sessions` allocates an isolated temp directory and WASI environment
2. **Active** — every API call to the session resets the idle timer
3. **Expired** — after `timeout` seconds of inactivity, the session is marked expired
4. **Cleaned up** — a background thread periodically removes expired sessions and their files

## Error Responses

| Status | When |
|--------|------|
| 401 | Auth enabled (`--auth`) but the API key is missing, malformed, or unknown |
| 404 | Session not found — or owned by another tenant (see below) |
| 410 | Session expired |
| 413 | Request body exceeded the server's `--max-body` limit |
| 429 | Maximum concurrent sessions reached |

When the server is started with [`--auth`](../agent.md#authentication), each session is owned by the tenant that created it. A request for a session owned by a different tenant returns **404** — identical to a nonexistent session — so existence isn't leaked across tenants.
