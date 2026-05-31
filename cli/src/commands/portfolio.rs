use crate::config::NetworkConfig;
use crate::export;
use crate::output::{emit_error, OutputFormat};
use serde_json::json;
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::types::{RegistryEntry, ResolutionResult};

async fn build_portfolio_records(
    client: &XlmNsClient,
    names: &[ResolutionResult],
    now_unix: i64,
) -> anyhow::Result<Vec<export::PortfolioRecord>> {
    let mut records = Vec::new();

    for entry in names {
        let metadata = client.get_registry_metadata(&entry.name).await?;
        let record = RegistryEntry {
            name: entry.name.clone(),
            owner: metadata.owner,
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

pub async fn run_portfolio(config: NetworkConfig, output: OutputFormat, owner: &str) -> anyhow::Result<()> {
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

pub async fn run_portfolio(
    config: NetworkConfig,
    output: OutputFormat,
    owner: &str,
) -> anyhow::Result<()> {
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

    match client.list_registrations_by_owner(owner) {
        Ok(names) => {
            match output {
                OutputFormat::Human => {
                    let mut lines = vec![format!("Portfolio for {owner}:")];
                    if names.is_empty() {
                        lines.push("  [no names found]".to_string());
                    } else {
                        for entry in &names {
                            let expires = entry
                                .expires_at
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "unknown".to_string());
                            lines.push(format!("  - {} (expires_at: {expires})", entry.name));
                        }
                    }

                    println!("{}", lines.join("\n"));
                }
                OutputFormat::Json => {
                    let records = build_portfolio_records(&client, &names, now_unix).await?;
                    export::write_json(&records, &mut std::io::stdout())
                        .map_err(anyhow::Error::msg)?;
                }
                OutputFormat::Csv => {
                    let records = build_portfolio_records(&client, &names, now_unix).await?;
                    export::write_csv(&records, &mut std::io::stdout())
                        .map_err(anyhow::Error::msg)?;
                }
            }
        }
        Err(err) => {
            let message = format!("ERROR: Failed to fetch portfolio for {owner}: {err}");
            emit_error(
                output,
                &message,
                json!({
                    "error": message,
                    "owner": owner,
                    "registry_contract_id": config.registry_contract_id,
                    "rpc_url": config.rpc_url,
                    "network": config.network.as_str(),
                }),
            );
        }
    }
    Ok(())
}
