use async_trait::async_trait;
use crate::store::DashItem;

pub mod rss;
pub mod github;

/// Pull 型ソースの共通インターフェース
#[async_trait]
pub trait Connector: Send + Sync {
    fn source_id(&self) -> &str;
    async fn fetch(&self) -> Result<Vec<DashItem>, ConnectorError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectorError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Feed parse error: {0}")]
    Parse(String),
}
