use serde::{Deserialize, Serialize};

/// Top-level configuration parsed from `interconnect.toml`.
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Config {
    #[serde(default)]
    pub room: Vec<RoomConfig>,
}

/// Configuration for a single room.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RoomConfig {
    /// Logical name used to identify this room in CLI commands.
    pub name: String,
    /// Connector type (e.g. "slack", "sqlite", "discord").
    pub connector: String,
    /// All remaining fields are passed through to the connector as-is.
    #[serde(flatten)]
    pub options: serde_json::Value,
}

impl Config {
    /// Load configuration from a TOML file at the given path.
    pub fn load(path: &std::path::Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::Io(path.display().to_string(), e))?;
        toml::from_str(&text).map_err(ConfigError::Parse)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {0}: {1}")]
    Io(String, std::io::Error),
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
}
