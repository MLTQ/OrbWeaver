# Database Management Documentation

This document describes the core database management functionality in Graphchan, implemented in `database.go`. This file provides the fundamental database connection and initialization capabilities for the application.

## Overview

The database management system is built on top of SQLite3, providing a lightweight, file-based database solution that's well-suited for peer-to-peer applications. The implementation wraps the standard `database/sql` package with additional functionality specific to Graphchan's needs.

## Core Components

### DB Struct

```go
type DB struct {
    *sql.DB
}
```

The `DB` struct is a thin wrapper around the standard `sql.DB` type, embedding it to inherit all its functionality while allowing for extension with Graphchan-specific methods. This pattern enables:
- Direct access to all standard SQL functionality
- Addition of custom methods specific to Graphchan
- Type safety for database operations

### Key Functions

#### GetDB
```go
func (db *DB) GetDB() *sql.DB
```
A utility method that:
- Returns the underlying `*sql.DB` instance
- Enables direct access to the standard database interface when needed
- Useful for operations that require the raw database connection

#### New
```go
func New(dbPath string) (*DB, error)
```
Creates a new database connection without schema initialization:
- Ensures the parent directory exists (creates if missing)
- Opens a SQLite3 database connection
- Returns a wrapped DB instance
- Does not modify the database schema

Key features:
- Directory creation with 0755 permissions
- Error handling for filesystem operations
- Clean connection management

#### NewWithSchema
```go
func NewWithSchema(dbPath string) (*DB, error)
```
Creates a new database connection with schema initialization:
- Calls `New()` to establish the connection
- Initializes the database schema (see [schema.md](schema.md))
- Handles cleanup on initialization failure
- Returns a fully prepared database instance

Error handling:
- Closes the database connection on schema initialization failure
- Propagates underlying errors from both connection and schema initialization

## Usage Patterns

The database management system supports two primary initialization patterns:

1. **Basic Connection** (`New`):
   - Use when connecting to an existing database
   - When schema is already initialized
   - For temporary connections or testing

2. **Full Initialization** (`NewWithSchema`):
   - Use when setting up a new database
   - Ensures schema is properly initialized
   - Recommended for application startup

## Dependencies

The implementation relies on:
- `database/sql` - Standard Go database interface
- `github.com/mattn/go-sqlite3` - SQLite3 driver
- `os` - Filesystem operations
- `path/filepath` - Path manipulation

## Integration with Other Components

This core database management functionality is used by:
- Schema management ([schema.md](schema.md))
- Database operations (operations.go)
- Image handling (image_operations.go)

## Best Practices

When using this database management system:
1. Always close database connections when done
2. Use `NewWithSchema` for application initialization
3. Use `New` only when certain the schema is already initialized
4. Handle returned errors appropriately
5. Use the wrapped `DB` type rather than raw `sql.DB` when possible

## Error Handling

The implementation provides comprehensive error handling for:
- Directory creation failures
- Database connection issues
- Schema initialization problems
- Resource cleanup on failure

Each operation returns an error value that should be checked by the caller. 