---
name: api-reference
tags: documentation, api, reference
type: _md_
---

# API Reference

This document provides comprehensive API documentation for our service.

## Authentication

All API endpoints require authentication using Bearer tokens:

```bash
curl -H "Authorization: Bearer YOUR_TOKEN" https://api.example.com/v1/users
```

## Endpoints

### Users

#### GET /v1/users
List all users in the system.

**Response:**
```json
{
  "users": [
    {"id": 1, "name": "Alice", "email": "alice@example.com"},
    {"id": 2, "name": "Bob", "email": "bob@example.com"}
  ]
}
```

#### POST /v1/users
Create a new user.

**Request:**
```json
{
  "name": "Charlie",
  "email": "charlie@example.com"
}
```