use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub emulator: EmulatorConfig,
    pub input: InputConfig,
    pub server: ServerConfig,
    pub stream: StreamConfig,
    pub chat: ChatConfig,
}

#[derive(Debug, Deserialize)]
pub struct EmulatorConfig {
    pub bios_path: String,
    pub rom_path: String,
    pub save_dir: String,
    pub target_fps: u32,
}

#[derive(Debug, Deserialize)]
pub struct InputConfig {
    pub default_mode: String,
    pub democracy_window_secs: u64,
    pub rate_limit_ms: u64,
    pub mode_switch_threshold: f64,
    pub mode_switch_cooldown_secs: u64,
    pub start_throttle_secs: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub ws_host: String,
    pub ws_port: u16,
    pub admin_port: u16,
    pub admin_token: String,
}

#[derive(Debug, Deserialize)]
pub struct StreamConfig {
    pub jpeg_quality: u8,
    pub audio_buffer_ms: u64,
}

#[derive(Debug, Deserialize)]
pub struct ChatConfig {
    pub streamplace_ws_url: String,
    pub streamplace_token: String,
}

impl Config {
    pub fn from_toml_str(s: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(s)
    }

    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_CONFIG: &str = r#"
        [emulator]
        bios_path = "/tmp/gba_bios.bin"
        rom_path = "/tmp/test.gba"
        save_dir = "/tmp/saves/"
        target_fps = 60

        [input]
        default_mode = "anarchy"
        democracy_window_secs = 10
        rate_limit_ms = 200
        mode_switch_threshold = 0.75
        mode_switch_cooldown_secs = 300

        [server]
        ws_host = "127.0.0.1"
        ws_port = 9001
        admin_port = 9002
        admin_token = "test-token"

        [stream]
        jpeg_quality = 85
        audio_buffer_ms = 100

        [chat]
        streamplace_ws_url = "wss://example.com"
        streamplace_token = "chat-token"
    "#;

    #[test]
    fn test_config_loads_from_toml_string() {
        let config = Config::from_toml_str(SAMPLE_CONFIG).expect("config should parse");
        assert_eq!(config.emulator.bios_path, "/tmp/gba_bios.bin");
        assert_eq!(config.emulator.target_fps, 60);
        assert_eq!(config.emulator.rom_path, "/tmp/test.gba");
        assert_eq!(config.emulator.save_dir, "/tmp/saves/");
    }

    #[test]
    fn test_config_stream_fields() {
        let config = Config::from_toml_str(SAMPLE_CONFIG).expect("config should parse");
        assert_eq!(config.stream.jpeg_quality, 85);
        assert_eq!(config.stream.audio_buffer_ms, 100);
    }

    #[test]
    fn test_config_input_fields() {
        let config = Config::from_toml_str(SAMPLE_CONFIG).expect("config should parse");
        assert_eq!(config.input.default_mode, "anarchy");
        assert_eq!(config.input.democracy_window_secs, 10);
        assert_eq!(config.input.rate_limit_ms, 200);
        assert!((config.input.mode_switch_threshold - 0.75).abs() < f64::EPSILON);
        assert_eq!(config.input.mode_switch_cooldown_secs, 300);
        assert!(config.input.start_throttle_secs.is_none());
    }

    #[test]
    fn test_config_server_fields() {
        let config = Config::from_toml_str(SAMPLE_CONFIG).expect("config should parse");
        assert_eq!(config.server.ws_host, "127.0.0.1");
        assert_eq!(config.server.ws_port, 9001);
        assert_eq!(config.server.admin_port, 9002);
        assert_eq!(config.server.admin_token, "test-token");
    }

    #[test]
    fn test_config_optional_start_throttle() {
        let with_throttle = r#"
            [emulator]
            bios_path = "/tmp/gba_bios.bin"
            rom_path = "/tmp/test.gba"
            save_dir = "/tmp/saves/"
            target_fps = 60
            [input]
            default_mode = "anarchy"
            democracy_window_secs = 10
            rate_limit_ms = 200
            mode_switch_threshold = 0.75
            mode_switch_cooldown_secs = 300
            start_throttle_secs = 5
            [server]
            ws_host = "127.0.0.1"
            ws_port = 9001
            admin_port = 9002
            admin_token = "tok"
            [stream]
            jpeg_quality = 85
            audio_buffer_ms = 100
            [chat]
            streamplace_ws_url = "wss://example.com"
            streamplace_token = "tok"
        "#;
        let config = Config::from_toml_str(with_throttle).expect("config should parse");
        assert_eq!(config.input.start_throttle_secs, Some(5));
    }

    #[test]
    fn test_config_rejects_missing_required_fields() {
        let bad = r#"
            [emulator]
            rom_path = "/tmp/test.gba"
        "#;
        assert!(Config::from_toml_str(bad).is_err());
    }
}
