# Incoming Connection Handler

## Overview
The incoming connection handler manages the process of accepting and authenticating new peer connections in Graphchan. It implements the server-side of the handshake protocol, handles connection setup, and manages connection lifecycle including duplicate connection detection.

## Core Function

### handleIncoming
```go
func (cm *ConnectionManager) handleIncoming(conn net.Conn)
```
Processes new incoming connections with the following steps:

1. **Initial Setup**
   - Sets 30-second deadline for handshake completion
   - Defers deadline clearing after successful handshake

2. **Handshake Process**
   - Reads peer's handshake message
   - Generates and sends response handshake
   - Performs mutual challenge-response verification
   - Validates timestamps and signatures

3. **Connection Management**
   - Creates new Connection object
   - Handles duplicate connection detection
   - Adds successful connections to connection manager
   - Launches message handling routine

## Security Features

### Timeouts and Deadlines
- 30-second handshake completion deadline
- Prevents hanging connections
- Automatic cleanup of failed handshakes

### Cryptographic Verification
- Full signature verification of peer messages
- Challenge-response authentication
- Timestamp validation within MaxTimeDrift

### Duplicate Protection
- Checks for existing connections from same peer
- Maintains older connection on duplicate
- Prevents connection hijacking attempts

## Error Handling

### Handshake Failures
Detailed error reporting for:
- Handshake read/write failures
- Challenge generation errors
- Signature verification failures
- Timestamp validation issues
- Challenge response verification

### Connection Management
- Automatic connection cleanup on errors
- Proper mutex handling for thread safety
- Graceful handling of duplicate connections

## Dependencies
- [Network Handshake System](network_handshake.md) for handshake protocol
- [Network Connection System](network_connection.md) for connection management
- Standard library `crypto/ed25519` for cryptographic operations
- Standard library `crypto/rand` for challenge generation
- Standard library `time` for deadline management

## Connection Lifecycle

1. **Connection Acceptance**
   - Receives raw TCP connection
   - Sets initial deadlines
   - Prepares for handshake

2. **Authentication**
   - Reads peer handshake
   - Verifies peer identity
   - Performs challenge-response
   - Validates timestamps

3. **Connection Setup**
   - Creates Connection object
   - Checks for duplicates
   - Adds to connection manager
   - Starts message handling

4. **Error Cases**
   - Closes connection on any failure
   - Logs detailed error information
   - Cleans up resources
   - Prevents partial connections

## Thread Safety
- Uses connection manager mutex for map access
- Ensures atomic connection registration
- Prevents race conditions in duplicate detection
- Safe concurrent access to shared resources

## Usage Context
The incoming connection handler is used to:
- Accept new peer connections
- Implement server-side handshake
- Manage connection lifecycle
- Prevent unauthorized connections
- Handle connection duplicates 