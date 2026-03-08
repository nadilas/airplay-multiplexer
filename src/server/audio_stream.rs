use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::State;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use bytes::Bytes;
use tokio::sync::broadcast;

use crate::server::AppState;

static STREAM_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/audio/stream", get(audio_stream))
}

async fn audio_stream(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let client_id = STREAM_COUNTER.fetch_add(1, Ordering::Relaxed);
    tracing::info!("[audio-stream] New client connected: http-client-{}", client_id);

    let wav_header = state.stream_manager.create_wav_header();
    let mut rx = state.stream_manager.subscribe();

    let stream = async_stream::stream! {
        // Send WAV header first
        yield Ok::<Bytes, std::io::Error>(Bytes::copy_from_slice(&wav_header));

        // Then stream audio data
        loop {
            match rx.recv().await {
                Ok(chunk) => yield Ok(chunk),
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!(
                        "[audio-stream] Client {} lagged, dropped {} chunks",
                        client_id, n
                    );
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }

        tracing::info!("[audio-stream] Client disconnected: http-client-{}", client_id);
    };

    (
        [
            ("content-type", "audio/wav"),
            ("transfer-encoding", "chunked"),
            ("cache-control", "no-cache, no-store"),
            ("connection", "keep-alive"),
        ],
        Body::from_stream(stream),
    )
}
