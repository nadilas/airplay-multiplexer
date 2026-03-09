use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::{mpsc, watch, RwLock};

use crate::devices::{self, DeviceControl};
use crate::shairport::{ShairportEvent, ShairportManager};
use crate::stream_manager::StreamManager;
use crate::types::{
    DeviceConfig, DeviceInfo, RoomConfig, RoomId, RoomStatus, TrackMetadata,
};

pub struct Room {
    id: RoomId,
    name: String,
    receiver_name: String,
    is_default: bool,
    master_volume: u8,
    stream_manager: Arc<StreamManager>,
    shairport: ShairportManager,
    devices: HashMap<String, Box<dyn DeviceControl>>,
    metadata: Arc<RwLock<TrackMetadata>>,
    streaming: bool,
    audio_stream_url: String,
    status_tx: watch::Sender<()>,
}

impl Room {
    pub fn new(
        config: &RoomConfig,
        shairport_path: &str,
        stream_manager: Arc<StreamManager>,
        audio_stream_url: String,
        status_tx: watch::Sender<()>,
    ) -> Self {
        let mut shairport = ShairportManager::new(shairport_path, &config.receiver_name);
        shairport = shairport.with_port(config.shairport_port);

        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            receiver_name: config.receiver_name.clone(),
            is_default: config.is_default,
            master_volume: 50,
            stream_manager,
            shairport,
            devices: HashMap::new(),
            metadata: Arc::new(RwLock::new(TrackMetadata::default())),
            streaming: false,
            audio_stream_url,
            status_tx,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }

    pub fn stream_manager(&self) -> &Arc<StreamManager> {
        &self.stream_manager
    }

    pub async fn start(&mut self) {
        tracing::info!("[room:{}] Starting room '{}'", self.id, self.name);

        let audio_tx = self.stream_manager.sender();
        let (event_tx, event_rx) = mpsc::channel(32);

        match self.shairport.start(audio_tx, event_tx).await {
            Ok(()) => {
                tracing::info!(
                    "[room:{}] Shairport-sync started (receiver: '{}', port: {:?})",
                    self.id,
                    self.receiver_name,
                    self.shairport.port()
                );
                self.spawn_shairport_handler(event_rx);
            }
            Err(e) => {
                tracing::warn!(
                    "[room:{}] Shairport-sync not available: {}",
                    self.id, e
                );
            }
        }

        self.notify_status_changed();
    }

    pub async fn stop(&mut self) {
        tracing::info!("[room:{}] Stopping room '{}'", self.id, self.name);

        for (_, device) in self.devices.iter_mut() {
            let _ = device.stop_audio().await;
            let _ = device.disconnect().await;
        }

        self.shairport.stop().await;
        self.streaming = false;
    }

    fn spawn_shairport_handler(&self, mut rx: mpsc::Receiver<ShairportEvent>) {
        let status_tx = self.status_tx.clone();
        let stream_manager = self.stream_manager.clone();
        let metadata = self.metadata.clone();
        let room_id = self.id.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    ShairportEvent::Started => {
                        tracing::info!("[room:{}] Audio stream started", room_id);
                        stream_manager.set_streaming(true);
                        let _ = status_tx.send(());
                    }
                    ShairportEvent::Stopped => {
                        stream_manager.set_streaming(false);
                        let _ = status_tx.send(());
                    }
                    ShairportEvent::Metadata(meta) => {
                        tracing::info!(
                            "[room:{}] Metadata: {:?}/{:?}/{:?}",
                            room_id,
                            meta.title,
                            meta.artist,
                            meta.album
                        );
                        *metadata.write().await = meta;
                        let _ = status_tx.send(());
                    }
                    ShairportEvent::Error(msg) => {
                        tracing::error!("[room:{}] Shairport error: {}", room_id, msg);
                    }
                }
            }
        });
    }

    pub fn add_device(&mut self, config: DeviceConfig) {
        if self.devices.contains_key(&config.id) {
            return;
        }
        let id = config.id.clone();
        let name = config.name.clone();
        let device = devices::create_device(config);
        self.devices.insert(id, device);
        tracing::info!("[room:{}] Added device: {}", self.id, name);
        self.notify_status_changed();
    }

    pub fn remove_device(&mut self, id: &str) -> Option<Box<dyn DeviceControl>> {
        let device = self.devices.remove(id);
        if device.is_some() {
            tracing::info!("[room:{}] Removed device: {}", self.id, id);
            self.notify_status_changed();
        }
        device
    }

    pub fn has_device(&self, id: &str) -> bool {
        self.devices.contains_key(id)
    }

    pub fn device_ids(&self) -> Vec<String> {
        self.devices.keys().cloned().collect()
    }

    pub async fn set_device_volume(&mut self, id: &str, volume: u8) -> Result<()> {
        let device = self
            .devices
            .get_mut(id)
            .ok_or_else(|| anyhow!("Device not found: {}", id))?;
        device.set_volume(volume).await?;
        self.notify_status_changed();
        Ok(())
    }

    pub async fn set_device_mute(&mut self, id: &str, muted: bool) -> Result<()> {
        let device = self
            .devices
            .get_mut(id)
            .ok_or_else(|| anyhow!("Device not found: {}", id))?;
        device.set_mute(muted).await?;
        self.notify_status_changed();
        Ok(())
    }

    pub async fn set_device_enabled(&mut self, id: &str, enabled: bool) -> Result<()> {
        let device = self
            .devices
            .get_mut(id)
            .ok_or_else(|| anyhow!("Device not found: {}", id))?;
        device.set_enabled(enabled);

        if enabled && self.streaming && device.state().connected {
            let url = self.audio_stream_url.clone();
            let _ = device.start_audio(&url).await;
        } else if !enabled {
            let _ = device.stop_audio().await;
        }

        self.notify_status_changed();
        Ok(())
    }

    pub async fn set_master_volume(&mut self, volume: u8) -> Result<()> {
        self.master_volume = volume;
        for (_, device) in self.devices.iter_mut() {
            let _ = device.set_volume(volume).await;
        }
        self.notify_status_changed();
        Ok(())
    }

    pub async fn get_status(&self) -> RoomStatus {
        let device_list: Vec<DeviceInfo> = self
            .devices
            .values()
            .map(|device| {
                DeviceInfo::from_config_and_state(device.config(), device.state())
                    .with_room_id(Some(self.id.clone()))
            })
            .collect();

        let metadata = self.metadata.read().await.clone();

        RoomStatus {
            id: self.id.clone(),
            name: self.name.clone(),
            receiver_running: self.shairport.is_running(),
            streaming: self.streaming || self.stream_manager.is_streaming(),
            metadata,
            master_volume: self.master_volume,
            devices: device_list,
            is_default: self.is_default,
        }
    }

    fn notify_status_changed(&self) {
        let _ = self.status_tx.send(());
    }
}
