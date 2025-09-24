const express = require('express');
const cors = require('cors');

const app = express();
const PORT = process.env.PORT || 3000;

// Middleware
app.use(cors());
app.use(express.json());
app.use(express.static('public'));

// Sample data
let users = [
  { id: 1, name: 'Alice', email: 'alice@example.com', role: 'admin' },
  { id: 2, name: 'Bob', email: 'bob@example.com', role: 'user' },
  { id: 3, name: 'Charlie', email: 'charlie@example.com', role: 'user' }
];

let todos = [
  { id: 1, title: 'Learn WebAssembly', completed: false, userId: 1 },
  { id: 2, title: 'Build wasmrun project', completed: true, userId: 1 },
  { id: 3, title: 'Test OS mode', completed: false, userId: 2 }
];

// Health check endpoint
app.get('/health', (req, res) => {
  res.json({
    status: 'ok',
    timestamp: new Date().toISOString(),
    uptime: process.uptime(),
    environment: 'wasmrun-os-mode',
    version: '1.0.0'
  });
});

// Root endpoint
app.get('/', (req, res) => {
  res.json({
    message: 'Welcome to wasmrun Express API Example! 🚀',
    description: 'This API is running in wasmrun OS mode',
    endpoints: {
      'GET /': 'This welcome message',
      'GET /health': 'Health check',
      'GET /api/users': 'Get all users',
      'GET /api/users/:id': 'Get user by ID',
      'POST /api/users': 'Create new user',
      'GET /api/todos': 'Get all todos',
      'GET /api/todos/:id': 'Get todo by ID',
      'POST /api/todos': 'Create new todo',
      'PUT /api/todos/:id': 'Update todo',
      'DELETE /api/todos/:id': 'Delete todo',
      'GET /api/stats': 'Get API statistics'
    }
  });
});

// Users API
app.get('/api/users', (req, res) => {
  const { role } = req.query;
  let filteredUsers = users;

  if (role) {
    filteredUsers = users.filter(user => user.role === role);
  }

  res.json({
    users: filteredUsers,
    count: filteredUsers.length
  });
});

app.get('/api/users/:id', (req, res) => {
  const id = parseInt(req.params.id);
  const user = users.find(u => u.id === id);

  if (!user) {
    return res.status(404).json({ error: 'User not found' });
  }

  res.json(user);
});

app.post('/api/users', (req, res) => {
  const { name, email, role = 'user' } = req.body;

  if (!name || !email) {
    return res.status(400).json({ error: 'Name and email are required' });
  }

  const newUser = {
    id: Math.max(...users.map(u => u.id)) + 1,
    name,
    email,
    role
  };

  users.push(newUser);
  res.status(201).json(newUser);
});

// Todos API
app.get('/api/todos', (req, res) => {
  const { userId, completed } = req.query;
  let filteredTodos = todos;

  if (userId) {
    filteredTodos = filteredTodos.filter(todo => todo.userId === parseInt(userId));
  }

  if (completed !== undefined) {
    filteredTodos = filteredTodos.filter(todo => todo.completed === (completed === 'true'));
  }

  res.json({
    todos: filteredTodos,
    count: filteredTodos.length
  });
});

app.get('/api/todos/:id', (req, res) => {
  const id = parseInt(req.params.id);
  const todo = todos.find(t => t.id === id);

  if (!todo) {
    return res.status(404).json({ error: 'Todo not found' });
  }

  res.json(todo);
});

app.post('/api/todos', (req, res) => {
  const { title, userId } = req.body;

  if (!title || !userId) {
    return res.status(400).json({ error: 'Title and userId are required' });
  }

  const newTodo = {
    id: Math.max(...todos.map(t => t.id)) + 1,
    title,
    completed: false,
    userId: parseInt(userId)
  };

  todos.push(newTodo);
  res.status(201).json(newTodo);
});

app.put('/api/todos/:id', (req, res) => {
  const id = parseInt(req.params.id);
  const todoIndex = todos.findIndex(t => t.id === id);

  if (todoIndex === -1) {
    return res.status(404).json({ error: 'Todo not found' });
  }

  const { title, completed } = req.body;

  if (title !== undefined) todos[todoIndex].title = title;
  if (completed !== undefined) todos[todoIndex].completed = completed;

  res.json(todos[todoIndex]);
});

app.delete('/api/todos/:id', (req, res) => {
  const id = parseInt(req.params.id);
  const todoIndex = todos.findIndex(t => t.id === id);

  if (todoIndex === -1) {
    return res.status(404).json({ error: 'Todo not found' });
  }

  const deletedTodo = todos.splice(todoIndex, 1)[0];
  res.json(deletedTodo);
});

// Statistics endpoint
app.get('/api/stats', (req, res) => {
  const stats = {
    totalUsers: users.length,
    totalTodos: todos.length,
    completedTodos: todos.filter(t => t.completed).length,
    pendingTodos: todos.filter(t => !t.completed).length,
    usersByRole: {
      admin: users.filter(u => u.role === 'admin').length,
      user: users.filter(u => u.role === 'user').length
    },
    serverInfo: {
      nodeVersion: process.version,
      platform: process.platform,
      uptime: process.uptime(),
      memory: process.memoryUsage(),
      pid: process.pid
    }
  };

  res.json(stats);
});

// 404 handler
app.use('*', (req, res) => {
  res.status(404).json({
    error: 'Endpoint not found',
    message: `${req.method} ${req.originalUrl} is not a valid endpoint`,
    availableEndpoints: '/'
  });
});

// Error handler
app.use((err, req, res, next) => {
  console.error('Error:', err);
  res.status(500).json({
    error: 'Internal server error',
    message: err.message
  });
});

// Start server
app.listen(PORT, () => {
  console.log(`🚀 Express API server running on port ${PORT}`);
  console.log(`📍 Visit http://localhost:${PORT} for API documentation`);
  console.log(`🔍 Health check: http://localhost:${PORT}/health`);
  console.log(`👥 Users API: http://localhost:${PORT}/api/users`);
  console.log(`📝 Todos API: http://localhost:${PORT}/api/todos`);
  console.log(`📊 Statistics: http://localhost:${PORT}/api/stats`);
  console.log('---');
  console.log('🌟 Running in wasmrun OS mode!');
});