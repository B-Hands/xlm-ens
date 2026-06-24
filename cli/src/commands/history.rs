use crate::config::NetworkConfig;
use crate::output::OutputFormat;
use anyhow::{anyhow, Context};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use xlm_ns_common::validation::validate_account_address;
use xlm_ns_sdk::client::XlmNsClient;

/// Event type classification for xlm-ns transactions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    Register,
    Renew,
    Transfer,
    Bid,
    AuctionWin,
    AuctionClaim,
    AuctionCancel,
    ResolverUpdate,
    MetadataUpdate,
    Unknown,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Register => "register",
            Self::Renew => "renew",
            Self::Transfer => "transfer",
            Self::Bid => "bid",
            Self::AuctionWin => "auction_win",
            Self::AuctionClaim => "auction_claim",
            Self::AuctionCancel => "auction_cancel",
            Self::ResolverUpdate => "resolver_update",
            Self::MetadataUpdate => "metadata_update",
            Self::Unknown => "unknown",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Register => "REGISTER",
            Self::Renew => "RENEW",
            Self::Transfer => "TRANSFER",
            Self::Bid => "BID",
            Self::AuctionWin => "AUCTION WIN",
            Self::AuctionClaim => "AUCTION CLAIM",
            Self::AuctionCancel => "AUCTION CANCEL",
            Self::ResolverUpdate => "RESOLVER UPDATE",
            Self::MetadataUpdate => "METADATA UPDATE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Normalized representation of a historical xlm-ns event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    pub timestamp: String,
    pub ledger: u32,
    pub tx_hash: String,
    #[serde(rename = "type")]
    pub event_type: EventType,
    pub name: Option<String>,
    pub owner: Option<String>,
    pub previous_owner: Option<String>,
    pub counterparty: Option<String>,
    pub amount: Option<String>,
    pub fee: Option<String>,
    pub contract_id: String,
    pub explorer_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy)]
pub struct HistoryOptions {
    pub limit: usize,
    pub no_cache: bool,
}

impl Default for HistoryOptions {
    fn default() -> Self {
        Self {
            limit: 50,
            no_cache: false,
        }
    }
}

impl HistoryOptions {
    pub fn normalized(mut self) -> Self {
        // Clamp limit to 1000 max
        if self.limit > 1000 {
            self.limit = 1000;
        }
        // Ensure at least 1
        if self.limit == 0 {
            self.limit = 1;
        }
        self
    }
}

/// Caching layer for history queries
struct HistoryCache {
    cache_dir: PathBuf,
}

impl HistoryCache {
    fn new() -> anyhow::Result<Self> {
        let cache_dir = Self::get_cache_dir()?;
        fs::create_dir_all(&cache_dir).context("Failed to create cache directory")?;
        Ok(Self { cache_dir })
    }

    fn get_cache_dir() -> anyhow::Result<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
                return Ok(PathBuf::from(local_app_data).join("xlm-ns").join("cache"));
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return Ok(PathBuf::from(home)
                    .join("Library")
                    .join("Caches")
                    .join("xlm-ns"));
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Ok(cache_home) = std::env::var("XDG_CACHE_HOME") {
                return Ok(PathBuf::from(cache_home).join("xlm-ns"));
            }
            if let Ok(home) = std::env::var("HOME") {
                return Ok(PathBuf::from(home).join(".cache").join("xlm-ns"));
            }
        }

        // Fallback to temp directory
        Ok(std::env::temp_dir().join("xlm-ns-cache"))
    }

    fn make_cache_key(address: Option<&str>, name: Option<&str>, limit: usize) -> String {
        let addr_part = address.unwrap_or("all");
        let name_part = name.unwrap_or("all");
        format!("history:{}:{}:{}", addr_part, name_part, limit)
    }

    fn get_cache_path(&self, key: &str) -> PathBuf {
        // Use a safe filename based on the key
        let safe_key = key
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        self.cache_dir.join(format!("{}.json", safe_key))
    }

    fn is_cache_valid(path: &PathBuf) -> bool {
        if !path.exists() {
            return false;
        }

        if let Ok(metadata) = fs::metadata(path) {
            if let Ok(modified) = metadata.modified() {
                if let Ok(elapsed) = modified.elapsed() {
                    // Cache valid for 5 minutes
                    return elapsed.as_secs() < 300;
                }
            }
        }
        false
    }

    fn read(
        &self,
        address: Option<&str>,
        name: Option<&str>,
        limit: usize,
    ) -> anyhow::Result<Option<Vec<HistoryEvent>>> {
        let key = Self::make_cache_key(address, name, limit);
        let path = self.get_cache_path(&key);

        if !Self::is_cache_valid(&path) {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).context("Failed to read cache file")?;
        let events = serde_json::from_str(&content).context("Failed to parse cached events")?;
        Ok(Some(events))
    }

    fn write(
        &self,
        address: Option<&str>,
        name: Option<&str>,
        limit: usize,
        events: &[HistoryEvent],
    ) -> anyhow::Result<()> {
        let key = Self::make_cache_key(address, name, limit);
        let path = self.get_cache_path(&key);

        let content = serde_json::to_string(events).context("Failed to serialize events")?;
        fs::write(&path, content).context("Failed to write cache file")?;
        Ok(())
    }
}

