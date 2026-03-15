use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub display: DisplayConfig,
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

/// 表示設定
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DisplayConfig {
    /// ハイライト表示するキーワード一覧（大文字小文字無視）
    #[serde(default)]
    pub highlight_keywords: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SourcesConfig {
    pub rss: Option<RssSourceConfig>,
    pub github: Option<GithubSourceConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RssSourceConfig {
    pub poll_interval_secs: u64,
    /// 当日を含む過去 N 日分を表示対象とする (0 = 当日のみ)
    #[serde(default = "default_lookback_days")]
    pub lookback_days: u32,
    pub feeds: Vec<FeedConfig>,
}

fn default_lookback_days() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedConfig {
    pub id: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// フィード個別の lookback_days。未設定なら RssSourceConfig の値を使う
    #[serde(default)]
    pub lookback_days: Option<u32>,
}

/// GitHub コネクタ設定
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubSourceConfig {
    /// GitHub Personal Access Token
    pub token: String,
    /// 取得対象のユーザー名
    pub username: String,
    pub poll_interval_secs: u64,
    #[serde(default = "default_lookback_days")]
    pub lookback_days: u32,
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
        let candidates = Self::config_candidates();
        for path in &candidates {
            if path.exists() {
                match Self::load(&path.to_string_lossy()) {
                    Ok(c) => {
                        eprintln!("[config] loaded {}", path.display());
                        return c;
                    }
                    Err(e) => eprintln!("[config] parse error {}: {e}", path.display()),
                }
            }
        }
        eprintln!("[config] Config.toml not found, using defaults");
        Config::default_config()
    }

    fn config_candidates() -> Vec<std::path::PathBuf> {
        let mut candidates = vec![
            std::path::PathBuf::from("Config.toml"),
            std::path::PathBuf::from("../Config.toml"),
        ];
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("Config.toml"));
                if let Some(parent) = dir.parent() {
                    candidates.push(parent.join("Config.toml"));
                }
            }
        }
        candidates
    }

    fn default_config() -> Self {
        Config {
            server: ServerConfig {
                port: 8080,
                auth_token: "change-me".to_string(),
            },
            memory: MemoryConfig::default(),
            display: DisplayConfig::default(),
            sources: SourcesConfig::default(),
            windows: WindowsConfig::default(),
        }
    }
}
