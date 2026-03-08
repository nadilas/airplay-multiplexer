use anyhow::Result;
use async_trait::async_trait;

use crate::types::{DeviceConfig, DeviceState, DeviceType};

pub mod airplay;
pub mod sonos;
pub mod teufel;

#[async_trait]
pub trait DeviceControl: Send + Sync {
    fn config(&self) -> &DeviceConfig;
    fn state(&self) -> &DeviceState;

    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn start_audio(&mut self, stream_url: &str) -> Result<()>;
    async fn stop_audio(&mut self) -> Result<()>;

    async fn set_volume(&mut self, volume: u8) -> Result<()>;
    async fn set_mute(&mut self, muted: bool) -> Result<()>;
    fn set_enabled(&mut self, enabled: bool);
}

/// Factory: create the right device implementation from config.
pub fn create_device(config: DeviceConfig) -> Box<dyn DeviceControl> {
    match config.device_type {
        DeviceType::Sonos => Box::new(sonos::SonosDevice::new(config)),
        DeviceType::Teufel => Box::new(teufel::TeufelDevice::new(config)),
        DeviceType::Airplay => Box::new(airplay::AirplayDevice::new(config)),
    }
}
