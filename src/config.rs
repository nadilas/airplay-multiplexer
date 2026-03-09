use crate::types::AudioFormat;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub receiver_name: String,
    pub http_port: u16,
    pub audio_format: AudioFormat,
    pub shairport_path: String,
    pub local_ip: String,
    pub db_path: String,
    pub shairport_base_port: u16,
}

pub fn load_config() -> AppConfig {
    AppConfig {
        receiver_name: std::env::var("RECEIVER_NAME")
            .unwrap_or_else(|_| "Multi-Room Audio".to_string()),
        http_port: std::env::var("HTTP_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5000),
        audio_format: AudioFormat {
            sample_rate: std::env::var("SAMPLE_RATE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(44100),
            bit_depth: std::env::var("BIT_DEPTH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(16),
            channels: std::env::var("CHANNELS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(2),
        },
        shairport_path: std::env::var("SHAIRPORT_PATH")
            .unwrap_or_else(|_| "shairport-sync".to_string()),
        local_ip: std::env::var("LOCAL_IP").unwrap_or_else(|_| get_local_ip()),
        db_path: std::env::var("DB_PATH")
            .unwrap_or_else(|_| "audio_multiplexer.db".to_string()),
        shairport_base_port: std::env::var("SHAIRPORT_BASE_PORT")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(5100),
    }
}

fn get_local_ip() -> String {
    if_addrs::get_if_addrs()
        .ok()
        .and_then(|addrs| {
            addrs.into_iter().find_map(|iface| {
                if !iface.is_loopback() {
                    if let std::net::IpAddr::V4(ipv4) = iface.ip() {
                        return Some(ipv4.to_string());
                    }
                }
                None
            })
        })
        .unwrap_or_else(|| "127.0.0.1".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn clear_env() {
        for key in [
            "RECEIVER_NAME",
            "HTTP_PORT",
            "SAMPLE_RATE",
            "BIT_DEPTH",
            "CHANNELS",
            "SHAIRPORT_PATH",
            "LOCAL_IP",
            "DB_PATH",
            "SHAIRPORT_BASE_PORT",
        ] {
            std::env::remove_var(key);
        }
    }

    #[test]
    #[serial]
    fn test_defaults() {
        clear_env();
        let config = load_config();
        assert_eq!(config.receiver_name, "Multi-Room Audio");
        assert_eq!(config.http_port, 5000);
        assert_eq!(config.audio_format.sample_rate, 44100);
        assert_eq!(config.audio_format.bit_depth, 16);
        assert_eq!(config.audio_format.channels, 2);
        assert_eq!(config.shairport_path, "shairport-sync");
        assert_eq!(config.db_path, "audio_multiplexer.db");
        assert_eq!(config.shairport_base_port, 5100);
    }

    #[test]
    #[serial]
    fn test_receiver_name_from_env() {
        clear_env();
        std::env::set_var("RECEIVER_NAME", "My Audio Hub");
        let config = load_config();
        assert_eq!(config.receiver_name, "My Audio Hub");
    }

    #[test]
    #[serial]
    fn test_http_port_from_env() {
        clear_env();
        std::env::set_var("HTTP_PORT", "8080");
        let config = load_config();
        assert_eq!(config.http_port, 8080);
    }

    #[test]
    #[serial]
    fn test_audio_format_from_env() {
        clear_env();
        std::env::set_var("SAMPLE_RATE", "48000");
        std::env::set_var("BIT_DEPTH", "24");
        std::env::set_var("CHANNELS", "1");
        let config = load_config();
        assert_eq!(config.audio_format.sample_rate, 48000);
        assert_eq!(config.audio_format.bit_depth, 24);
        assert_eq!(config.audio_format.channels, 1);
    }

    #[test]
    #[serial]
    fn test_shairport_path_from_env() {
        clear_env();
        std::env::set_var("SHAIRPORT_PATH", "/usr/local/bin/shairport-sync");
        let config = load_config();
        assert_eq!(config.shairport_path, "/usr/local/bin/shairport-sync");
    }

    #[test]
    #[serial]
    fn test_local_ip_from_env() {
        clear_env();
        std::env::set_var("LOCAL_IP", "10.0.0.5");
        let config = load_config();
        assert_eq!(config.local_ip, "10.0.0.5");
    }

    #[test]
    #[serial]
    fn test_db_path_from_env() {
        clear_env();
        std::env::set_var("DB_PATH", "/data/rooms.db");
        let config = load_config();
        assert_eq!(config.db_path, "/data/rooms.db");
    }

    #[test]
    #[serial]
    fn test_shairport_base_port_from_env() {
        clear_env();
        std::env::set_var("SHAIRPORT_BASE_PORT", "6000");
        let config = load_config();
        assert_eq!(config.shairport_base_port, 6000);
    }
}
