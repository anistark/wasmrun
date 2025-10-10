# Express API Example for wasmrun OS Mode

This is a sample Node.js Express application designed to test wasmrun's OS mode functionality. It provides a complete REST API with multiple endpoints for managing users and todos.

## API Endpoints

### General
- `GET /` - Welcome message and API documentation
- `GET /health` - Health check and server status

### Users API
- `GET /api/users` - Get all users (supports `?role=admin|user` filter)
- `GET /api/users/:id` - Get user by ID
- `POST /api/users` - Create new user (requires `name`, `email`, optional `role`)

### Todos API
- `GET /api/todos` - Get all todos (supports `?userId=1` and `?completed=true|false` filters)
- `GET /api/todos/:id` - Get todo by ID
- `POST /api/todos` - Create new todo (requires `title`, `userId`)
- `PUT /api/todos/:id` - Update todo (optional `title`, `completed`)
- `DELETE /api/todos/:id` - Delete todo

### Statistics
- `GET /api/stats` - Get comprehensive server and data statistics

## Testing the API

Once running, you can test the endpoints:

```bash
# Health check
curl http://localhost:3000/health

# Get all users
curl http://localhost:3000/api/users

# Get admin users only
curl "http://localhost:3000/api/users?role=admin"

# Create a new user
curl -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d '{"name":"David","email":"david@example.com","role":"user"}'

# Get all todos
curl http://localhost:3000/api/todos

# Create a new todo
curl -X POST http://localhost:3000/api/todos \
  -H "Content-Type: application/json" \
  -d '{"title":"Test wasmrun OS mode","userId":1}'

# Update a todo
curl -X PUT http://localhost:3000/api/todos/1 \
  -H "Content-Type: application/json" \
  -d '{"completed":true}'

# Get statistics
curl http://localhost:3000/api/stats
```
