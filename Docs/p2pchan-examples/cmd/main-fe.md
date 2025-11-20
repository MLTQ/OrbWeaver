# Frontend Server Documentation

## Overview
The `main-fe.go` file implements the main frontend server for Graphchan, serving both the API endpoints and static content. It integrates the database, P2P network, and HTTP server components while managing file system resources and cryptographic identities.

## Core Components

### Server Initialization
1. **Directory Structure**
   - Creates required directories:
     - `data`: For database and node keys
     - `uploads`: For user-uploaded content
     - `uploads/thumbnails`: For image thumbnails
     - `internal/static`: For Next.js static files

2. **Database Setup**
   - Initializes SQLite database with WAL mode
   - Configures optimizations:
     - WAL journaling
     - Normal synchronization
     - Foreign key constraints

3. **Cryptographic Identity**
   - Manages node keypair (Ed25519)
   - Functions:
     - `loadOrCreateKeypair`: Loads existing or generates new keys
     - Persists keys to `node.key`
     - Maintains consistent node identity

### Middleware Stack

1. **Custom Response Writer**
   - `responseWriter` struct extends `http.ResponseWriter`
   - Captures response status and body
   - Enables response logging and modification

2. **Logging Middleware**
   - `logResponseMiddleware`: Logs API responses
   - Specifically targets conversation endpoints
   - Maintains response integrity

3. **Standard Middleware**
   - Logger: Request logging
   - Recoverer: Panic recovery
   - CORS: Cross-origin resource sharing
     - Configured for development endpoints
     - Allows specific methods and headers

### Routing Structure

1. **API Routes** (`/api`)
   - Thread Management:
     - `GET /threads`: List threads
     - `GET /conversations/{id}`: Get conversation
   - Post Operations:
     - `POST /posts`: Create post
     - `POST /posts/{id}/like`: Like post
     - `PUT /posts/{id}/position`: Update position
   - Import Functionality:
     - `POST /import/4chan`: Import 4chan threads
   - Media Handling:
     - `POST /images/upload`: Upload images

2. **Static File Serving**
   - Uploads directory (`/uploads/*`)
   - Next.js static files
   - Client-side routing support

3. **Frontend Routes**
   - Root path handling
   - Next.js asset serving (`/_next/*`)
   - Fallback to index.html for client routing

## Integration Points

### Database Integration
- SQLite database initialization
- WAL mode configuration
- Performance optimizations
- Foreign key enforcement

### P2P Network
- Initializes network manager on port 8081
- Integrates with API handlers
- Uses Ed25519 keypair for identity

### File System
- Manages directory structure
- Handles uploads and thumbnails
- Serves static content
- Maintains node identity files

## Security Considerations
- Secure key management
- CORS configuration
- File system permissions
- Response logging controls
- Database security settings

## Dependencies
1. **External Packages**
   - `github.com/go-chi/chi/v5`: HTTP routing
   - `github.com/go-chi/cors`: CORS handling

2. **Internal Packages**
   - `Graphchan/internal/api`: API handlers
   - `Graphchan/internal/database`: Data persistence
   - `Graphchan/internal/network`: P2P networking

3. **Standard Library**
   - `crypto/ed25519`: Cryptographic operations
   - `crypto/rand`: Secure random generation
   - `net/http`: HTTP server
   - `os`: File system operations

## Configuration
- HTTP server: Port 8080
- P2P network: Port 8081
- Database location: `data/graphchan.db`
- Upload directory: `uploads/`
- Static content: `internal/static/out/`

## Limitations
- Development-focused CORS settings
- Single-node configuration
- Local file system dependency
- Fixed port assignments

## Error Handling
- Directory creation validation
- Database initialization checks
- Keypair management
- Server startup validation
- Middleware error recovery 