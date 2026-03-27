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
| 404 | Session not found |
| 410 | Session expired |
| 429 | Maximum concurrent sessions reached |
