---
name: graphchan
description: Run Graphchan desktop or backend binaries and use the REST/CLI interfaces.
---

# Graphchan (desktop + backend)

Use this skill when the user wants to run or interact with Graphchan nodes, automate posts,
or troubleshoot the local API/CLI.

## Quick start (desktop, recommended)

- Ensure `graphchan_desktop` is available and executable.
- Run: `./graphchan_desktop`
- The app bootstraps keys/data, starts the REST API, and launches the GUI.
- Default API base: `http://127.0.0.1:8080` (if in use, the backend tries the next free port).

## Quick start (backend only)

- Run the headless REST server: `./graphchan_backend -- serve`
- Run the interactive CLI: `./graphchan_backend -- cli`

## CLI essentials

- `friendcode` to print your short + legacy friend codes.
- `add-friend <code>` to register and attempt connection.
- `list-friends`, `list-threads [N]`, `view-thread <id>`, `new-thread "title" ["body"]`,
  `post <thread_id> "message"`, `upload <thread_id> <path>`, `download <file_id> [dest]`.

## REST API sanity checks

- Health: `curl http://127.0.0.1:8080/health`
- Threads: `curl http://127.0.0.1:8080/threads`
- Thread details: `curl http://127.0.0.1:8080/threads/<id>`
- Post files: `curl http://127.0.0.1:8080/posts/<id>/files`
- Peers: `curl http://127.0.0.1:8080/peers`

## Runtime layout (relative to the executable)

- `data/graphchan.db`
- `files/uploads/` and `files/downloads/`
- `blobs/`
- `keys/gpg/` and `keys/iroh.key`
- `logs/`

Copying the binary into a new folder creates an isolated node with its own data and keys.

## Configuration (environment variables)

- `GRAPHCHAN_API_PORT` (default `8080`)
- `GRAPHCHAN_API_URL` (frontend override when using a separate UI)
- `GRAPHCHAN_PUBLIC_ADDRS` (comma-separated multiaddrs/IPs to advertise)
- `GRAPHCHAN_RELAY_URL` (custom relay endpoint)
- `GRAPHCHAN_DISABLE_DHT` / `GRAPHCHAN_DISABLE_MDNS` (set to `1` or `true` to disable)
- `GRAPHCHAN_MAX_UPLOAD_BYTES` (global upload body limit)
