// A REST API on Node's built-in http module, with no dependencies.
//
// The sandbox has no node_modules: OS mode ships the project files and nothing
// else, so a require('express') has nothing to find. Everything here comes
// from the runtime itself. The same file runs under `wasmrun os`, under agent
// mode's POST /sessions/:id/serve, and under plain `node index.js`.
const http = require('http');
const fs = require('fs');
const path = require('path');

// The port is advisory. Under wasmrun the host binds the socket and hands it
// in, so listen() picks that up and this number is only used by plain node.
const PORT = process.env.PORT || 3000;
const startedAt = Date.now();

const users = [
  { id: 1, name: 'Alice', email: 'alice@example.com', role: 'admin' },
  { id: 2, name: 'Bob', email: 'bob@example.com', role: 'user' },
  { id: 3, name: 'Charlie', email: 'charlie@example.com', role: 'user' },
];

const todos = [
  { id: 1, title: 'Learn WebAssembly', completed: false, userId: 1 },
  { id: 2, title: 'Build wasmrun project', completed: true, userId: 1 },
  { id: 3, title: 'Test OS mode', completed: false, userId: 2 },
];

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css',
  '.js': 'text/javascript',
  '.json': 'application/json',
  '.png': 'image/png',
  '.svg': 'image/svg+xml',
};

function send(res, status, body, headers = {}) {
  const payload = typeof body === 'string' || Buffer.isBuffer(body) ? body : JSON.stringify(body, null, 2);
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(payload),
    'Access-Control-Allow-Origin': '*',
    ...headers,
  });
  res.end(payload);
}

function readJson(req) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    req.on('data', chunk => chunks.push(chunk));
    req.on('end', () => {
      const raw = Buffer.concat(chunks).toString('utf8');
      if (!raw) return resolve({});
      try {
        resolve(JSON.parse(raw));
      } catch (err) {
        reject(new Error('Body is not valid JSON'));
      }
    });
    req.on('error', reject);
  });
}

function nextId(items) {
  return items.reduce((max, item) => Math.max(max, item.id), 0) + 1;
}

// Files under public/ are served as-is, with the path kept inside that
// directory. Anything else falls through to the API routes.
function serveStatic(res, pathname) {
  const root = path.join(__dirname, 'public');
  const target = path.normalize(path.join(root, pathname === '/' ? 'index.html' : pathname));
  if (!target.startsWith(root)) return false;
  let data;
  try {
    data = fs.readFileSync(target);
  } catch (err) {
    return false;
  }
  const type = MIME[path.extname(target)] || 'application/octet-stream';
  send(res, 200, data, { 'Content-Type': type });
  return true;
}

