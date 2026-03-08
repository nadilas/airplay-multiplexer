use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Sonos,
    Teufel,
    Airplay,
}

impl std::fmt::Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeviceType::Sonos => write!(f, "sonos"),
            DeviceType::Teufel => write!(f, "teufel"),
            DeviceType::Airplay => write!(f, "airplay"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(rename = "type")]
    pub device_type: DeviceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {
    pub volume: u8,
    pub muted: bool,
    pub enabled: bool,
    pub connected: bool,
    pub playing: bool,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            volume: 50,
            muted: false,
            enabled: true,
            connected: false,
            playing: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub bit_depth: u16,
    pub channels: u16,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            bit_depth: 16,
            channels: 2,
        }
    }
}

impl AudioFormat {
    pub fn byte_rate(&self) -> u32 {
        self.sample_rate * self.channels as u32 * (self.bit_depth as u32 / 8)
    }

    pub fn block_align(&self) -> u16 {
        self.channels * (self.bit_depth / 8)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
}

/// Flattened DeviceConfig + DeviceState for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    #[serde(rename = "type")]
    pub device_type: DeviceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    pub volume: u8,
    pub muted: bool,
    pub enabled: bool,
    pub connected: bool,
    pub playing: bool,
}

impl DeviceInfo {
    pub fn from_config_and_state(config: &DeviceConfig, state: &DeviceState) -> Self {
        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            host: config.host.clone(),
            port: config.port,
            device_type: config.device_type,
            location: config.location.clone(),
            model: config.model.clone(),
            volume: state.volume,
            muted: state.muted,
            enabled: state.enabled,
            connected: state.connected,
            playing: state.playing,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MultiplexerStatus {
    pub receiver_running: bool,
    pub receiver_name: String,
    pub streaming: bool,
    pub metadata: TrackMetadata,
    pub devices: Vec<DeviceInfo>,
    pub http_port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_state_defaults() {
        let state = DeviceState::default();
        assert_eq!(state.volume, 50);
        assert!(!state.muted);
        assert!(state.enabled);
        assert!(!state.connected);
        assert!(!state.playing);
    }

    #[test]
    fn test_audio_format_defaults() {
        let fmt = AudioFormat::default();
        assert_eq!(fmt.sample_rate, 44100);
        assert_eq!(fmt.bit_depth, 16);
        assert_eq!(fmt.channels, 2);
    }

    #[test]
    fn test_audio_format_byte_rate() {
        let fmt = AudioFormat::default();
        // 44100 * 2 * (16/8) = 176400
        assert_eq!(fmt.byte_rate(), 176400);
    }

    #[test]
    fn test_audio_format_block_align() {
        let fmt = AudioFormat::default();
        // 2 * (16/8) = 4
        assert_eq!(fmt.block_align(), 4);
    }

    #[test]
    fn test_device_type_serialization() {
        assert_eq!(serde_json::to_string(&DeviceType::Sonos).unwrap(), "\"sonos\"");
        assert_eq!(serde_json::to_string(&DeviceType::Teufel).unwrap(), "\"teufel\"");
        assert_eq!(serde_json::to_string(&DeviceType::Airplay).unwrap(), "\"airplay\"");
    }

    #[test]
    fn test_multiplexer_status_camel_case() {
        let status = MultiplexerStatus {
            receiver_running: false,
            receiver_name: "Test".to_string(),
            streaming: false,
            metadata: TrackMetadata::default(),
            devices: vec![],
            http_port: 5000,
        };
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("receiverRunning"));
        assert!(json.contains("receiverName"));
        assert!(json.contains("httpPort"));
    }

    #[test]
    fn test_device_info_from_config_and_state() {
        let config = DeviceConfig {
            id: "test-1".to_string(),
            name: "Speaker".to_string(),
            host: "192.168.1.1".to_string(),
            port: 1400,
            device_type: DeviceType::Sonos,
            location: None,
            model: None,
        };
        let state = DeviceState {
            volume: 75,
            muted: true,
            ..DeviceState::default()
        };
        let info = DeviceInfo::from_config_and_state(&config, &state);
        assert_eq!(info.id, "test-1");
        assert_eq!(info.volume, 75);
        assert!(info.muted);
    }

    #[test]
    fn test_track_metadata_skips_none_fields() {
        let meta = TrackMetadata {
            title: Some("Song".to_string()),
            artist: None,
            album: None,
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("title"));
        assert!(!json.contains("artist"));
        assert!(!json.contains("album"));
    }
}
