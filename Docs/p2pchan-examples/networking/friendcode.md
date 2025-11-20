# Friend Code System Documentation

This document describes the friend code system implemented in `friendcode.go`. Friend codes are secure, shareable identifiers that enable peer discovery and connection in the Graphchan network.

## Overview

Friend codes are cryptographically secure, time-limited tokens that contain:
- Peer identification (public key)
- Connection information (IP, port)
- Node metadata (name)
- Temporal validity (generation and expiration times)
- Cryptographic proof of ownership (signature)

## Data Structures

### ConnectionInfo
```go
type ConnectionInfo struct {
    IP       net.IP
    Port     uint16
    NodeName string
}
```
Represents peer connection details:
- Supports both IPv4 and IPv6 addresses
- Uses standard port numbering
- Optional human-readable node identification

### FriendCode
```go
type FriendCode struct {
    PubKey     ed25519.PublicKey
    Connection ConnectionInfo
    Generated  time.Time
    Expires    time.Time
    Signature  []byte
}
```
The complete friend code structure containing:
- Ed25519 public key for peer identification
- Connection details for network access
- Temporal bounds for validity
- Cryptographic signature for authenticity

## Core Operations

### Generate Friend Code
```go
func Generate(privateKey ed25519.PrivateKey, ip net.IP, port uint16, nodeName string) (*FriendCode, error)
```
Creates a new friend code:
- Uses Ed25519 for cryptographic operations
- Sets 24-hour default expiration
- Signs all fields with private key
- Returns complete, verified friend code

### Verify Friend Code
```go
func (fc *FriendCode) Verify() error
```
Validates a friend code:
- Checks temporal validity
- Verifies cryptographic signature
- Returns error if invalid

### String Conversion

#### ToString
```go
func (fc *FriendCode) ToString() string
```
Serializes friend code to string:
- Efficient byte packing
- Base64URL encoding
- Includes all necessary fields
- Preserves cryptographic properties

#### FromString
```go
func FromString(code string) (*FriendCode, error)
```
Parses string representation:
- Validates minimum length
- Handles IPv4/IPv6
- Parses timestamps
- Verifies data integrity

## Binary Format

The friend code uses a compact binary format:
1. Public Key (32 bytes)
2. IP Version (1 byte)
3. IP Address (4 or 16 bytes)
4. Port (2 bytes, big-endian)
5. Node Name (null-terminated string)
6. Generated Time (RFC3339 format)
7. Expiry Time (RFC3339 format)
8. Ed25519 Signature (64 bytes)

## Security Features

### Cryptographic Security
- Ed25519 public-key cryptography
- Signature covers all fields
- Tamper-evident design

### Temporal Security
- Time-limited validity
- Explicit expiration
- Generation timestamp

### Format Security
- Length validation
- Format verification
- Error detection

## Error Handling

The implementation handles various error conditions:
1. Invalid format or encoding
2. Insufficient data length
3. Malformed fields
4. Invalid timestamps
5. Failed signature verification
6. Expired codes

## Best Practices

When using friend codes:

1. **Generation**
   - Use strong private keys
   - Set appropriate expiration
   - Validate input data

2. **Verification**
   - Always verify before use
   - Check expiration time
   - Validate signatures

3. **Storage**
   - Store securely
   - Handle expiration
   - Manage key lifecycle

4. **Network Use**
   - Validate before connecting
   - Handle IP version differences
   - Respect node names

## Integration Points

The friend code system integrates with:
- Peer discovery system
- Network connection handling
- Cryptographic subsystem
- Database peer tracking

## Performance Considerations

The implementation optimizes for:
1. **Memory Usage**
   - Efficient binary packing
   - Pre-allocated buffers
   - Minimal copying

2. **CPU Usage**
   - Single-pass parsing
   - Efficient encoding
   - Minimal allocations

3. **Security**
   - Fast verification
   - Compact signatures
   - Quick validity checks 