use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::{mpsc, watch};

use crate::config::AppConfig;
use crate::devices::{self, DeviceControl};
use crate::discovery::{DeviceDiscovery, DiscoveryEvent};
use crate::shairport::{ShairportEvent, ShairportManager};
use crate::stream_manager::StreamManager;
use crate::types::{DeviceConfig, DeviceInfo, MultiplexerStatus, TrackMetadata};

pub struct AudioMultiplexer {
    config: AppConfig,
    stream_manager: Arc<StreamManager>,
    shairport: ShairportManager,
    discovery: DeviceDiscovery,
    devices: HashMap<String, Box<dyn DeviceControl>>,
    metadata: TrackMetadata,
    streaming: bool,
    audio_stream_url: String,
    status_tx: watch::Sender<()>,
}

impl AudioMultiplexer {
    pub fn new(
        config: AppConfig,
        stream_manager: Arc<StreamManager>,
        status_tx: watch::Sender<()>,
    ) -> Self {
        let audio_stream_url = format!(
            "http://{}:{}/audio/stream",
            config.local_ip, config.http_port
        );

        let shairport = ShairportManager::new(&config.shairport_path, &config.receiver_name);
        let discovery = DeviceDiscovery::new();

        Self {
            config,
            stream_manager,
            shairport,
            discovery,
            devices: HashMap::new(),
            metadata: TrackMetadata::default(),
            streaming: false,
            audio_stream_url,
            status_tx,
        }
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub async fn start(&mut self) {
        tracing::info!("[multiplexer] Starting Audio Multiplexer...");
        tracing::info!("[multiplexer] Receiver name: {}", self.config.receiver_name);
        tracing::info!("[multiplexer] HTTP port: {}", self.config.http_port);
        tracing::info!("[multiplexer] Audio stream URL: {}", self.audio_stream_url);

        // Start device discovery
        let (discovery_tx, discovery_rx) = mpsc::channel(32);
        self.discovery.start(discovery_tx);
        self.spawn_discovery_handler(discovery_rx);

        // Start shairport-sync
        let audio_tx = self.stream_manager.sender();
        let (event_tx, event_rx) = mpsc::channel(32);

        match self.shairport.start(audio_tx, event_tx).await {
            Ok(()) => {
                tracing::info!("[multiplexer] Shairport-sync started successfully");
                self.spawn_shairport_handler(event_rx);
            }
            Err(e) => {
                tracing::warn!("[multiplexer] Shairport-sync not available: {}", e);
                tracing::warn!(
                    "[multiplexer] Running without AirPlay receiver - use audio stream endpoint for testing"
                );
            }
        }

        self.notify_status_changed();
    }

    pub async fn stop(&mut self) {
        tracing::info!("[multiplexer] Stopping Audio Multiplexer...");

        self.stop_all_device_audio().await;

        for (_, device) in self.devices.iter_mut() {
            let _ = device.disconnect().await;
        }
        self.devices.clear();

        self.shairport.stop().await;
        self.discovery.stop().await;

        tracing::info!("[multiplexer] Stopped");
    }

    fn spawn_discovery_handler(&self, mut rx: mpsc::Receiver<DiscoveryEvent>) {
        let status_tx = self.status_tx.clone();
        // We can't directly modify self from a spawned task, so we use
        // a channel to send discovered devices back to the multiplexer.
        // But since the multiplexer is behind Arc<RwLock>, the main loop
        // handles adding devices. We'll use a separate approach:
        // store a reference to a shared device-add channel.
        //
        // For simplicity in this architecture, we expose the discovery events
        // externally and process them in the main run loop.
        // But to keep the same EventEmitter-like pattern, we just log here.
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    DiscoveryEvent::DeviceFound(config) => {
                        tracing::info!(
                            "[multiplexer] Discovered device: {} ({})",
                            config.name,
                            config.device_type
                        );
                        // Status notification will be triggered when addDevice is called
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

    fn spawn_shairport_handler(&self, mut rx: mpsc::Receiver<ShairportEvent>) {
        let status_tx = self.status_tx.clone();
        let stream_manager = self.stream_manager.clone();

        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    ShairportEvent::Started => {
                        tracing::info!("[multiplexer] Audio stream started from shairport-sync");
                        stream_manager.set_streaming(true);
                        let _ = status_tx.send(());
                    }
                    ShairportEvent::Stopped => {
                        stream_manager.set_streaming(false);
                        let _ = status_tx.send(());
                    }
                    ShairportEvent::Metadata(meta) => {
                        tracing::info!(
                            "[multiplexer] Metadata: {:?}/{:?}/{:?}",
                            meta.title,
                            meta.artist,
                            meta.album
                        );
                        let _ = status_tx.send(());
                    }
                    ShairportEvent::Error(msg) => {
                        tracing::error!("[multiplexer] Shairport error: {}", msg);
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
        self.devices.insert(id.clone(), device);
        tracing::info!("[multiplexer] Added device: {}", name);
        self.notify_status_changed();
    }

    pub fn remove_device(&mut self, id: &str) {
        if let Some(device) = self.devices.remove(id) {
            tracing::info!("[multiplexer] Removed device: {}", device.config().name);
            self.notify_status_changed();
        }
    }

    async fn stop_all_device_audio(&mut self) {
        for (_, device) in self.devices.iter_mut() {
            let _ = device.stop_audio().await;
        }
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
        for (_, device) in self.devices.iter_mut() {
            let _ = device.set_volume(volume).await;
        }
        self.notify_status_changed();
        Ok(())
    }

    pub fn get_status(&self) -> MultiplexerStatus {
        let device_list: Vec<DeviceInfo> = self
            .devices
            .values()
            .map(|device| DeviceInfo::from_config_and_state(device.config(), device.state()))
            .collect();

        MultiplexerStatus {
            receiver_running: self.shairport.is_running(),
            receiver_name: self.config.receiver_name.clone(),
            streaming: self.streaming || self.stream_manager.is_streaming(),
            metadata: self.metadata.clone(),
            devices: device_list,
            http_port: self.config.http_port,
        }
    }

    fn notify_status_changed(&self) {
        let _ = self.status_tx.send(());
    }
}
