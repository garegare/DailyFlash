use async_trait::async_trait;
use chrono::{DateTime, Duration, Local, TimeZone, Utc};
use serde::Deserialize;

use crate::config::GithubSourceConfig;
use crate::store::DashItem;
use super::{Connector, ConnectorError};

pub struct GithubConnector {
    config: GithubSourceConfig,
    client: reqwest::Client,
}

impl GithubConnector {
    pub fn new(config: GithubSourceConfig) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("DailyFlash/0.1")
            .build()
            .unwrap_or_default();
        Self { config, client }
    }
}

// GitHub Events API レスポンス型
#[derive(Debug, Deserialize)]
struct GhEvent {
    id: String,
    #[serde(rename = "type")]
    event_type: Option<String>,
    repo: GhRepo,
    payload: serde_json::Value,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct GhRepo {
    name: String, // "owner/repo"
}

#[async_trait]
impl Connector for GithubConnector {
    fn source_id(&self) -> &str {
        "github"
    }

    async fn fetch(&self) -> Result<Vec<DashItem>, ConnectorError> {
        let url = format!(
            "https://api.github.com/users/{}/events?per_page=100",
            self.config.username
        );

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.config.token))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?
            .error_for_status()
            .map_err(|e| ConnectorError::Http(e))?
            .json::<Vec<GhEvent>>()
            .await?;

        let cutoff = (Local::now() - Duration::days(self.config.lookback_days as i64))
            .date_naive();

        let items = resp
            .into_iter()
            .filter_map(|ev| {
                let published_at = Local.from_utc_datetime(&ev.created_at.naive_utc());
                if published_at.date_naive() < cutoff {
                    return None;
                }

                let event_type = ev.event_type.as_deref().unwrap_or("UnknownEvent");
                let repo_name = &ev.repo.name;
                let repo_url = format!("https://github.com/{}", repo_name);

                let (title, url, tags) = match event_type {
                    "PushEvent" => {
                        let commits = ev.payload["commits"]
                            .as_array()
                            .map(|c| c.len())
                            .unwrap_or(0);
                        let branch = ev.payload["ref"]
                            .as_str()
                            .unwrap_or("")
                            .trim_start_matches("refs/heads/");
                        let title = format!("Push: {} ({} commits, {})", repo_name, commits, branch);
                        let url = format!("{}/commits/{}", repo_url, branch);
                        (title, url, vec!["push".to_string(), repo_name.clone()])
                    }
                    "PullRequestEvent" => {
                        let action = ev.payload["action"].as_str().unwrap_or("updated");
                        let pr = &ev.payload["pull_request"];
                        let num = pr["number"].as_u64().unwrap_or(0);
                        let pr_title = pr["title"].as_str().unwrap_or("").to_string();
                        let pr_url = pr["html_url"].as_str().unwrap_or(&repo_url).to_string();
                        let title = format!("PR #{} {}: {}", num, action, pr_title);
                        (title, pr_url, vec!["pull_request".to_string(), repo_name.clone()])
                    }
                    "IssuesEvent" => {
                        let action = ev.payload["action"].as_str().unwrap_or("updated");
                        let issue = &ev.payload["issue"];
                        let num = issue["number"].as_u64().unwrap_or(0);
                        let issue_title = issue["title"].as_str().unwrap_or("").to_string();
                        let issue_url = issue["html_url"].as_str().unwrap_or(&repo_url).to_string();
                        let title = format!("Issue #{} {}: {}", num, action, issue_title);
                        (title, issue_url, vec!["issue".to_string(), repo_name.clone()])
                    }
                    "IssueCommentEvent" => {
                        let issue = &ev.payload["issue"];
                        let num = issue["number"].as_u64().unwrap_or(0);
                        let issue_title = issue["title"].as_str().unwrap_or("").to_string();
                        let comment_url = ev.payload["comment"]["html_url"]
                            .as_str()
                            .unwrap_or(&repo_url)
                            .to_string();
                        let title = format!("Comment on #{}: {}", num, issue_title);
                        (title, comment_url, vec!["comment".to_string(), repo_name.clone()])
                    }
                    "CreateEvent" => {
                        let ref_type = ev.payload["ref_type"].as_str().unwrap_or("ref");
                        let ref_name = ev.payload["ref"].as_str().unwrap_or("");
                        let title = if ref_name.is_empty() {
                            format!("Created {} in {}", ref_type, repo_name)
                        } else {
                            format!("Created {} '{}' in {}", ref_type, ref_name, repo_name)
                        };
                        (title, repo_url, vec!["create".to_string(), repo_name.clone()])
                    }
                    "DeleteEvent" => {
                        let ref_type = ev.payload["ref_type"].as_str().unwrap_or("ref");
                        let ref_name = ev.payload["ref"].as_str().unwrap_or("");
                        let title = format!("Deleted {} '{}' in {}", ref_type, ref_name, repo_name);
                        (title, repo_url, vec!["delete".to_string(), repo_name.clone()])
                    }
                    "WatchEvent" => {
                        let title = format!("Starred {}", repo_name);
                        (title, repo_url, vec!["star".to_string(), repo_name.clone()])
                    }
                    "ForkEvent" => {
                        let forkee = ev.payload["forkee"]["full_name"]
                            .as_str()
                            .unwrap_or(repo_name);
                        let fork_url = format!("https://github.com/{}", forkee);
                        let title = format!("Forked {} → {}", repo_name, forkee);
                        (title, fork_url, vec!["fork".to_string(), repo_name.clone()])
                    }
                    other => {
                        let title = format!("{} in {}", other, repo_name);
                        (title, repo_url, vec![other.to_lowercase(), repo_name.clone()])
                    }
                };

                Some(DashItem {
                    id: ev.id,
                    source_id: "github".to_string(),
                    source_name: "GitHub".to_string(),
                    title,
                    body: None,
                    url: Some(url),
                    published_at,
                    tags,
                })
            })
            .collect();

        Ok(items)
    }
}
