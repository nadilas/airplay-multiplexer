use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use slug::slugify;
use tokio::sync::{mpsc, watch};

use crate::config::AppConfig;
use crate::devices::{self, DeviceControl};
use crate::discovery::{DeviceDiscovery, DiscoveryEvent};
use crate::persistence::Database;
use crate::room::Room;
use crate::stream_manager::StreamManager;
use crate::types::{
    DeviceConfig, DeviceInfo, MultiplexerStatus, RoomConfig, RoomId,
    RoomStatus, SystemStatus, TrackMetadata,
};

pub struct AudioMultiplexer {
    config: AppConfig,
    rooms: HashMap<RoomId, Room>,
    unassigned_devices: HashMap<String, Box<dyn DeviceControl>>,
    discovery: DeviceDiscovery,
    status_tx: watch::Sender<()>,
    db: Arc<Database>,
    default_room_id: Option<RoomId>,
}

impl AudioMultiplexer {
    pub fn new(
        config: AppConfig,
        db: Arc<Database>,
        status_tx: watch::Sender<()>,
    ) -> Self {
        Self {
            config,
            rooms: HashMap::new(),
            unassigned_devices: HashMap::new(),
            discovery: DeviceDiscovery::new(),
            status_tx,
            db,
            default_room_id: None,
        }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn default_room_id(&self) -> Option<&str> {
        self.default_room_id.as_deref()
    }

    /// Initialize rooms from database. Creates default room if none exist.
    pub async fn initialize_rooms(&mut self) -> Result<()> {
        let room_configs = self.db.load_rooms()?;

        if room_configs.is_empty() {
            // Create default room
            let default_config = RoomConfig {
                id: slugify(&self.config.receiver_name),
                name: self.config.receiver_name.clone(),
                receiver_name: self.config.receiver_name.clone(),
                shairport_port: self.config.shairport_base_port,
                is_default: true,
            };
            self.db.save_room(&default_config)?;
            self.add_room_from_config(&default_config);
            self.default_room_id = Some(default_config.id);
        } else {
            for rc in &room_configs {
                self.add_room_from_config(rc);
                if rc.is_default {
                    self.default_room_id = Some(rc.id.clone());
                }
            }
        }

        Ok(())
    }

    fn add_room_from_config(&mut self, config: &RoomConfig) {
        let audio_stream_url = format!(
            "http://{}:{}/audio/stream/{}",
            self.config.local_ip, self.config.http_port, config.id
        );

        let stream_manager = Arc::new(StreamManager::new(self.config.audio_format.clone()));

        let room = Room::new(
            config,
            &self.config.shairport_path,
            stream_manager,
            audio_stream_url,
            self.status_tx.clone(),
        );

        self.rooms.insert(config.id.clone(), room);
    }

    pub async fn start(&mut self) {
        tracing::info!("[multiplexer] Starting Audio Multiplexer...");

        // Initialize rooms from DB
        if let Err(e) = self.initialize_rooms().await {
            tracing::error!("[multiplexer] Failed to initialize rooms: {}", e);
        }

        // Start all rooms (shairport instances)
        let room_ids: Vec<_> = self.rooms.keys().cloned().collect();
        for room_id in room_ids {
            if let Some(room) = self.rooms.get_mut(&room_id) {
                room.start().await;
            }
        }

        // Start device discovery
        let (discovery_tx, discovery_rx) = mpsc::channel(32);
        self.discovery.start(discovery_tx);
        self.spawn_discovery_handler(discovery_rx);

        self.notify_status_changed();
    }

    pub async fn stop(&mut self) {
        tracing::info!("[multiplexer] Stopping Audio Multiplexer...");

        for (_, room) in self.rooms.iter_mut() {
            room.stop().await;
        }

        for (_, device) in self.unassigned_devices.iter_mut() {
            let _ = device.disconnect().await;
        }
        self.unassigned_devices.clear();

        self.discovery.stop().await;
        tracing::info!("[multiplexer] Stopped");
    }

    fn spawn_discovery_handler(&self, mut rx: mpsc::Receiver<DiscoveryEvent>) {
        let status_tx = self.status_tx.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    DiscoveryEvent::DeviceFound(config) => {
                        tracing::info!(
                            "[multiplexer] Discovered device: {} ({})",
                            config.name,
                            config.device_type
                        );
                        let _ = status_tx.send(());
                    }
                    DiscoveryEvent::DeviceLost(id) => {
                        tracing::info!("[multiplexer] Lost device: {}", id);
                        let _ = status_tx.send(());
                    }
                }
            }
        });
    }

    // --- Room management ---

    pub fn create_room(&mut self, name: &str) -> Result<RoomId> {
        let id = slugify(name);
        if self.rooms.contains_key(&id) {
            return Err(anyhow!("Room with ID '{}' already exists", id));
        }

        let port = self.db.get_next_shairport_port(self.config.shairport_base_port)?;

        let config = RoomConfig {
            id: id.clone(),
            name: name.to_string(),
            receiver_name: format!("{} - {}", self.config.receiver_name, name),
            shairport_port: port,
            is_default: false,
        };

        self.db.save_room(&config)?;
        self.add_room_from_config(&config);
        self.notify_status_changed();

        Ok(id)
    }

    pub async fn delete_room(&mut self, room_id: &str) -> Result<()> {
        if self.default_room_id.as_deref() == Some(room_id) {
            return Err(anyhow!("Cannot delete the default room"));
        }

        if let Some(mut room) = self.rooms.remove(room_id) {
            room.stop().await;

            // Move devices to unassigned
            let device_ids = room.device_ids();
            for did in device_ids {
                if let Some(device) = room.remove_device(&did) {
                    self.unassigned_devices.insert(did.clone(), device);
                    let _ = self.db.unassign_device(&did);
                }
            }
        }

        self.db.delete_room(room_id)?;
        self.notify_status_changed();
        Ok(())
    }

    pub fn rename_room(&mut self, room_id: &str, name: &str) -> Result<()> {
        let room = self.rooms.get_mut(room_id)
            .ok_or_else(|| anyhow!("Room not found: {}", room_id))?;
        room.set_name(name.to_string());
        self.db.update_room_name(room_id, name)?;
        self.notify_status_changed();
        Ok(())
    }

    pub async fn start_room(&mut self, room_id: &str) -> Result<()> {
        let room = self.rooms.get_mut(room_id)
            .ok_or_else(|| anyhow!("Room not found: {}", room_id))?;
        room.start().await;
        Ok(())
    }

    // --- Device management ---

    pub fn add_device(&mut self, config: DeviceConfig) {
        let device_id = config.id.clone();

        // Check if device is already in a room
        for room in self.rooms.values() {
            if room.has_device(&device_id) {
                return;
            }
        }
        if self.unassigned_devices.contains_key(&device_id) {
            return;
        }

        // Check if device has a persisted room assignment
        if let Ok(Some(room_id)) = self.db.get_device_room(&device_id) {
            if let Some(room) = self.rooms.get_mut(&room_id) {
                room.add_device(config);
                return;
            }
        }

        // Otherwise add to unassigned
        let name = config.name.clone();
        let device = devices::create_device(config);
        self.unassigned_devices.insert(device_id, device);
        tracing::info!("[multiplexer] Added unassigned device: {}", name);
        self.notify_status_changed();
    }

    pub fn remove_device(&mut self, id: &str) {
        // Try removing from rooms
        for room in self.rooms.values_mut() {
            if room.has_device(id) {
                room.remove_device(id);
                let _ = self.db.unassign_device(id);
                self.notify_status_changed();
                return;
            }
        }

        // Try removing from unassigned
        if self.unassigned_devices.remove(id).is_some() {
            self.notify_status_changed();
        }
    }

    pub fn assign_device(&mut self, device_id: &str, room_id: &str) -> Result<()> {
        if !self.rooms.contains_key(room_id) {
            return Err(anyhow!("Room not found: {}", room_id));
        }

        // Find and remove device from its current location
        let device = self.take_device(device_id)?;
        let config = device.config().clone();

        // Add to target room
        self.rooms
            .get_mut(room_id)
            .unwrap()
            .add_device(config);

        self.db.assign_device(device_id, room_id)?;
        self.notify_status_changed();
        Ok(())
    }

    pub fn unassign_device(&mut self, device_id: &str) -> Result<()> {
        // Find in rooms and remove
        let mut found_device: Option<Box<dyn DeviceControl>> = None;

        for room in self.rooms.values_mut() {
            if room.has_device(device_id) {
                found_device = room.remove_device(device_id);
                break;
            }
        }

        let device = found_device
            .ok_or_else(|| anyhow!("Device not found in any room: {}", device_id))?;

        let config = device.config().clone();
        let new_device = devices::create_device(config);
        self.unassigned_devices.insert(device_id.to_string(), new_device);
        self.db.unassign_device(device_id)?;
        self.notify_status_changed();
        Ok(())
    }

    fn take_device(&mut self, device_id: &str) -> Result<Box<dyn DeviceControl>> {
        // Try unassigned first
        if let Some(device) = self.unassigned_devices.remove(device_id) {
            return Ok(device);
        }

        // Try rooms
        for room in self.rooms.values_mut() {
            if let Some(device) = room.remove_device(device_id) {
                return Ok(device);
            }
        }

        Err(anyhow!("Device not found: {}", device_id))
    }

    // --- Room-scoped device control ---

    pub async fn set_device_volume(&mut self, room_id: &str, id: &str, volume: u8) -> Result<()> {
        let room = self.rooms.get_mut(room_id)
            .ok_or_else(|| anyhow!("Room not found: {}", room_id))?;
        room.set_device_volume(id, volume).await
    }

    pub async fn set_device_mute(&mut self, room_id: &str, id: &str, muted: bool) -> Result<()> {
        let room = self.rooms.get_mut(room_id)
            .ok_or_else(|| anyhow!("Room not found: {}", room_id))?;
        room.set_device_mute(id, muted).await
    }

    pub async fn set_device_enabled(&mut self, room_id: &str, id: &str, enabled: bool) -> Result<()> {
        let room = self.rooms.get_mut(room_id)
            .ok_or_else(|| anyhow!("Room not found: {}", room_id))?;
        room.set_device_enabled(id, enabled).await
    }

    pub async fn set_room_master_volume(&mut self, room_id: &str, volume: u8) -> Result<()> {
        let room = self.rooms.get_mut(room_id)
            .ok_or_else(|| anyhow!("Room not found: {}", room_id))?;
        room.set_master_volume(volume).await
    }

    // --- Legacy device control (operates on default room) ---

    pub async fn legacy_set_device_volume(&mut self, id: &str, volume: u8) -> Result<()> {
        // Try default room first, then all rooms
        if let Some(room_id) = &self.default_room_id.clone() {
            if let Some(room) = self.rooms.get_mut(room_id) {
                if room.has_device(id) {
                    return room.set_device_volume(id, volume).await;
                }
            }
        }
        for room in self.rooms.values_mut() {
            if room.has_device(id) {
                return room.set_device_volume(id, volume).await;
            }
        }
        Err(anyhow!("Device not found: {}", id))
    }

    pub async fn legacy_set_device_mute(&mut self, id: &str, muted: bool) -> Result<()> {
        if let Some(room_id) = &self.default_room_id.clone() {
            if let Some(room) = self.rooms.get_mut(room_id) {
                if room.has_device(id) {
                    return room.set_device_mute(id, muted).await;
                }
            }
        }
        for room in self.rooms.values_mut() {
            if room.has_device(id) {
                return room.set_device_mute(id, muted).await;
            }
        }
        Err(anyhow!("Device not found: {}", id))
    }

    pub async fn legacy_set_device_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        if let Some(room_id) = &self.default_room_id.clone() {
            if let Some(room) = self.rooms.get_mut(room_id) {
                if room.has_device(id) {
                    return room.set_device_enabled(id, enabled).await;
                }
            }
        }
        for room in self.rooms.values_mut() {
            if room.has_device(id) {
                return room.set_device_enabled(id, enabled).await;
            }
        }
        Err(anyhow!("Device not found: {}", id))
    }

    pub async fn legacy_set_master_volume(&mut self, volume: u8) -> Result<()> {
        if let Some(room_id) = &self.default_room_id.clone() {
            if let Some(room) = self.rooms.get_mut(room_id) {
                return room.set_master_volume(volume).await;
            }
        }
        Err(anyhow!("No default room"))
    }

    // --- Status ---

    pub async fn get_system_status(&self) -> SystemStatus {
        let mut rooms = Vec::new();
        for room in self.rooms.values() {
            rooms.push(room.get_status().await);
        }
        // Sort: default first, then alphabetical
        rooms.sort_by(|a, b| b.is_default.cmp(&a.is_default).then(a.name.cmp(&b.name)));

        let unassigned: Vec<DeviceInfo> = self
            .unassigned_devices
            .values()
            .map(|d| DeviceInfo::from_config_and_state(d.config(), d.state()))
            .collect();

        SystemStatus {
            http_port: self.config.http_port,
            rooms,
            unassigned_devices: unassigned,
        }
    }

    pub async fn get_room_status_async(&self, room_id: &str) -> Option<RoomStatus> {
        if let Some(room) = self.rooms.get(room_id) {
            Some(room.get_status().await)
        } else {
            None
        }
    }

    /// Legacy status from default room for backward compatibility.
    pub async fn get_status(&self) -> MultiplexerStatus {
        if let Some(room_id) = &self.default_room_id {
            if let Some(room) = self.rooms.get(room_id) {
                let rs = room.get_status().await;
                return MultiplexerStatus {
                    receiver_running: rs.receiver_running,
                    receiver_name: rs.name.clone(),
                    streaming: rs.streaming,
                    metadata: rs.metadata,
                    devices: rs.devices,
                    http_port: self.config.http_port,
                };
            }
        }

        MultiplexerStatus {
            receiver_running: false,
            receiver_name: self.config.receiver_name.clone(),
            streaming: false,
            metadata: TrackMetadata::default(),
            devices: vec![],
            http_port: self.config.http_port,
        }
    }

    pub fn get_room_stream_manager(&self, room_id: &str) -> Option<Arc<StreamManager>> {
        self.rooms.get(room_id).map(|r| r.stream_manager().clone())
    }

    pub fn get_default_stream_manager(&self) -> Option<Arc<StreamManager>> {
        self.default_room_id
            .as_ref()
            .and_then(|id| self.get_room_stream_manager(id))
    }

    pub fn room_ids(&self) -> Vec<RoomId> {
        self.rooms.keys().cloned().collect()
    }

    fn notify_status_changed(&self) {
        let _ = self.status_tx.send(());
    }
}
