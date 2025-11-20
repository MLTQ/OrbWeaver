# Network Message System

## Overview
The message system implements the Graphchan protocol's message handling, including message types, serialization, authentication, and connection maintenance through ping/pong mechanisms. It provides a secure and reliable way for peers to exchange various types of messages.

## Core Components

### Message Types
```go
type MessageType uint8
```
Supported message categories:
1. **Core Protocol**
   - `MsgPing` - Connection keepalive request
   - `MsgPong` - Connection keepalive response

2. **Post Management**
   - `MsgNewPost` - Notification of new post
   - `MsgRequestPost` - Request for post data
   - `MsgPostData` - Post content delivery

3. **Thread Management**
   - `MsgRequestThreadHashes` - Request for thread state
   - `MsgThreadHashes` - Thread state response
   - `MsgRequestMissingPosts` - Request for missing posts
   - `MsgText` - Plain text message

### Data Structures

#### Message
Represents a protocol message:
- `Type` - MessageType identifier
- `Payload` - Message content bytes
- `Signature` - Ed25519 signature of Type + Payload

## Key Functions

### Message Handling

#### handleMessages
```go
func (cm *ConnectionManager) handleMessages(conn *Connection)
```
Main message loop that:
- Maintains connection through periodic pings
- Reads and verifies incoming messages
- Handles message routing
- Manages connection lifecycle
- Cleans up on connection termination

#### handleMessage
```go
func (cm *ConnectionManager) handleMessage(conn *Connection, msg *Message) error
```
Routes messages based on type:
- Responds to pings with pongs
- Processes text messages
- Handles unknown message types

### Message I/O

#### readMessage
```go
func (conn *Connection) readMessage() (*Message, error)
```
Reads a message from the connection:
1. Reads message type (1 byte)
2. Reads payload length (4 bytes)
3. Reads payload (if present)
4. Reads Ed25519 signature
5. Returns parsed message

#### writeMessage
```go
func (conn *Connection) writeMessage(msgType MessageType, payload []byte) error
```
Writes a message to the connection:
1. Signs message content
2. Writes message type
3. Writes payload length and data
4. Writes signature

### Connection Maintenance

#### Ping/Pong
```go
func (conn *Connection) sendPing() error
func (conn *Connection) sendPong() error
```
Implements keepalive mechanism:
- Pings sent every minute
- Pongs must be received to maintain connection
- Automatic connection cleanup on failure

## Security Features

### Message Authentication
- All messages signed with Ed25519
- Signatures cover both type and payload
- Immediate disconnect on signature failure

### Connection Health
- Regular ping/pong verification
- Timestamp tracking of last received message
- Automatic cleanup of dead connections

## Protocol Format

### Message Structure
Binary format with big-endian encoding:
1. Message Type (1 byte)
2. Payload Length (4 bytes)
3. Payload (variable length)
4. Ed25519 Signature (fixed length)

## Dependencies
- [Network Connection System](network_connection.md) for connection management
- Standard library `crypto/ed25519` for message signing
- Standard library `encoding/binary` for message serialization
- Standard library `time` for connection maintenance

## Error Handling
- Detailed error reporting for message processing
- Graceful connection cleanup on errors
- Automatic resource cleanup
- Connection state maintenance

## Thread Safety
- Connection-level mutex protection
- Safe concurrent message handling
- Protected access to shared resources
- Clean connection termination

## Usage Context
The message system is used to:
- Exchange post and thread data
- Maintain peer connections
- Provide reliable message delivery
- Enable secure peer communication 