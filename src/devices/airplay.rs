use anyhow::Result;
use async_trait::async_trait;

use crate::devices::DeviceControl;
use crate::types::{DeviceConfig, DeviceState};

/// AirPlay output device — HTTP fallback mode only.
///
/// Without an RAOP library (airtunes equivalent), AirPlay devices are
/// discovered and tracked but audio is delivered via the HTTP stream URL.
pub struct AirplayDevice {
    config: DeviceConfig,
    state: DeviceState,
}

impl AirplayDevice {
    pub fn new(config: DeviceConfig) -> Self {
        tracing::info!(
            "[airplay] {} will use HTTP streaming fallback",
            config.name
        );
        Self {
            config,
            state: DeviceState::default(),
        }
    }
}

#[async_trait]
impl DeviceControl for AirplayDevice {
    fn config(&self) -> &DeviceConfig {
        &self.config
    }
    fn state(&self) -> &DeviceState {
        &self.state
    }

    async fn connect(&mut self) -> Result<()> {
        self.state.connected = true;
        tracing::info!("[airplay] Connected to {} (HTTP mode)", self.config.name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        let _ = self.stop_audio().await;
        self.state.connected = false;
        self.state.playing = false;
        tracing::info!("[airplay] Disconnected from {}", self.config.name);
        Ok(())
    }

    async fn start_audio(&mut self, _stream_url: &str) -> Result<()> {
        if !self.state.connected || !self.state.enabled {
            return Ok(());
        }
        self.state.playing = true;
        tracing::info!("[airplay] {} started audio", self.config.name);
        Ok(())
    }

    async fn stop_audio(&mut self) -> Result<()> {
        self.state.playing = false;
        tracing::info!("[airplay] {} stopped audio", self.config.name);
        Ok(())
    }

    async fn set_volume(&mut self, volume: u8) -> Result<()> {
        self.state.volume = volume.min(100);
        Ok(())
    }

    async fn set_mute(&mut self, muted: bool) -> Result<()> {
        self.state.muted = muted;
        Ok(())
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.state.enabled = enabled;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DeviceType;

    fn test_config() -> DeviceConfig {
        DeviceConfig {
            id: "airplay-1".to_string(),
            name: "AirPlay Speaker".to_string(),
            host: "192.168.1.60".to_string(),
            port: 7000,
            device_type: DeviceType::Airplay,
            location: None,
            model: None,
        }
    }

    #[test]
    fn test_new_device() {
        let device = AirplayDevice::new(test_config());
        assert_eq!(device.config().id, "airplay-1");
        assert_eq!(device.config().name, "AirPlay Speaker");
    }

    #[test]
    fn test_default_state() {
        let device = AirplayDevice::new(test_config());
        let state = device.state();
        assert!(!state.connected);
        assert!(!state.playing);
        assert_eq!(state.volume, 50);
        assert!(state.enabled);
        assert!(!state.muted);
    }

    #[tokio::test]
    async fn test_connect() {
        let mut device = AirplayDevice::new(test_config());
        device.connect().await.unwrap();
        assert!(device.state().connected);
    }

    #[tokio::test]
    async fn test_disconnect() {
        let mut device = AirplayDevice::new(test_config());
        device.connect().await.unwrap();
        device.disconnect().await.unwrap();
        assert!(!device.state().connected);
        assert!(!device.state().playing);
    }

    #[tokio::test]
    async fn test_start_audio_when_connected() {
        let mut device = AirplayDevice::new(test_config());
        device.connect().await.unwrap();
        device.start_audio("http://example.com/stream").await.unwrap();
        assert!(device.state().playing);
    }

    #[tokio::test]
    async fn test_start_audio_noop_when_not_connected() {
        let mut device = AirplayDevice::new(test_config());
        device.start_audio("http://example.com/stream").await.unwrap();
        assert!(!device.state().playing);
    }

    #[tokio::test]
    async fn test_start_audio_noop_when_disabled() {
        let mut device = AirplayDevice::new(test_config());
        device.connect().await.unwrap();
        device.set_enabled(false);
        device.start_audio("http://example.com/stream").await.unwrap();
        assert!(!device.state().playing);
    }

    #[tokio::test]
    async fn test_stop_audio() {
        let mut device = AirplayDevice::new(test_config());
        device.connect().await.unwrap();
        device.start_audio("http://example.com/stream").await.unwrap();
        device.stop_audio().await.unwrap();
        assert!(!device.state().playing);
    }

    #[tokio::test]
    async fn test_set_volume() {
        let mut device = AirplayDevice::new(test_config());
        device.set_volume(75).await.unwrap();
        assert_eq!(device.state().volume, 75);
    }

    #[tokio::test]
    async fn test_set_volume_clamps() {
        let mut device = AirplayDevice::new(test_config());
        device.set_volume(150).await.unwrap();
        assert_eq!(device.state().volume, 100);
    }

    #[tokio::test]
    async fn test_set_mute() {
        let mut device = AirplayDevice::new(test_config());
        device.set_mute(true).await.unwrap();
        assert!(device.state().muted);
        device.set_mute(false).await.unwrap();
        assert!(!device.state().muted);
    }

    #[test]
    fn test_set_enabled() {
        let mut device = AirplayDevice::new(test_config());
        device.set_enabled(false);
        assert!(!device.state().enabled);
        device.set_enabled(true);
        assert!(device.state().enabled);
    }
}
