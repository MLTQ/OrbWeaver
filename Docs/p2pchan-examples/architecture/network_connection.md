# Network Connection System

## Overview
The network connection system manages peer-to-peer connections in Graphchan. It provides a thread-safe way to establish, maintain, and communicate through authenticated connections between peers using the friend code system for initial connection setup.

## Core Components

### Data Structures

#### Connection
Represents a single authenticated peer connection:
- `conn` - Underlying TCP network connection
- `peerKey` - Ed25519 public key of the connected peer
- `privateKey` - Local node's Ed25519 private key for signing
- `connectedAt` - Connection establishment timestamp
- `lastReceived` - Timestamp of last received message
- `mutex` - Thread safety for connection operations

#### ConnectionManager
Manages the lifecycle of all peer connections:
- `listener` - TCP listener for incoming connections
- `connections` - Thread-safe map of active connections (keyed by peer public key)
- `privateKey` - Node's Ed25519 private key
- `mutex` - Thread safety for connection map operations

## Key Functions

### Connection Management

#### NewConnectionManager
```go
func NewConnectionManager(listenAddr string, privateKey ed25519.PrivateKey) (*ConnectionManager, error)
```
Creates a new connection manager that:
- Starts a TCP listener on the specified address
- Initializes the connection tracking map
- Begins accepting incoming connections in the background

#### Connect
```go
func (cm *ConnectionManager) Connect(code *friendcode.FriendCode) error
```
Establishes an outbound connection using a friend code:
- Creates TCP connection to peer's address
- Performs authentication handshake
- Adds successful connection to the connection map
- Starts message handling routine

### Message Broadcasting

#### Broadcast
```go
func (cm *ConnectionManager) Broadcast(text []byte)
```
Sends a message to all connected peers:
- Thread-safe access to connection map
- Reports number of connected peers
- Handles individual send failures gracefully

#### BroadcastTextMessage
```go
func (cm *ConnectionManager) BroadcastTextMessage(text []byte)
```
Specialized version of broadcast for text messages.

#### SendTextMessage
```go
func (cm *ConnectionManager) SendTextMessage(toPeer ed25519.PublicKey, text []byte) error
```
Sends a text message to a specific peer:
- Looks up peer connection by public key
- Returns error if peer not connected
- Thread-safe access to connection map

## Background Operations

### Connection Acceptance
The connection manager runs a continuous background routine that:
- Accepts incoming TCP connections
- Handles each new connection in a separate goroutine
- Maintains the listener loop for the lifetime of the manager

## Dependencies
- [Friend Code System](friendcode.md) for peer authentication
- Standard library `crypto/ed25519` for cryptographic operations
- Standard library `net` for TCP networking
- Standard library `sync` for thread safety
- Standard library `time` for connection timing

## Security Considerations
- All connections are authenticated using Ed25519 public keys
- Connection state is protected by mutexes for thread safety
- Connection tracking uses peer public keys as unique identifiers
- Separate goroutines for connection handling prevent blocking

## Usage Context
The connection system is used in Graphchan to:
- Maintain the peer-to-peer network topology
- Provide reliable message delivery between peers
- Enable both broadcast and direct messaging
- Handle concurrent connections efficiently 