/// Provider interface for fetching history events
#[async_trait::async_trait]
trait HistoryProvider {
    async fn get_address_history(
        &self,
        address: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<HistoryEvent>>;

    async fn get_name_history(&self, name: &str, limit: usize)
        -> anyhow::Result<Vec<HistoryEvent>>;
}

/// Soroban-based history provider using stellar-rpc-client
struct SorobanHistoryProvider {
    client: XlmNsClient,
    network: String,
}

impl SorobanHistoryProvider {
    fn new(client: XlmNsClient, network: &str) -> Self {
        Self {
            client,
            network: network.to_string(),
        }
    }

    fn build_explorer_url(&self, tx_hash: &str) -> String {
        let network_name = if self.network == "testnet" {
            "testnet"
        } else {
            "public"
        };
        format!(
            "https://stellar.expert/explorer/{}/tx/{}",
            network_name, tx_hash
        )
    }

    fn parse_timestamp(ledger_close_time: Option<i64>) -> String {
        if let Some(timestamp) = ledger_close_time {
            // Stellar timestamps are Unix epoch seconds
            let system_time =
                SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64);
            let datetime = chrono::DateTime::<Utc>::from(system_time);
            return datetime.to_rfc3339();
        }
        Utc::now().to_rfc3339()
    }

    /// Classify event type from contract ID and event data
    fn classify_event(&self, contract_id: &str, _event_data: &serde_json::Value) -> EventType {
        // Classify based on contract_id patterns
        // In a real implementation, we would parse event topics and data
        // For now, return Unknown as placeholder
        EventType::Unknown
    }

    /// Create sample events for demonstration
    /// In production, this would fetch from Soroban RPC and parse real events
    fn create_sample_events(&self, _address: &str, _limit: usize) -> Vec<HistoryEvent> {
        // Placeholder: Return empty vector
        // Real implementation will use stellar-rpc-client to fetch events
        // from soroban_rpcEventGetEventsByContractID or soroban_rpcEventGetEventsByTopics
        vec![]
    }
}

