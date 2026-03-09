use std::sync::Arc;

use tokio::sync::{watch, RwLock};

mod config;
mod devices;
mod discovery;
mod multiplexer;
mod persistence;
mod room;
mod server;
mod shairport;
mod stream_manager;
mod types;
mod upnp;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config::load_config();

    tracing::info!("=== Multi-Room Audio Multiplexer ===");
    tracing::info!("Receiver Name: {}", config.receiver_name);
    tracing::info!("Local IP: {}", config.local_ip);
    tracing::info!("HTTP Port: {}", config.http_port);
    tracing::info!("Database: {}", config.db_path);
    tracing::info!(
        "Audio Format: {}Hz / {}bit / {}ch",
        config.audio_format.sample_rate,
        config.audio_format.bit_depth,
        config.audio_format.channels,
    );

    // Open database
    let db = Arc::new(persistence::Database::open(&config.db_path)?);

    let (status_tx, _status_rx) = watch::channel(());
    let http_port = config.http_port;

    let mux = multiplexer::AudioMultiplexer::new(
        config,
        db,
        status_tx.clone(),
    );
    let multiplexer = Arc::new(RwLock::new(mux));

    let app_state = Arc::new(server::AppState {
        multiplexer: multiplexer.clone(),
        status_tx,
    });

    let router = server::create_router(app_state);

    // Start background tasks (discovery + rooms + shairport)
    {
        let mut mux = multiplexer.write().await;
        mux.start().await;
    }

    // Start HTTP server
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], http_port));
    tracing::info!("[server] Listening on http://0.0.0.0:{}", http_port);
    tracing::info!("[server] Dashboard: http://0.0.0.0:{}/", http_port);
    tracing::info!("[server] API: http://0.0.0.0:{}/api/system/status", http_port);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let shutdown_mux = multiplexer.clone();
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let ctrl_c = tokio::signal::ctrl_c();
            let mut sigterm = tokio::signal::unix::signal(
                tokio::signal::unix::SignalKind::terminate(),
            )
            .expect("failed to install SIGTERM handler");

            tokio::select! {
                _ = ctrl_c => tracing::info!("[main] Received SIGINT"),
                _ = sigterm.recv() => tracing::info!("[main] Received SIGTERM"),
            }

            tracing::info!("[main] Shutting down...");
            let mut mux = shutdown_mux.write().await;
            mux.stop().await;
        })
        .await?;

    Ok(())
}
