use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Config load failed: {0}")]
    Config(#[from] toml::de::Error),

    #[error("Config file not found: {0}")]
    ConfigIo(#[from] std::io::Error),

    #[error("Connector error [{source_id}]: {message}")]
    Connector { source_id: String, message: String },

    #[error("Auth token mismatch")]
    Unauthorized,

    #[error("Validation error: {0}")]
    Validation(String),
}

impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
