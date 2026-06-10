---
sidebar_position: 7
title: Agent File Operations
---

# File Operations

Each session has an isolated filesystem. Files are stored in a temp directory on the host, accessible to the WASM program via WASI preopens.

## Write a File

```
POST /api/v1/sessions/:id/files
```

**Request body:**
```json
{
  "path": "src/main.py",
  "content": "print('hello')"
}
```

Parent directories are created automatically. Paths are relative to the session root.

**Response** (200):
```json
{
  "message": "Written: src/main.py"
}
```

A write is rejected with **400 Bad Request** if it would exceed the session's `--max-file-size` (per file) or `--max-disk` (total) quota, and the whole request is rejected with **413 Payload Too Large** if the request body exceeds the server's `--max-body` limit. See [resource limits](../agent.md#starting-the-server).

## Read a File

```
GET /api/v1/sessions/:id/files?path=src/main.py
```

**Response** (200):
```json
{
  "path": "src/main.py",
  "content": "print('hello')"
}
```

## List a Directory

```
GET /api/v1/sessions/:id/files?path=/&list=true
```

**Response** (200):
```json
{
  "path": "/",
  "entries": [
    { "name": "src", "is_dir": true, "size": 0 },
    { "name": "hello.wasm", "is_dir": false, "size": 1024 }
  ]
}
```

## Delete a File or Directory

```
DELETE /api/v1/sessions/:id/files?path=src/main.py
```

Deletes files or directories (recursive for directories).

**Response** (200):
```json
{
  "message": "Deleted: src/main.py"
}
```

## Path Safety

- All paths are relative to the session root
- Leading `/` is stripped (treated as session root)
- Path traversal (`../`) is rejected with 400 Bad Request
- Files are only accessible within the session's isolated directory

## Authentication & Tenant Scoping

When the server runs with [`--auth`](../agent.md#authentication), every file request must carry a valid `Authorization: Bearer <key>` header — a missing, malformed, or unknown key returns **401 Unauthorized**. File operations are scoped to the calling tenant's own sessions; targeting a session owned by another tenant returns **404 Not Found**, the same as a nonexistent session.
