use anyhow::Result;
use async_trait::async_trait;
use std::time::Duration;

use crate::devices::DeviceControl;
use crate::types::{DeviceConfig, DeviceState};
use crate::upnp;

pub struct SonosDevice {
    config: DeviceConfig,
    state: DeviceState,
    client: reqwest::Client,
    control_url: Option<String>,
    rendering_url: Option<String>,
}

impl SonosDevice {
    pub fn new(config: DeviceConfig) -> Self {
        Self {
            config,
            state: DeviceState::default(),
            client: reqwest::Client::new(),
            control_url: None,
            rendering_url: None,
        }
    }

    fn build_control_urls(&mut self) {
        let base = format!("http://{}:{}", self.config.host, self.config.port);
        self.control_url = Some(format!("{}/MediaRenderer/AVTransport/Control", base));
        self.rendering_url = Some(format!(
            "{}/MediaRenderer/RenderingControl/Control",
            base
        ));
    }
}

#[async_trait]
impl DeviceControl for SonosDevice {
    fn config(&self) -> &DeviceConfig {
        &self.config
    }
    fn state(&self) -> &DeviceState {
        &self.state
    }

    async fn connect(&mut self) -> Result<()> {
        self.build_control_urls();

        let url = format!(
            "http://{}:{}/status/zp",
            self.config.host, self.config.port
        );
        let resp = self
            .client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if resp.status().is_success() {
            self.state.connected = true;
            tracing::info!("[sonos] Connected to {}", self.config.name);
        } else {
            anyhow::bail!("Sonos device returned status {}", resp.status());
        }
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        let _ = self.stop_audio().await;
        self.state.connected = false;
        self.state.playing = false;
        tracing::info!("[sonos] Disconnected from {}", self.config.name);
        Ok(())
    }

    async fn start_audio(&mut self, stream_url: &str) -> Result<()> {
        if !self.state.connected || !self.state.enabled {
            return Ok(());
        }

        let control = self
            .control_url
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No control URL"))?
            .clone();
        upnp::set_av_transport_and_play(&self.client, &control, stream_url).await?;
        self.state.playing = true;
        tracing::info!("[sonos] {} started playing from {}", self.config.name, stream_url);
        Ok(())
    }

    async fn stop_audio(&mut self) -> Result<()> {
        if !self.state.connected {
            return Ok(());
        }
        if let Some(control) = &self.control_url {
            let control = control.clone();
            upnp::call_action(
                &self.client,
                &control,
                upnp::AV_TRANSPORT,
                "Stop",
                &[("InstanceID", "0")],
            )
            .await?;
        }
        self.state.playing = false;
        tracing::info!("[sonos] {} stopped", self.config.name);
        Ok(())
    }

    async fn set_volume(&mut self, volume: u8) -> Result<()> {
        self.state.volume = volume.min(100);
        if !self.state.connected {
            return Ok(());
        }
        if let Some(rendering) = &self.rendering_url {
            let rendering = rendering.clone();
            let vol_str = self.state.volume.to_string();
            upnp::call_action(
                &self.client,
                &rendering,
                upnp::RENDERING_CONTROL,
                "SetVolume",
                &[
                    ("InstanceID", "0"),
                    ("Channel", "Master"),
                    ("DesiredVolume", &vol_str),
                ],
            )
            .await?;
        }
        Ok(())
    }

    async fn set_mute(&mut self, muted: bool) -> Result<()> {
        self.state.muted = muted;
        if !self.state.connected {
            return Ok(());
        }
        if let Some(rendering) = &self.rendering_url {
            let rendering = rendering.clone();
            let mute_str = if muted { "1" } else { "0" };
            upnp::call_action(
                &self.client,
                &rendering,
                upnp::RENDERING_CONTROL,
                "SetMute",
                &[
                    ("InstanceID", "0"),
                    ("Channel", "Master"),
                    ("DesiredMute", mute_str),
                ],
            )
            .await?;
        }
        Ok(())
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.state.enabled = enabled;
    }
}
