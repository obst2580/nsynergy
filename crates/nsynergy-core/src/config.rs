use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};

/// Which side of the screen a neighbor machine is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScreenPosition {
    Left,
    Right,
    Top,
    Bottom,
}

/// Describes a neighbor machine in the screen layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Neighbor {
    /// Display name of the machine
    pub name: String,
    /// Which edge of *this* screen leads to the neighbor
    pub position: ScreenPosition,
    /// Optional fixed IP; if None, mDNS discovery is used
    pub address: Option<Ipv4Addr>,
}

/// Runtime role of this instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Role {
    #[default]
    Server,
    Client,
}

/// Top-level application configuration, persisted as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Display name of this machine
    pub machine_name: String,
    /// Current role
    pub role: Role,
    /// UDP port for input events
    pub udp_port: u16,
    /// TCP port for clipboard / large data
    pub tcp_port: u16,
    /// Neighboring machines
    pub neighbors: Vec<Neighbor>,
    /// Number of pixels at the screen edge that trigger transition
    pub edge_threshold: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            machine_name: hostname(),
            role: Role::default(),
            udp_port: 24800,
            tcp_port: 24801,
            neighbors: Vec::new(),
            edge_threshold: 2,
        }
    }
}

impl AppConfig {
    /// Loads config from a JSON file, falling back to defaults if
    /// the file does not exist.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let config: Self = serde_json::from_str(&contents)
            .with_context(|| format!("parsing {}", path.display()))?;
        Ok(config)
    }

    /// Saves config to a JSON file, creating parent directories if needed.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(self)
            .with_context(|| "serializing config to JSON")?;
        std::fs::write(path, json).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Returns the default config file path for the current platform.
    ///
    /// - macOS: `~/Library/Application Support/nsynergy/config.json`
    /// - Windows: `%APPDATA%\nsynergy\config.json`
    /// - Linux: `~/.config/nsynergy/config.json`
    pub fn default_path() -> PathBuf {
        let base = dirs_base();
        base.join("config.json")
    }
}

/// Best-effort hostname; falls back to "unknown".
fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("COMPUTERNAME"))
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Platform-specific config directory.
fn dirs_base() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("nsynergy")
    }
    #[cfg(target_os = "windows")]
    {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "C:\\".to_string());
        PathBuf::from(appdata).join("nsynergy")
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(".config").join("nsynergy")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn default_config_has_sensible_values() {
        let config = AppConfig::default();
        assert_eq!(config.role, Role::Server);
        assert_eq!(config.udp_port, 24800);
        assert_eq!(config.tcp_port, 24801);
        assert!(config.neighbors.is_empty());
        assert_eq!(config.edge_threshold, 2);
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test_config.json");

        let config = AppConfig {
            machine_name: "test-machine".to_string(),
            role: Role::Client,
            udp_port: 9000,
            tcp_port: 9001,
            neighbors: vec![Neighbor {
                name: "server-pc".to_string(),
                position: ScreenPosition::Left,
                address: Some(Ipv4Addr::new(192, 168, 1, 100)),
            }],
            edge_threshold: 5,
        };

        config.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(config, loaded);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let path = Path::new("/tmp/nsynergy_nonexistent_test_config.json");
        let config = AppConfig::load(path).unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.json");
        std::fs::write(&path, "not valid json {{{").unwrap();
        let result = AppConfig::load(&path);
        assert!(result.is_err());
    }

    #[test]
    fn json_serialization_format() {
        let config = AppConfig::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        assert!(json.contains("machine_name"));
        assert!(json.contains("udp_port"));
        assert!(json.contains("tcp_port"));
        assert!(json.contains("neighbors"));
    }

    #[test]
    fn screen_position_serialization() {
        let positions = vec![
            ScreenPosition::Left,
            ScreenPosition::Right,
            ScreenPosition::Top,
            ScreenPosition::Bottom,
        ];
        for pos in positions {
            let json = serde_json::to_string(&pos).unwrap();
            let deserialized: ScreenPosition = serde_json::from_str(&json).unwrap();
            assert_eq!(pos, deserialized);
        }
    }

    #[test]
    fn default_path_is_not_empty() {
        let path = AppConfig::default_path();
        assert!(!path.as_os_str().is_empty());
        assert!(path.ends_with("config.json"));
    }
}
