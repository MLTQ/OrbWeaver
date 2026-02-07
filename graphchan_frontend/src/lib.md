# lib.rs

## Purpose
Library entry point for the graphchan_frontend crate. Exposes public modules and provides convenience functions for launching the egui application with various configurations.

## Components

### Public Modules
- `api` - HTTP client for backend communication
- `app` - Main application state and UI
- `color_theme` - Theme color generation
- `importer` - Thread import helpers
- `models` - API data structures

### `run_frontend`
- **Does**: Launches app with default window options
- **Interacts with**: `run_frontend_with_options`, `default_native_options`
- **Returns**: `Result<(), eframe::Error>`

### `run_frontend_with_options`
- **Does**: Launches app with caller-provided NativeOptions
- **Interacts with**: `eframe::run_native`, `GraphchanApp::new`
- **Behavior**: Initializes env_logger, creates native window

### `default_native_options`
- **Does**: Returns sensible default window configuration
- **Settings**: 1100x720 initial size, 800x600 minimum

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `run_frontend()` available | Function removal |
| `graphchan_desktop` | `run_frontend_with_options` available | Signature change |
| External users | `GraphchanApp` re-exported | Export removal |

## Re-exports

- `pub use app::GraphchanApp` - Main app struct for embedding

## Usage

### Standalone
```rust
fn main() -> Result<(), eframe::Error> {
    graphchan_frontend::run_frontend()
}
```

### With Custom Options
```rust
let options = eframe::NativeOptions {
    viewport: egui::ViewportBuilder::default()
        .with_inner_size([1400.0, 900.0]),
    ..Default::default()
};
graphchan_frontend::run_frontend_with_options(options)
```

### Desktop Bundle
```rust
// Set API URL via environment
std::env::set_var("GRAPHCHAN_API_URL", "http://127.0.0.1:8080");
graphchan_frontend::run_frontend()
```

## Notes
- Logger initialized with `is_test(false)` to enable in release builds
- Window title is "Graphchan Frontend"
- App created via `GraphchanApp::new(cc)` with egui CreationContext
