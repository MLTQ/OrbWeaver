# Graphchan Network System Overview

## Architecture Overview
Graphchan implements a secure peer-to-peer network system that enables decentralized communication between nodes. The system is built on several interconnected components that work together to provide secure, reliable peer connections and message exchange.

## Core Components

### 1. [Friend Code System](friendcode.md)
The entry point for peer connections:
- Generates shareable connection codes
- Includes peer public keys and connection info
- Implements time-limited validity
- Provides secure serialization format
- Prevents unauthorized connections

### 2. [Connection Management](network_connection.md)
Handles the lifecycle of peer connections:
- Maintains active peer connections
- Manages connection state
- Provides thread-safe access
- Implements connection pooling
- Handles connection cleanup

### 3. [Handshake Protocol](network_handshake.md)
Ensures secure connection establishment:
- Mutual authentication
- Challenge-response verification
- Timestamp validation
- Signature verification
- Prevents replay attacks

### 4. [Incoming Connection Handler](network_incoming.md)
Manages new peer connections:
- Implements server-side handshake
- Handles connection setup
- Detects duplicate connections
- Enforces timeouts
- Manages connection lifecycle

### 5. [Message System](network_message.md)
Implements the communication protocol:
- Defines message types
- Handles message serialization
- Maintains connection health
- Provides secure message exchange
- Implements ping/pong mechanism

## Security Model

### Authentication
- Ed25519 public key cryptography
- Signed friend codes
- Mutual handshake verification
- Signed messages
- Challenge-response protocol

### Connection Security
- Time-limited friend codes
- Handshake timeouts
- Connection keepalive
- Duplicate connection prevention
- Clock synchronization checks

### Message Security
- Signed messages
- Type safety
- Length validation
- Immediate termination on verification failure

## Protocol Flow

### 1. Connection Establishment
```
User A                        User B
  |                            |
  |-- Friend Code Generation --|
  |-- Code Share (out of band) ->|
  |<---- Connection Request ----|
  |---- Handshake Exchange ---->|
  |<--- Challenge-Response ---->|
  |---- Connection Complete --->|
```

### 2. Message Exchange
```
Peer A                        Peer B
  |                            |
  |---- Signed Message ------->|
  |<--- Message Verification --|
  |<--- Message Handling ------|
  |                            |
  |<---- Periodic Pings ------>|
```

## Thread Safety
The system implements comprehensive thread safety through:
- Connection-level mutexes
- Connection manager locks
- Atomic operations
- Safe resource cleanup
- Protected shared state

## Error Handling
Robust error handling across components:
- Detailed error reporting
- Graceful connection termination
- Resource cleanup
- State consistency
- Automatic recovery

## Dependencies
External dependencies are minimal:
- Standard library `crypto/ed25519`
- Standard library `net`
- Standard library `time`
- Standard library `encoding/binary`
- Standard library `crypto/rand`

## Future Considerations
Areas for potential enhancement:
1. Connection pooling optimization
2. Advanced peer discovery
3. NAT traversal
4. Network topology optimization
5. Message compression
6. Rate limiting
7. Bandwidth management

## Usage Examples
Common usage patterns:
1. Establishing new peer connections
2. Broadcasting messages
3. Direct peer messaging
4. Thread state synchronization
5. Post data exchange

## System Constraints
Current limitations and boundaries:
- Single thread per connection
- In-memory connection tracking
- Fixed protocol version
- Synchronous message handling
- Local network focus 