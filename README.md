# OrbWeaver

This workspace now contains the Rust backend (`graphchan_backend`), the egui client (`graphchan_frontend`), and a bundled desktop launcher (`graphchan_desktop`).

## Running
- `cargo run -p graphchan_desktop` – bundled desktop mode. Boots the backend internally and points the GUI at `http://127.0.0.1:{GRAPHCHAN_API_PORT}`.
- `cargo run -p graphchan_backend -- serve` – standalone REST daemon; pair with the frontend or scripts of your choice.
- `cargo run -p graphchan_backend -- cli` – interactive shell for friendcodes, manual posting, and file transfers.
- `cargo run -p graphchan_frontend` – GUI that connects to any reachable backend (configure the API URL in the toolbar).
