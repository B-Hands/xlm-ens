use crate::config::NetworkConfig;
use chrono::{DateTime, Duration, Utc};
use clap::Subcommand;
use colored::*;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use xlm_ns_sdk::client::XlmNsClient;

#[derive(Subcommand, Clone)]
pub enum WatchCommand {
    /// Add a name to the watchlist
    Add {
        /// Name to watch
        name: String,
    },
    /// Remove a name from the watchlist
    Remove {
        /// Name to remove
        name: String,
    },
    /// List all watched names
    List,
    /// Check the status of all watched names
    Check {
        /// Optional webhook URL for notifications
        #[arg(long)]
        webhook_url: Option<String>,
        /// Output in a cron-friendly format
        #[arg(long)]
        cron: bool,
    },
}

#[derive(Serialize, Deserialize, Debug, Default)]
struct Watchlist {
    names: Vec<String>,
}

fn get_watchlist_path() -> anyhow::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("xlm-ns");
    std::fs::create_dir_all(&config_dir)?;
    Ok(config_dir.join("watchlist.json"))
}

fn load_watchlist() -> anyhow::Result<Watchlist> {
    let path = get_watchlist_path()?;
    if !path.exists() {
        return Ok(Watchlist::default());
    }
    let content = fs::read_to_string(path)?;
    let watchlist = serde_json::from_str(&content)?;
    Ok(watchlist)
}

fn save_watchlist(watchlist: &Watchlist) -> anyhow::Result<()> {
    let path = get_watchlist_path()?;
    let content = serde_json::to_string_pretty(watchlist)?;
    fs::write(path, content)?;
    Ok(())
}

enum ExpiryStatus {
    Safe(DateTime<Utc>),
    ExpiringSoon(DateTime<Utc>),
    InGracePeriod(DateTime<Utc>),
    Expired,
    NotRegistered,
}

impl ExpiryStatus {
    fn from_expires_at(expires_at: Option<DateTime<Utc>>) -> Self {
        let now = Utc::now();
        match expires_at {
            Some(expiry) => {
                if now > expiry + Duration::days(90) {
                    Self::Expired
                } else if now > expiry {
                    Self::InGracePeriod(expiry)
                } else if now > expiry - Duration::days(30) {
                    Self::ExpiringSoon(expiry)
                } else {
                    Self::Safe(expiry)
                }
            }
            None => Self::NotRegistered,
        }
    }
}

pub async fn run(config: NetworkConfig, command: WatchCommand) -> anyhow::Result<()> {
    match command {
        WatchCommand::Add { name } => {
            let mut watchlist = load_watchlist()?;
            if !watchlist.names.contains(&name) {
                watchlist.names.push(name.clone());
                save_watchlist(&watchlist)?;
                println!("Added '{}' to the watchlist.", name);
            } else {
                println!("'{}' is already in the watchlist.", name);
            }
        }
        WatchCommand::Remove { name } => {
            let mut watchlist = load_watchlist()?;
            if let Some(pos) = watchlist.names.iter().position(|n| n == &name) {
                watchlist.names.remove(pos);
                save_watchlist(&watchlist)?;
                println!("Removed '{}' from the watchlist.", name);
            } else {
                println!("'{}' is not in the watchlist.", name);
            }
        }
        WatchCommand::List => {
            let watchlist = load_watchlist()?;
            if watchlist.names.is_empty() {
                println!("The watchlist is empty.");
            } else {
                println!("Watched names:");
                for name in watchlist.names {
                    println!("- {}", name);
                }
            }
        }
        WatchCommand::Check { webhook_url, cron } => {
            let watchlist = load_watchlist()?;
            if watchlist.names.is_empty() {
                if !cron {
                    println!("The watchlist is empty.");
                }
                return Ok(());
            }

            let client = XlmNsClient::new(
                config.rpc_url.clone(),
                Some(config.network_passphrase.clone()),
                config.registry_contract_id.clone(),
                config.subdomain_contract_id.clone(),
                config.bridge_contract_id.clone(),
                config.auction_contract_id.clone(),
            );

            let mut notifications = Vec::new();

            for name in &watchlist.names {
                let registration = client.get_registration(name).await?;
                let status = ExpiryStatus::from_expires_at(registration.and_then(|r| r.expires_at));

                let (status_str, color) = match status {
                    ExpiryStatus::Safe(expiry) => (
                        format!("Safe (expires on {})", expiry.format("%Y-%m-%d")),
                        "green",
                    ),
                    ExpiryStatus::ExpiringSoon(expiry) => (
                        format!("Expiring soon (expires on {})", expiry.format("%Y-%m-%d")),
                        "yellow",
                    ),
                    ExpiryStatus::InGracePeriod(expiry) => (
                        format!(
                            "In grace period (expired on {})",
                            expiry.format("%Y-%m-%d")
                        ),
                        "red",
                    ),
                    ExpiryStatus::Expired => ("Expired and available".to_string(), "bright-black"),
                    ExpiryStatus::NotRegistered => ("Not registered".to_string(), "bright-black"),
                };

                let notification = if cron {
                    format!("{}: {}", name, status_str)
                } else {
                    format!("{}: {}", name, status_str.color(color))
                };

                notifications.push(notification.clone());
                if !cron {
                    println!("{}", notification);
                }
            }

            if let Some(url) = webhook_url {
                let client = reqwest::Client::new();
                let payload = serde_json::json!({
                    "text": notifications.join("\n"),
                });
                client.post(&url).json(&payload).send().await?;
            }
        }
    }
    Ok(())
}
