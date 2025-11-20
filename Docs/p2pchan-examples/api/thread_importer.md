# Thread Importer Documentation

## Overview
The `thread_importer.go` file implements functionality to import threads from 4chan into the Graphchan system. It handles the complex process of downloading, parsing, and recreating thread structures while preserving relationships between posts and managing associated media.

## Importer Structure
The `ThreadImporter` struct manages the import process with these components:
- `db`: Database connection for storing imported content
- `postMap`: Maps 4chan post numbers to local post IDs
- `uploadsDir`: Directory for storing downloaded media
- `client`: HTTP client with timeout configuration

## Core Functionality

### Thread Import Process
1. **URL Parsing**
   - Function: `parseThreadURL`
   - Extracts board name and thread ID from 4chan URLs
   - Uses regex pattern matching
   - Returns board and thread identifiers

2. **Image Management**
   - Function: `downloadImage`
   - Downloads images from 4chan servers
   - Implements timeout handling
   - Returns raw image data

3. **Image Processing**
   - Function: `saveImage`
   - Features:
     - Hash calculation for deduplication
     - Thumbnail generation
     - Metadata extraction
     - File system organization
     - Database record creation
   - Implements file deduplication via hash checking
   - Creates standardized filename formats
   - Manages both full-size and thumbnail versions

4. **Post Parsing**
   - Function: `parsePosts`
   - Processes HTML structure using goquery
   - Extracts:
     - Post content and numbers
     - Image attachments
     - Reply references
     - Thread metadata
   - Creates spatial layout for posts
   - Maintains post relationships

## Error Handling

### Retry Mechanism
- Function: `retryWithBackoff`
- Implements exponential backoff
- Specifically handles database locks
- Maximum of 5 retry attempts
- Configurable delay between attempts

## Data Processing

### Thread Creation
1. Generates unique thread ID using SHA256
2. Creates thread record with metadata:
   - Title (from first post or default)
   - Creation timestamp
   - Activity status
   - Moderation flags

### Post Processing
1. Spatial Organization:
   - Grid-based layout
   - Configurable width
   - Automatic Y-axis progression

2. Content Extraction:
   - Post numbers
   - Message content
   - Author information
   - Creation timestamps
   - Image attachments
   - Reply references

### Image Processing Pipeline
1. Download Phase:
   - URL validation
   - HTTP GET request
   - Error handling

2. Storage Phase:
   - Hash calculation
   - Deduplication check
   - File system storage
   - Thumbnail generation

3. Database Integration:
   - Metadata storage
   - Path management
   - Relationship mapping

## Dependencies
- `github.com/PuerkitoBio/goquery`: HTML parsing
- `github.com/nfnt/resize`: Image processing
- Internal packages:
  - `database`: Data persistence
- Standard library:
  - `crypto/sha256`: Hash generation
  - `image`: Image processing
  - `net/http`: Network requests
  - `regexp`: URL parsing

## Security Considerations
- HTTP timeout configuration
- File path sanitization
- Hash-based deduplication
- Error handling and logging
- Safe file system operations

## Integration Points
- Database layer for content storage
- File system for media management
- External HTTP requests to 4chan
- Image processing pipeline

## Limitations
- Dependent on 4chan's HTML structure
- Subject to rate limiting
- Requires sufficient storage space
- Network dependency for media downloads 