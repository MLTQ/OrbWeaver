# Friend Code System

## Overview
The friend code system provides a secure way to share peer connection information in Graphchan. It implements a self-contained verification system that allows peers to safely exchange connection details using signed, time-limited codes.

## Core Components

### Data Structures

#### ConnectionInfo
Represents the basic information needed to connect to a peer:
- `IP` - IPv4 or IPv6 address
- `Port` - Network port number
- `NodeName` - Optional friendly name for the node

#### FriendCode
The main structure that encapsulates a complete friend code:
- `PubKey` - Ed25519 public key of the peer
- `Connection` - ConnectionInfo for reaching the peer
- `Generated` - Timestamp of code generation
- `Expires` - Expiration timestamp (default 24 hours after generation)
- `Signature` - Ed25519 signature of all fields using peer's private key

## Key Functions

### Generate
```go
func Generate(privateKey ed25519.PrivateKey, ip net.IP, port uint16, nodeName string) (*FriendCode, error)
```
Creates a new friend code with:
- Automatic 24-hour expiration
- Digital signature using the provided private key
- Connection details from the provided parameters

### Verify
```go
func (fc *FriendCode) Verify() error
```
Validates a friend code by:
- Checking if the code has expired
- Verifying the digital signature

### String Conversion
The package provides bidirectional string conversion for friend codes:

#### ToString
```go
func (fc *FriendCode) ToString() string
```
Serializes a friend code to a base64 URL-encoded string with a compact binary format:
1. Public key
2. IP version and address
3. Port number
4. Node name (null-terminated)
5. Timestamps
6. Signature

#### FromString
```go
func FromString(code string) (*FriendCode, error)
```
Deserializes a friend code from its string representation with comprehensive error checking.

## Security Considerations
- Uses Ed25519 for cryptographic signatures
- Time-limited validity to prevent replay attacks
- All critical fields are included in the signature
- Compact binary format with careful bounds checking during parsing

## Usage Context
Friend codes are used in Graphchan to:
- Share peer connection details securely
- Verify the authenticity of connection information
- Enable time-limited peer discovery
- Facilitate human-shareable connection strings

## Dependencies
- Standard library `crypto/ed25519` for digital signatures
- Standard library `encoding/base64` for string encoding
- Standard library `net` for IP address handling
- Standard library `time` for expiration management 