use tracing_subscriber::EnvFilter;

/// Initializes a global tracing subscriber respecting the `RUST_LOG`
/// environment variable. Subsequent calls become no-ops so multiple binaries
/// can safely invoke it.
pub fn init_tracing() {
    let env_filter = std::env::var("RUST_LOG")
        .map(EnvFilter::new)
        .unwrap_or_else(|_| EnvFilter::new("graphchan_backend=info,tower_http=info"));
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}
