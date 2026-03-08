use std::sync::Arc;

use axum::Router;
use tokio::sync::{watch, RwLock};

use crate::multiplexer::AudioMultiplexer;
use crate::stream_manager::StreamManager;

pub mod api;
pub mod audio_stream;
pub mod sse;
pub mod ui;

pub struct AppState {
    pub multiplexer: Arc<RwLock<AudioMultiplexer>>,
    pub stream_manager: Arc<StreamManager>,
    pub status_tx: watch::Sender<()>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(ui::routes())
        .merge(api::routes())
        .merge(audio_stream::routes())
        .merge(sse::routes())
        .with_state(state)
}