#[async_trait::async_trait]
impl HistoryProvider for SorobanHistoryProvider {
    async fn get_address_history(
        &self,
        address: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<HistoryEvent>> {
        // TODO: Query Soroban RPC for contract invocations by this address
        // This requires:
        // 1. Use stellar-rpc-client to query events
        // 2. Filter for xlm-ns contract IDs
        // 3. Parse contract events and classify by type
        // 4. Return normalized HistoryEvent objects

        // For MVP, return sample events
        Ok(self.create_sample_events(address, limit))
    }

    async fn get_name_history(
        &self,
        name: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<HistoryEvent>> {
        // TODO: Similar to address_history but filters by domain name
        // Would need to query events and parse domain names from event data

        // For MVP, return sample events
        Ok(self.create_sample_events(name, limit))
    }
}

/// Format events for human-readable output
fn format_human_output(events: &[HistoryEvent]) -> String {
    if events.is_empty() {
        return "No xlm-ns activity found for this address.".to_string();
    }

    let mut output = String::new();
    output.push_str("Name History\n");
    output.push_str(&"=".repeat(80));
    output.push('\n');

    for event in events {
        output.push('\n');
        output.push_str(&format!("{}\n", event.timestamp));
        output.push_str(&format!("{}\n", event.event_type.display_name()));

        if let Some(name) = &event.name {
            output.push_str(&format!("Name: {}\n", name));
        }
        if let Some(owner) = &event.owner {
            output.push_str(&format!("Owner: {}\n", owner));
        }
        if let Some(prev_owner) = &event.previous_owner {
            output.push_str(&format!("Previous Owner: {}\n", prev_owner));
        }
        if let Some(counterparty) = &event.counterparty {
            output.push_str(&format!("Counterparty: {}\n", counterparty));
        }
        if let Some(amount) = &event.amount {
            output.push_str(&format!("Amount: {} XLM\n", amount));
        }
        if let Some(fee) = &event.fee {
            output.push_str(&format!("Fee: {} XLM\n", fee));
        }

        output.push_str(&format!(
            "Tx: {}\n",
            &event.tx_hash[..event.tx_hash.len().min(12)]
        ));
        output.push_str(&format!("Ledger: {}\n", event.ledger));
        output.push_str(&"-".repeat(80));
    }

    output
}

pub async fn run_history(
    config: NetworkConfig,
    address: Option<&str>,
    name: Option<&str>,
    limit: usize,
    no_cache: bool,
    output_format: OutputFormat,
) -> anyhow::Result<()> {
    // Validate address if provided
    if let Some(addr) = address {
        validate_account_address(addr).map_err(|e| anyhow!("Invalid Stellar address: {}", e))?;
    }

    // Ensure we have either address or name
    if address.is_none() && name.is_none() {
        return Err(anyhow!("Either <address> or --name must be provided"));
    }

    let options = HistoryOptions { limit, no_cache }.normalized();

    // Try cache first
    let events = if !options.no_cache {
        let cache = HistoryCache::new().ok();
        if let Some(cache) = cache {
            if let Ok(Some(cached)) = cache.read(address, name, options.limit) {
                cached
            } else {
                // Fetch from provider
                let provider = SorobanHistoryProvider::new(
                    XlmNsClient::new(
                        config.rpc_url.clone(),
                        Some(config.network_passphrase.clone()),
                        config.registry_contract_id.clone(),
                        config.subdomain_contract_id.clone(),
                        config.bridge_contract_id.clone(),
                        config.auction_contract_id.clone(),
                    ),
                    config.network.as_str(),
                );

                let events = if let Some(addr) = address {
                    provider.get_address_history(addr, options.limit).await?
                } else if let Some(n) = name {
                    provider.get_name_history(n, options.limit).await?
                } else {
                    vec![]
                };

                // Cache the results
                let _ = cache.write(address, name, options.limit, &events);
                events
            }
        } else {
            // Fetch from provider without caching
            let provider = SorobanHistoryProvider::new(
                XlmNsClient::new(
                    config.rpc_url.clone(),
                    Some(config.network_passphrase.clone()),
                    config.registry_contract_id.clone(),
                    config.subdomain_contract_id.clone(),
                    config.bridge_contract_id.clone(),
                    config.auction_contract_id.clone(),
                ),
                config.network.as_str(),
            );

            if let Some(addr) = address {
                provider.get_address_history(addr, options.limit).await?
            } else if let Some(n) = name {
                provider.get_name_history(n, options.limit).await?
            } else {
                vec![]
            }
        }
    } else {
        // Fetch from provider without caching
        let provider = SorobanHistoryProvider::new(
            XlmNsClient::new(
                config.rpc_url.clone(),
                Some(config.network_passphrase.clone()),
                config.registry_contract_id.clone(),
                config.subdomain_contract_id.clone(),
                config.bridge_contract_id.clone(),
                config.auction_contract_id.clone(),
            ),
            config.network.as_str(),
        );

        if let Some(addr) = address {
            provider.get_address_history(addr, options.limit).await?
        } else if let Some(n) = name {
            provider.get_name_history(n, options.limit).await?
        } else {
            vec![]
        }
    };

    // Output results
    match output_format {
        OutputFormat::Human => {
            println!("{}", format_human_output(&events));
        }
        OutputFormat::Json => {
            let json =
                serde_json::to_value(&events).context("Failed to serialize events to JSON")?;
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        OutputFormat::Csv => {
            // For CSV, output header then rows
            if !events.is_empty() {
                println!(
                    "timestamp,type,name,owner,counterparty,amount,fee,tx_hash,ledger,explorer_url"
                );
                for event in &events {
                    println!(
                        "{},{},{},{},{},{},{},{},{},{}",
                        event.timestamp,
                        event.event_type,
                        event.name.as_deref().unwrap_or(""),
                        event.owner.as_deref().unwrap_or(""),
                        event.counterparty.as_deref().unwrap_or(""),
                        event.amount.as_deref().unwrap_or(""),
                        event.fee.as_deref().unwrap_or(""),
                        event.tx_hash,
                        event.ledger,
                        event.explorer_url,
                    );
                }
            }
        }
    }

    Ok(())
}
