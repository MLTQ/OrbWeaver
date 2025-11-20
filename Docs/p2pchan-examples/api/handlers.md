# API Handlers Documentation

## Overview
The `handlers.go` file implements the HTTP API handlers for the Graphchan application. It provides the core functionality for managing posts, images, threads, and user interactions within the system.

## Handler Structure
The main `Handler` struct encapsulates three critical dependencies:
- `DB`: Database connection for persistent storage
- `Net`: Network connection manager for P2P communications
- `UploadsDir`: Directory path for storing uploaded files

## Core Endpoints

### Post Management
1. **GetConversation**
   - Endpoint: GET (implicit)
   - Retrieves all posts with their associated metadata
   - Constructs a graph-like structure with posts and edges
   - Returns posts with properly formatted image URLs and thread IDs
   - Includes metadata about the conversation state

2. **CreatePost**
   - Endpoint: POST (implicit)
   - Handles creation of new posts and threads
   - Manages thread ID generation and validation
   - Supports image attachments via ID references
   - Creates thread records for new conversations
   - Uses base64 encoding for thread IDs in responses

3. **UpdatePosition**
   - Endpoint: PUT `/:id/position`
   - Updates the X/Y coordinates of posts
   - Used for the spatial layout of the conversation

4. **LikePost**
   - Endpoint: POST `/:id/like`
   - Implements post reaction functionality
   - Tracks like counts per post

### Image Handling
1. **UploadImage**
   - Endpoint: POST (implicit)
   - Processes image uploads with the following features:
     - SHA256 hash calculation
     - Automatic thumbnail generation
     - MIME type detection
     - Image dimension extraction
     - Storage path management
   - Creates both full-size and thumbnail versions
   - Returns URLs for both versions

### Thread Management
1. **GetThreads**
   - Endpoint: GET (implicit)
   - Retrieves all active threads
   - Returns thread metadata including:
     - Title, content, author
     - Creation time and likes
     - Position coordinates
     - Associated images
   - Excludes deleted threads

2. **ImportThread**
   - Endpoint: POST (implicit)
   - Handles importing threads from 4chan
   - Uses a separate thread importer implementation

## Helper Functions
- `saveUploadedFile`: Manages file system storage of uploads
- `createThumbnail`: Generates thumbnails for uploaded images
- `getImageDimensions`: Extracts image dimensions
- `min`: Utility function for number comparison

## Dependencies
The handler implementation relies on several key packages:
- `database`: Internal package for data persistence
- `network`: Internal package for P2P communication
- `github.com/go-chi/chi/v5`: HTTP routing
- `github.com/nfnt/resize`: Image processing
- Standard library packages for:
  - Cryptography (sha256)
  - Image processing
  - HTTP handling
  - File system operations

## Security Considerations
- Implements file size limits (32MB) for uploads
- Uses SHA256 for file integrity
- Sanitizes file paths
- Validates thread IDs
- Implements proper error handling and status codes

## Data Flow
1. Requests come in through HTTP endpoints
2. Handler methods process and validate input
3. Database operations are performed
4. Files are managed when needed
5. Responses are formatted and returned as JSON

## Integration Points
- Database layer through `database.DB`
- Network layer through `network.ConnectionManager`
- File system through `UploadsDir`
- External services (4chan import functionality) 