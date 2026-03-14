use async_trait::async_trait;
use chrono::{Local, TimeZone};
use feed_rs::parser;

use crate::config::FeedConfig;
use crate::store::DashItem;
use super::{Connector, ConnectorError};

pub struct RssConnector {
    config: FeedConfig,
    client: reqwest::Client,
}

impl RssConnector {
    pub fn new(config: FeedConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Connector for RssConnector {
    fn source_id(&self) -> &str {
        &self.config.id
    }

    async fn fetch(&self) -> Result<Vec<DashItem>, ConnectorError> {
        let bytes = self
            .client
            .get(&self.config.url)
            .send()
            .await?
            .bytes()
            .await?;

        let feed = parser::parse(bytes.as_ref())
            .map_err(|e| ConnectorError::Parse(e.to_string()))?;

        let today = Local::now().date_naive();

        let items = feed
            .entries
            .into_iter()
            .filter_map(|entry| {
                let published_at = entry
                    .published
                    .or(entry.updated)
                    .map(|dt| Local.from_utc_datetime(&dt.naive_utc()))
                    .unwrap_or_else(Local::now);

                // 当日分のみ
                if published_at.date_naive() != today {
                    return None;
                }

                let id = entry.id;
                let title = entry.title.map(|t| t.content).unwrap_or_default();
                let url = entry.links.into_iter().next().map(|l| l.href);
                let body = entry
                    .summary
                    .map(|s| s.content)
                    .or_else(|| entry.content.and_then(|c| c.body));

                Some(DashItem {
                    id,
                    source_id: self.config.id.clone(),
                    source_name: self.config.name.clone(),
                    title,
                    body,
                    url,
                    published_at,
                    tags: vec![],
                })
            })
            .collect();

        Ok(items)
    }
}
