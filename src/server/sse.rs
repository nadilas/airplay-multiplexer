use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::Router;
use futures::Stream;

use crate::server::AppState;

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/api/events", get(sse_handler))
}

async fn sse_handler(
    State(state): State<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut status_rx = state.status_tx.subscribe();
    let multiplexer = state.multiplexer.clone();

    let stream = async_stream::stream! {
        // Send initial connected event
        yield Ok(Event::default().data(r#"{"type":"connected"}"#));

        // Send initial status
        {
            let mux = multiplexer.read().await;
            let status = mux.get_status();
            let mut payload = serde_json::to_value(&status).unwrap_or_default();
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("type".to_string(), serde_json::Value::String("status".to_string()));
            }
            yield Ok(Event::default().data(serde_json::to_string(&payload).unwrap_or_default()));
        }

        // Watch for changes
        loop {
            match status_rx.changed().await {
                Ok(()) => {
                    let mux = multiplexer.read().await;
                    let status = mux.get_status();
                    let mut payload = serde_json::to_value(&status).unwrap_or_default();
                    if let Some(obj) = payload.as_object_mut() {
                        obj.insert("type".to_string(), serde_json::Value::String("status".to_string()));
                    }
                    yield Ok(Event::default().data(serde_json::to_string(&payload).unwrap_or_default()));
                }
                Err(_) => break,
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("heartbeat"),
    )
}
