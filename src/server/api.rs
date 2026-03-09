use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::server::AppState;
use crate::types::{DeviceInfo, MultiplexerStatus, RoomStatus, SystemStatus};

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

#[derive(Deserialize)]
pub struct CreateRoomRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct RenameRoomRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct AssignDeviceRequest {
    #[serde(rename = "deviceId")]
    device_id: String,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Legacy endpoints (operate on default room)
        .route("/api/status", get(get_status))
        .route("/api/devices", get(get_devices))
        .route("/api/devices/{id}/volume", post(set_device_volume))
        .route("/api/devices/{id}/mute", post(set_device_mute))
        .route("/api/devices/{id}/enable", post(set_device_enable))
        .route("/api/master-volume", post(set_master_volume))
        // New room endpoints
        .route("/api/system/status", get(get_system_status))
        .route("/api/rooms", get(list_rooms).post(create_room))
        .route("/api/rooms/{room_id}", get(get_room).put(rename_room).delete(delete_room))
        .route("/api/rooms/{room_id}/devices", post(assign_device_to_room))
        .route("/api/rooms/{room_id}/devices/{device_id}", delete(unassign_device_from_room))
        .route("/api/rooms/{room_id}/devices/{device_id}/volume", post(set_room_device_volume))
        .route("/api/rooms/{room_id}/devices/{device_id}/mute", post(set_room_device_mute))
        .route("/api/rooms/{room_id}/devices/{device_id}/enable", post(set_room_device_enable))
        .route("/api/rooms/{room_id}/master-volume", post(set_room_master_volume))
        .route("/api/unassigned-devices", get(get_unassigned_devices))
}

// --- Legacy endpoints ---

async fn get_status(State(state): State<Arc<AppState>>) -> Json<MultiplexerStatus> {
    let mux = state.multiplexer.read().await;
    Json(mux.get_status().await)
}

async fn get_devices(State(state): State<Arc<AppState>>) -> Json<Vec<DeviceInfo>> {
    let mux = state.multiplexer.read().await;
    Json(mux.get_status().await.devices)
}

async fn set_device_volume(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<VolumeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let volume = parse_volume(&body.volume)?;
    let mut mux = state.multiplexer.write().await;
    match mux.legacy_set_device_volume(&id, volume).await {
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
    let muted = parse_bool(&body.muted, "muted")?;
    let mut mux = state.multiplexer.write().await;
    match mux.legacy_set_device_mute(&id, muted).await {
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
    let enabled = parse_bool(&body.enabled, "enabled")?;
    let mut mux = state.multiplexer.write().await;
    match mux.legacy_set_device_enabled(&id, enabled).await {
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
    let volume = parse_volume(&body.volume)?;
    let mut mux = state.multiplexer.write().await;
    match mux.legacy_set_master_volume(volume).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

// --- System endpoints ---

async fn get_system_status(State(state): State<Arc<AppState>>) -> Json<SystemStatus> {
    let mux = state.multiplexer.read().await;
    Json(mux.get_system_status().await)
}

// --- Room endpoints ---

async fn list_rooms(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<RoomStatus>> {
    let mux = state.multiplexer.read().await;
    let status = mux.get_system_status().await;
    Json(status.rooms)
}

async fn create_room(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateRoomRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if body.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Room name cannot be empty"})),
        ));
    }

    let mut mux = state.multiplexer.write().await;
    match mux.create_room(&body.name) {
        Ok(room_id) => {
            // Start the room's shairport instance
            let _ = mux.start_room(&room_id).await;
            Ok(Json(serde_json::json!({"ok": true, "roomId": room_id})))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn get_room(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
) -> Result<Json<RoomStatus>, (StatusCode, Json<serde_json::Value>)> {
    let mux = state.multiplexer.read().await;
    match mux.get_room_status_async(&room_id).await {
        Some(status) => Ok(Json(status)),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Room not found"})),
        )),
    }
}

async fn rename_room(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Json(body): Json<RenameRoomRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if body.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Room name cannot be empty"})),
        ));
    }

    let mut mux = state.multiplexer.write().await;
    match mux.rename_room(&room_id, &body.name) {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn delete_room(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut mux = state.multiplexer.write().await;
    match mux.delete_room(&room_id).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn assign_device_to_room(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Json(body): Json<AssignDeviceRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut mux = state.multiplexer.write().await;
    match mux.assign_device(&body.device_id, &room_id) {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn unassign_device_from_room(
    State(state): State<Arc<AppState>>,
    Path((_room_id, device_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let mut mux = state.multiplexer.write().await;
    match mux.unassign_device(&device_id) {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn set_room_device_volume(
    State(state): State<Arc<AppState>>,
    Path((room_id, device_id)): Path<(String, String)>,
    Json(body): Json<VolumeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let volume = parse_volume(&body.volume)?;
    let mut mux = state.multiplexer.write().await;
    match mux.set_device_volume(&room_id, &device_id, volume).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn set_room_device_mute(
    State(state): State<Arc<AppState>>,
    Path((room_id, device_id)): Path<(String, String)>,
    Json(body): Json<MuteRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let muted = parse_bool(&body.muted, "muted")?;
    let mut mux = state.multiplexer.write().await;
    match mux.set_device_mute(&room_id, &device_id, muted).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn set_room_device_enable(
    State(state): State<Arc<AppState>>,
    Path((room_id, device_id)): Path<(String, String)>,
    Json(body): Json<EnableRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let enabled = parse_bool(&body.enabled, "enabled")?;
    let mut mux = state.multiplexer.write().await;
    match mux.set_device_enabled(&room_id, &device_id, enabled).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn set_room_master_volume(
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<String>,
    Json(body): Json<VolumeRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let volume = parse_volume(&body.volume)?;
    let mut mux = state.multiplexer.write().await;
    match mux.set_room_master_volume(&room_id, volume).await {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Err((
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": e.to_string()})),
        )),
    }
}

async fn get_unassigned_devices(
    State(state): State<Arc<AppState>>,
) -> Json<Vec<DeviceInfo>> {
    let mux = state.multiplexer.read().await;
    let status = mux.get_system_status().await;
    Json(status.unassigned_devices)
}

// --- Helpers ---

fn parse_volume(value: &serde_json::Value) -> Result<u8, (StatusCode, Json<serde_json::Value>)> {
    match value.as_f64() {
        Some(v) if v >= 0.0 && v <= 100.0 => Ok(v as u8),
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Volume must be a number between 0 and 100"})),
        )),
    }
}

fn parse_bool(value: &serde_json::Value, name: &str) -> Result<bool, (StatusCode, Json<serde_json::Value>)> {
    match value.as_bool() {
        Some(b) => Ok(b),
        None => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("{} must be a boolean", name)})),
        )),
    }
}
