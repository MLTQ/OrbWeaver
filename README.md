# OrbWeaver

This workspace now contains the Rust backend (`graphchan_backend`), the egui client (`graphchan_frontend`), and a bundled desktop launcher (`graphchan_desktop`).

## Highlights (Recent Updates)

- **Portable & Self-Contained**: 
  - **Embedded GPG**: Identity generation uses the pure Rust `sequoia-openpgp` library. No external `gpg` installation required.
  - **Static Linking**: SDL2 and FFmpeg are statically linked on macOS/Linux/Windows. No system dependencies needed to run.
- **Enhanced UI**:
  - **New Views**: Added "Sugiyama" (Hierarchical) and "Chronological" (Timeline) views alongside the existing Graph view.
  - **Keyboard Navigation**: Full keyboard support (Tab/Shift+Tab, Arrows) for navigating threads.
  - **Polished UX**: Improved camera centering, dot grid backgrounds, and smoother interactions.

## Running
- `cargo run -p graphchan_desktop` – bundled desktop mode. Boots the backend internally and points the GUI at `http://127.0.0.1:{GRAPHCHAN_API_PORT}`.
- `cargo run -p graphchan_backend -- serve` – standalone REST daemon; pair with the frontend or scripts of your choice.
- `cargo run -p graphchan_backend -- cli` – interactive shell for friendcodes, manual posting, and file transfers.
- `cargo run -p graphchan_frontend` – GUI that connects to any reachable backend (configure the API URL in the toolbar).
