# MyApp — Sample Project

## Overview

MyApp is a REST API for managing tasks and users. Built with Node.js, Express, and PostgreSQL.

## Architecture

- **API Layer:** Express.js routes in `src/routes/`
- **Business Logic:** Services in `src/services/`
- **Database:** PostgreSQL via Knex.js ORM
- **Authentication:** JWT tokens stored in `Authorization` header

## API Endpoints

### Authentication
- `POST /auth/login` — Login with email + password, returns JWT token
- `POST /auth/register` — Register new user
- `POST /auth/logout` — Invalidate token

### Users
- `GET /users/:id` — Get user by ID
- `PUT /users/:id` — Update user profile
- `DELETE /users/:id` — Delete user (admin only)

### Tasks
- `GET /tasks` — List all tasks for authenticated user
- `POST /tasks` — Create new task
- `PUT /tasks/:id` — Update task
- `DELETE /tasks/:id` — Delete task
- `PATCH /tasks/:id/complete` — Mark task as complete

## Environment Variables

```env
DATABASE_URL=postgresql://user:pass@localhost:5432/myapp
JWT_SECRET=your-secret-key
PORT=3000
NODE_ENV=production
```

## Authentication Flow

1. User sends POST /auth/login with credentials
2. Server validates against database
3. Server returns JWT token (expires in 24h)
4. Client sends token in `Authorization: Bearer <token>` header
5. Middleware validates token on protected routes
