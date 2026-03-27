---
sidebar_position: 8
title: Agent Environment
---

# Environment Variables

Each session maintains its own set of environment variables, accessible to WASM programs via WASI `environ_get` / `environ_sizes_get`.

## Set Environment Variables

```
POST /api/v1/sessions/:id/env
```

**Request body:**
```json
{
  "DATABASE_URL": "postgres://localhost/mydb",
  "DEBUG": "true"
}
```

Variables are merged — existing keys are updated, new keys are added.

**Response** (200):
```json
{
  "message": "Set 2 environment variable(s)"
}
```

## Get Environment Variables

```
GET /api/v1/sessions/:id/env
```

**Response** (200):
```json
{
  "env": {
    "DATABASE_URL": "postgres://localhost/mydb",
    "DEBUG": "true"
  }
}
```

## Per-Execution Environment

You can also set environment variables in the exec request. These are applied before execution and persist for subsequent calls:

```json
POST /api/v1/sessions/:id/exec
{
  "wasm_path": "app.wasm",
  "env": {
    "MODE": "production"
  }
}
```