const routes = [
  {
    method: 'GET',
    pattern: /^\/health$/,
    handler: (req, res) =>
      send(res, 200, {
        status: 'ok',
        timestamp: new Date().toISOString(),
        uptime: (Date.now() - startedAt) / 1000,
        environment: process.env.WASMHUB_LISTEN_FD ? 'wasmrun' : 'node',
        version: '1.0.0',
      }),
  },
  {
    method: 'GET',
    pattern: /^\/api$/,
    handler: (req, res) =>
      send(res, 200, {
        message: 'Welcome to the wasmrun HTTP API example',
        endpoints: {
          'GET /': 'This page, served from public/',
          'GET /health': 'Health check',
          'GET /api/users': 'Get all users, ?role=admin|user to filter',
          'GET /api/users/:id': 'Get user by ID',
          'POST /api/users': 'Create a user',
          'GET /api/todos': 'Get all todos, ?userId= and ?completed= to filter',
          'GET /api/todos/:id': 'Get todo by ID',
          'POST /api/todos': 'Create a todo',
          'PUT /api/todos/:id': 'Update a todo',
          'DELETE /api/todos/:id': 'Delete a todo',
          'GET /api/stats': 'Server and data statistics',
        },
      }),
  },
  {
    method: 'GET',
    pattern: /^\/api\/users$/,
    handler: (req, res, params, query) => {
      const role = query.get('role');
      const result = role ? users.filter(u => u.role === role) : users;
      send(res, 200, { users: result, count: result.length });
    },
  },
  {
    method: 'GET',
    pattern: /^\/api\/users\/(\d+)$/,
    handler: (req, res, [id]) => {
      const user = users.find(u => u.id === Number(id));
      if (!user) return send(res, 404, { error: 'User not found' });
      send(res, 200, user);
    },
  },
  {
    method: 'POST',
    pattern: /^\/api\/users$/,
    handler: async (req, res) => {
      const { name, email, role = 'user' } = await readJson(req);
      if (!name || !email) return send(res, 400, { error: 'Name and email are required' });
      const user = { id: nextId(users), name, email, role };
      users.push(user);
      send(res, 201, user);
    },
  },
  {
    method: 'GET',
    pattern: /^\/api\/todos$/,
    handler: (req, res, params, query) => {
      let result = todos;
      const userId = query.get('userId');
      const completed = query.get('completed');
      if (userId) result = result.filter(t => t.userId === Number(userId));
      if (completed !== null) result = result.filter(t => t.completed === (completed === 'true'));
      send(res, 200, { todos: result, count: result.length });
    },
  },
  {
    method: 'GET',
    pattern: /^\/api\/todos\/(\d+)$/,
    handler: (req, res, [id]) => {
      const todo = todos.find(t => t.id === Number(id));
      if (!todo) return send(res, 404, { error: 'Todo not found' });
      send(res, 200, todo);
    },
  },
  {
    method: 'POST',
    pattern: /^\/api\/todos$/,
    handler: async (req, res) => {
      const { title, userId } = await readJson(req);
      if (!title || !userId) return send(res, 400, { error: 'Title and userId are required' });
      const todo = { id: nextId(todos), title, completed: false, userId: Number(userId) };
      todos.push(todo);
      send(res, 201, todo);
    },
  },
  {
    method: 'PUT',
    pattern: /^\/api\/todos\/(\d+)$/,
    handler: async (req, res, [id]) => {
      const todo = todos.find(t => t.id === Number(id));
      if (!todo) return send(res, 404, { error: 'Todo not found' });
      const { title, completed } = await readJson(req);
      if (title !== undefined) todo.title = title;
      if (completed !== undefined) todo.completed = Boolean(completed);
      send(res, 200, todo);
    },
  },
  {
    method: 'DELETE',
    pattern: /^\/api\/todos\/(\d+)$/,
    handler: (req, res, [id]) => {
      const index = todos.findIndex(t => t.id === Number(id));
      if (index === -1) return send(res, 404, { error: 'Todo not found' });
      send(res, 200, todos.splice(index, 1)[0]);
    },
  },
  {
    method: 'GET',
    pattern: /^\/api\/stats$/,
    handler: (req, res) =>
      send(res, 200, {
        totalUsers: users.length,
        totalTodos: todos.length,
        completedTodos: todos.filter(t => t.completed).length,
        pendingTodos: todos.filter(t => !t.completed).length,
        usersByRole: {
          admin: users.filter(u => u.role === 'admin').length,
          user: users.filter(u => u.role === 'user').length,
        },
        serverInfo: {
          nodeVersion: process.version,
          platform: process.platform,
          uptime: (Date.now() - startedAt) / 1000,
          pid: process.pid,
        },
      }),
  },
];

const server = http.createServer(async (req, res) => {
  const url = new URL(req.url, 'http://localhost');
  console.log(`${req.method} ${url.pathname}${url.search}`);

  if (req.method === 'OPTIONS') {
    return send(res, 204, '', {
      'Access-Control-Allow-Methods': 'GET, POST, PUT, DELETE, OPTIONS',
      'Access-Control-Allow-Headers': 'Content-Type',
    });
  }

  if (req.method === 'GET' && !url.pathname.startsWith('/api') && url.pathname !== '/health') {
    if (serveStatic(res, url.pathname)) return;
  }

  for (const route of routes) {
    if (route.method !== req.method) continue;
    const match = url.pathname.match(route.pattern);
    if (!match) continue;
    try {
      await route.handler(req, res, match.slice(1), url.searchParams);
    } catch (err) {
      send(res, 400, { error: err.message });
    }
    return;
  }

  send(res, 404, { error: 'Not found', path: url.pathname });
});

server.listen(PORT, () => {
  const address = server.address();
  const where = typeof address === 'string' ? address : `${address.address}:${address.port}`;
  console.log(`HTTP API listening on ${where}`);
});
