# RetLister Server

Rust/Axum REST API for managing leftover wood pieces with SQLite backend.

## Features

- CRUD operations for wood remnants
- Search by dimensions, thickness, and material (case-insensitive)
- Statistics aggregation by material and thickness
- Partial update support
- SQLite with WAL mode for concurrent access
- RFC3339 timestamps

## Endpoints

### Resto Operations
- `POST /add` - Add new leftover piece
- `DELETE /remove/:id` - Remove piece by ID
- `POST /update/:id` - Partial update of piece attributes
- `GET /list` - List all pieces (newest first)
- `POST /search` - Find suitable leftovers for given dimensions

### Statistics
- `GET /stats` - Aggregate statistics by material and thickness

## Running

```powershell
cd RustServer
cargo run --release
```

Server listens on `http://localhost:8000`.

## Database

Location: `data/retlister.db`
Migrations: `Migrations/0001_leftovers.sql`

## Sample Data

```powershell
.\populate_db.ps1
```
