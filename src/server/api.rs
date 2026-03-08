use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::server::AppState;
use crate::types::{DeviceInfo, MultiplexerStatus};

#[derive(Deserialize)]
pub struct VolumeRequest {
    volume: serde_json::Value,
}

#[derive(Deserialize)]
pub struct MuteRequest {
    muted: serde_json::Value,
}

#[derive(Deserialize)]
pub struct EnableRequest {
    enabled: serde_json::Value,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/status", get(get_status))
        .route("/api/devices", get(get_devices))
        .route("/api/devices/{id}/volume", post(set_device_volume))
        .route("/api/devices/{id}/mute", post(set_device_mute))
        .route("/api/devices/{id}/enable", post(set_device_enable))
        .route("/api/master-volume", post(set_master_volume))
}

async fn get_status(State(state): State<Arc<AppState>>) -> Json<MultiplexerStatus> {
    let mux = state.multiplexer.read().await;
    Json(mux.get_status())
}

async fn get_devices(State(state): State<Arc<AppState>>) -> Json<Vec<DeviceInfo>> {
    let mux = state.multiplexer.read().await;
    Json(mux.get_status().devices)
}

async fn set_device_volume(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<VolumeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let volume = match body.volume.as_f64() {
        Some(v) if v >= 0.0 && v <= 100.0 => v as u8,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Volume must be a number between 0 and 100"})),
            ));
        }
    };

    let mut mux = state.multiplexer.write().await;
    match mux.set_device_volume(&id, volume).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn set_device_mute(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<MuteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let muted = match body.muted.as_bool() {
        Some(m) => m,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "muted must be a boolean"})),
            ));
        }
    };

    let mut mux = state.multiplexer.write().await;
    match mux.set_device_mute(&id, muted).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn set_device_enable(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<EnableRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let enabled = match body.enabled.as_bool() {
        Some(e) => e,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "enabled must be a boolean"})),
            ));
        }
    };

    let mut mux = state.multiplexer.write().await;
    match mux.set_device_enabled(&id, enabled).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn set_master_volume(
    State(state): State<Arc<AppState>>,
    Json(body): Json<VolumeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let volume = match body.volume.as_f64() {
        Some(v) if v >= 0.0 && v <= 100.0 => v as u8,
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Volume must be a number between 0 and 100"})),
            ));
        }
    };

    let mut mux = state.multiplexer.write().await;
    match mux.set_master_volume(volume).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}
