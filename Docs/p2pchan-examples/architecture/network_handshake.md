# Network Handshake System

## Overview
The handshake system implements a secure peer authentication protocol for Graphchan. It ensures that connections are established only between verified peers using a challenge-response mechanism with Ed25519 signatures and timestamp verification to prevent replay attacks.

## Core Components

### Data Structures

#### HandshakeMessage
Represents the initial handshake data exchanged between peers:
- `Version` - Protocol version (uint16)
- `Challenge` - 32-byte random challenge
- `Timestamp` - Unix timestamp for time synchronization check
- `PublicKey` - Ed25519 public key of the sender
- `Signature` - Ed25519 signature of all above fields

### Constants

```go
const MaxTimeDrift = time.Minute
```
Maximum allowed time difference between peers to prevent replay attacks and ensure reasonable clock synchronization.

## Key Functions

### Handshake Process

#### performHandshake
```go
func (cm *ConnectionManager) performHandshake(conn net.Conn, expectedPeerKey ed25519.PublicKey) error
```
Executes the complete handshake protocol:
1. Creates and signs local handshake message
2. Sends handshake to peer
3. Receives and verifies peer's handshake
4. Performs challenge-response verification
5. Validates timestamps are within acceptable drift

### Message Handling

#### bytesToSign
```go
func (msg *HandshakeMessage) bytesToSign() []byte
```
Serializes handshake message fields for signing:
1. Version (2 bytes, big-endian)
2. Challenge (32 bytes)
3. Timestamp (8 bytes, big-endian)
4. Public key (Ed25519 public key bytes)

#### writeHandshake
```go
func writeHandshake(conn net.Conn, msg HandshakeMessage) error
```
Writes handshake message to connection in binary format:
- All integers in big-endian format
- Fixed-size fields written directly
- No length prefixing (uses known sizes)

#### readHandshake
```go
func readHandshake(conn net.Conn) (*HandshakeMessage, error)
```
Reads and parses handshake message from connection:
- Reads fields in same order as written
- Validates all reads complete fully
- Returns parsed HandshakeMessage structure

## Security Measures

### Authentication
- Ed25519 signatures verify message authenticity
- Public keys must match expected peer keys
- Challenge-response prevents replay attacks

### Time Synchronization
- Timestamp validation ensures messages are recent
- MaxTimeDrift constant prevents old message replay
- Helps ensure network time synchronization

### Protocol Safety
- Fixed-size message format prevents buffer overflows
- Big-endian encoding for consistent cross-platform operation
- Complete message verification before acceptance

## Dependencies
- [Network Connection System](network_connection.md) for connection management
- Standard library `crypto/ed25519` for signatures
- Standard library `crypto/rand` for secure random challenges
- Standard library `encoding/binary` for message serialization
- Standard library `time` for timestamp handling

## Usage Context
The handshake system is used to:
- Authenticate peer connections
- Prevent connection spoofing
- Ensure message freshness
- Establish secure communication channels

## Error Handling
The system provides detailed error messages for:
- Public key mismatches
- Invalid signatures
- Time synchronization issues
- Network I/O failures
- Challenge-response failures 