use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub sources: SourcesConfig,
    #[serde(default)]
    pub windows: WindowsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub auth_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    pub default_capacity: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            default_capacity: 50,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourcesConfig {
    pub rss: Option<RssSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssSourceConfig {
    pub poll_interval_secs: u64,
    pub feeds: Vec<FeedConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsConfig {
    pub startup: bool,
    pub notifications: bool,
    pub tray_icon: bool,
    pub start_hidden: bool,
}

impl Default for WindowsConfig {
    fn default() -> Self {
        Self {
            startup: false,
            notifications: true,
            tray_icon: true,
            start_hidden: false,
        }
    }
}

impl Config {
    pub fn load(path: &str) -> Result<Self, crate::error::AppError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default() -> Self {
        Self::load("Config.toml").unwrap_or_else(|_| Config::default_config())
    }

    fn default_config() -> Self {
        Config {
            server: ServerConfig {
                port: 8080,
                auth_token: "change-me".to_string(),
            },
            memory: MemoryConfig::default(),
            sources: SourcesConfig::default(),
            windows: WindowsConfig::default(),
        }
    }
}
