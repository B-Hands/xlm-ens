use crate::config::NetworkConfig;
use crate::export;
use crate::output::{emit_error, print_human, print_human_err, OutputFormat};
use serde_json::json;
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::errors::SdkError;
use xlm_ns_sdk::types::{RegistryEntry, ResolutionResult};

const DEFAULT_BATCH_SIZE: usize = 50;
const MIN_BATCH_SIZE: usize = 1;

#[derive(Debug, Clone, Copy)]
pub struct PortfolioOptions {
    pub batch_size: usize,
    pub limit: Option<usize>,
    pub page: Option<usize>,
}

impl Default for PortfolioOptions {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
            limit: None,
            page: None,
        }
    }
}

impl PortfolioOptions {
    pub fn normalized(self) -> Self {
        Self {
            batch_size: self.batch_size.max(MIN_BATCH_SIZE),
            limit: self.limit,
            page: self.page,
        }
    }
}

async fn build_portfolio_records(
    client: &XlmNsClient,
    names: &[ResolutionResult],
    now_unix: i64,
) -> anyhow::Result<Vec<export::PortfolioRecord>> {
    let mut records = Vec::new();

    for entry in names {
        let metadata = client.get_registry_metadata(&entry.name).await?;
        let owner = entry
            .address
            .clone()
            .unwrap_or_else(|| metadata.owner.clone());
        let record = RegistryEntry {
            name: entry.name.clone(),
            owner,
            resolver: metadata.resolver.or_else(|| entry.resolver.clone()),
            target_address: entry.address.clone(),
            metadata_uri: None,
            ttl_seconds: 0,
            registered_at: metadata.registered_at,
            expires_at: metadata.expires_at,
            grace_period_ends_at: metadata.grace_period_ends_at,
            transfer_count: 0,
        };
        records.push(export::PortfolioRecord::from_name_record(&record, now_unix));
    }

    Ok(records)
}

fn is_timeout_error(err: &SdkError) -> bool {
    match err {
        SdkError::Transport(message) | SdkError::InvalidRequest(message) => {
            let message = message.to_ascii_lowercase();
            message.contains("timeout") || message.contains("timed out")
        }
        SdkError::TransactionTimeout { .. } => true,
        _ => false,
    }
}

fn progress(output: OutputFormat, fetched: usize, total: usize) {
    if output == OutputFormat::Human {
        print_human_err(&format!("Fetched {fetched}/{total} names..."));
    }
}

fn print_human_batch(owner: &str, names: &[ResolutionResult], printed_header: &mut bool) {
    if !*printed_header {
        print_human(&format!("Portfolio for {owner}:"));
        *printed_header = true;
    }

    for entry in names {
        let expires = entry
            .expires_at
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        print_human(&format!("  - {} (expires_at: {expires})", entry.name));
    }
}

pub async fn run_portfolio(
    config: NetworkConfig,
    output: OutputFormat,
    owner: &str,
    options: PortfolioOptions,
) -> anyhow::Result<()> {
    let options = options.normalized();
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    let client = XlmNsClient::new(
        config.rpc_url.clone(),
        Some(config.network_passphrase.clone()),
        config.registry_contract_id.clone(),
        config.subdomain_contract_id.clone(),
        config.bridge_contract_id.clone(),
        config.auction_contract_id.clone(),
    )
    .with_resolver(
        config
            .resolver_contract_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    );

    let mut cursor = options
        .page
        .map(|page| page.saturating_sub(1) * options.batch_size);
    let mut page_size = options.batch_size;
    let mut fetched = 0usize;
    let mut total_seen = None;
    let mut names = Vec::new();
    let mut printed_header = false;

    loop {
        let remaining_limit = options.limit.map(|limit| limit.saturating_sub(fetched));
        if remaining_limit == Some(0) {
            break;
        }
        let page = loop {
            let requested = remaining_limit.map_or(page_size, |remaining| remaining.min(page_size));
            match client.list_registrations_by_owner_page(owner, cursor, requested) {
                Ok(page) => break page,
                Err(err) if is_timeout_error(&err) && page_size > MIN_BATCH_SIZE => {
                    page_size = (page_size / 2).max(MIN_BATCH_SIZE);
                    print_human_err(&format!(
                        "RPC timed out while fetching portfolio; retrying with batch size {page_size}"
                    ));
                    continue;
                }
                Err(err) => {
                    let message = format!("ERROR: Failed to fetch portfolio for {owner}: {err}");
                    return Err(anyhow::anyhow!(message));
                }
            }
        };

        total_seen = Some(page.total);
        if output == OutputFormat::Human {
            print_human_batch(owner, &page.items, &mut printed_header);
        }
        fetched += page.items.len();
        names.extend(page.items);
        progress(output, fetched, page.total);

        if options.page.is_some() || cursor == page.next_cursor || page.next_cursor.is_none() {
            break;
        }
        cursor = page.next_cursor;
    }

    if names.is_empty() && output == OutputFormat::Human {
        print_human(&format!("Portfolio for {owner}:\n  [no names found]"));
        if let Some(total) = total_seen {
            progress(output, 0, total);
        }
    }

    match output {
        OutputFormat::Human => {}
        OutputFormat::Json => {
            let records = build_portfolio_records(&client, &names, now_unix).await?;
            export::write_json(&records, &mut std::io::stdout()).map_err(anyhow::Error::msg)?;
        }
        OutputFormat::Csv => {
            let records = build_portfolio_records(&client, &names, now_unix).await?;
            export::write_csv(&records, &mut std::io::stdout()).map_err(anyhow::Error::msg)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portfolio_options_clamps_zero_batch_size() {
        let options = PortfolioOptions {
            batch_size: 0,
            limit: Some(10),
            page: Some(1),
        }
        .normalized();

        assert_eq!(options.batch_size, 1);
        assert_eq!(options.limit, Some(10));
        assert_eq!(options.page, Some(1));
    }

    #[test]
    fn detects_timeout_errors_for_retry() {
        assert!(is_timeout_error(&SdkError::Transport(
            "request timed out".to_string()
        )));
        assert!(!is_timeout_error(&SdkError::InvalidRequest(
            "owner must not be empty".to_string()
        )));
    }
}
