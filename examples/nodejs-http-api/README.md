# HTTP API Example for wasmrun

A Node.js REST API on the built-in `http` module, with no dependencies. It runs in all three places wasmrun can run JavaScript: the browser VM (`wasmrun os`), a session in agent mode (`POST /sessions/:id/serve`), and plain `node`. The same `index.js`, unchanged, in each.

It manages users and todos in memory and serves a small test page from `public/`.

## Why no Express

The sandbox has no `node_modules`. OS mode ships the project's own files into the VM and nothing else, so `require('express')` has nothing to find. Everything the program uses has to come from the runtime: `http`, `fs`, `path`, `URL`, `Buffer`. Agent mode can vendor npm packages for `POST /exec`, but a server that works everywhere is one that needs none.

## Running in OS mode

```sh
wasmrun os examples/nodejs-http-api
```

Open `http://localhost:8420`, switch to the Console panel and press **Run**. The console prints the port the host bound for the program, then the program's own startup line:

```
Port bound for the program: http://127.0.0.1:3000
HTTP API listening on 127.0.0.1:3000
```

The API is now reachable from the host, and from any tab in the browser:

```sh
curl http://127.0.0.1:3000/health
open http://127.0.0.1:3000/
```

Each request is logged to the Console panel as it arrives. **Stop** ends the program and releases the port. The port comes from the `bind_ports` range in the project's network policy (`3000-9999` by default); see [Serving a Port](https://wasmrun.readthedocs.io/en/latest/docs/os/port-forwarding) for how the host hands it in.

## Running in agent mode

```sh
wasmrun agent
```

```sh
SESSION=$(curl -s -X POST http://localhost:8430/api/v1/sessions | jq -r .session_id)

# Upload the two files the server needs and start it
jq -n --rawfile index index.js --rawfile page public/index.html \
  '{files: {"index.js": $index, "public/index.html": $page}, entry: "index.js", language: "javascript"}' \
  | curl -s -X POST "http://localhost:8430/api/v1/sessions/$SESSION/serve" \
      -H 'Content-Type: application/json' -d @-
# {"server_id":"srv_...","addr":"127.0.0.1:53412","port":53412}

curl http://127.0.0.1:53412/health
curl -X DELETE "http://localhost:8430/api/v1/sessions/$SESSION/serve"
```

## Running with Node

```sh
node index.js
# HTTP API listening on :::3000
```

`PORT` picks the port here. Under wasmrun the host has already bound one, so `listen()` takes what it is given and the number is ignored.

## API endpoints

### General
- `GET /`: the test page from `public/`
- `GET /api`: welcome message and endpoint list
- `GET /health`: health check, with `environment` set to `wasmrun` when running in a sandbox

### Users
- `GET /api/users`: all users, `?role=admin|user` to filter
- `GET /api/users/:id`: one user
- `POST /api/users`: create a user, `{ "name", "email", "role"? }`

### Todos
- `GET /api/todos`: all todos, `?userId=1` and `?completed=true|false` to filter
- `GET /api/todos/:id`: one todo
- `POST /api/todos`: create a todo, `{ "title", "userId" }`
- `PUT /api/todos/:id`: update a todo, `{ "title"?, "completed"? }`
- `DELETE /api/todos/:id`: delete a todo

### Statistics
- `GET /api/stats`: counts, plus what the runtime reports about itself

## Trying the endpoints

```sh
BASE=http://127.0.0.1:3000

curl $BASE/health
curl $BASE/api/users
curl "$BASE/api/users?role=admin"

curl -X POST $BASE/api/users \
  -H "Content-Type: application/json" \
  -d '{"name":"David","email":"david@example.com","role":"user"}'

curl -X POST $BASE/api/todos \
  -H "Content-Type: application/json" \
  -d '{"title":"Test wasmrun OS mode","userId":1}'

curl -X PUT $BASE/api/todos/1 \
  -H "Content-Type: application/json" \
  -d '{"completed":true}'

curl -X DELETE $BASE/api/todos/2

curl $BASE/api/stats
```

## What the runtime provides

The wasmhub `nodejs` runtime is not Node. It covers what this example needs (`http` servers, `fs`, `path`, `URL`, `Buffer`, `process.env`) and leaves out some things a Node program might reach for. `process.uptime()` and `process.memoryUsage()` are absent, which is why the uptime here is computed from `Date.now()`. Outbound requests (`http.request`, `fetch`, `net.connect`) are not available in the sandbox.
