use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use bytes::Bytes;
use tokio::sync::broadcast;

use crate::server::AppState;
use crate::stream_manager::StreamManager;

static STREAM_COUNTER: AtomicU64 = AtomicU64::new(0);

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/audio/stream", get(audio_stream_default))
        .route("/audio/stream/{room_id}", get(audio_stream_room))
}

async fn audio_stream_default(
    State(state): State<Arc<AppState>>,
) -> Result<impl IntoResponse, StatusCode> {
    let mux = state.multiplexer.read().await;
    let sm = mux.get_default_stream_manager()
        .ok_or(StatusCode::NOT_FOUND)?;
    drop(mux);
    Ok(stream_audio(sm))
}

async fn audio_stream_room(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
) -> Result<impl IntoResponse, StatusCode> {
    let mux = state.multiplexer.read().await;
    let sm = mux.get_room_stream_manager(&room_id)
        .ok_or(StatusCode::NOT_FOUND)?;
    drop(mux);
    Ok(stream_audio(sm))
}

fn stream_audio(stream_manager: Arc<StreamManager>) -> impl IntoResponse {
    let client_id = STREAM_COUNTER.fetch_add(1, Ordering::Relaxed);
    tracing::info!("[audio-stream] New client connected: http-client-{}", client_id);

    let wav_header = stream_manager.create_wav_header();
    let mut rx = stream_manager.subscribe();

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
