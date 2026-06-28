use crate::config::NetworkConfig;
use crate::output::{emit, emit_error, OutputFormat};
use anyhow::{anyhow, Context};
use serde_json::json;
use xlm_ns_sdk::client::XlmNsClient;

pub async fn run_resolve(
    config: NetworkConfig,
    output: OutputFormat,
    name: &str,
) -> anyhow::Result<()> {
    let client = XlmNsClient::new(
        config.rpc_url,
        Some(config.network_passphrase),
        config.registry_contract_id.clone(),
        config.subdomain_contract_id.clone(),
        config.bridge_contract_id.clone(),
        config.auction_contract_id.clone(),
    );

    let result = client
        .resolve(name)
        .await
        .context("Failed to resolve name")?;

    if let Some(addr) = result.address {
        println!("Name: {}", result.name);
        println!("Address: {}", addr);
        if let Some(resolver) = result.resolver {
            println!("Resolver: {}", resolver);
        }
        println!(
            "Resolved via wildcard: {}",
            if result.is_wildcard { "yes" } else { "no" }
        );
        if let Some(expiry) = result.expires_at {
            println!("Expires at: {}", expiry);
        }
        Ok(())
    } else {
        let message = format!("Name '{}' not found or has no resolution", name);
        emit_error(
            output,
            &message,
            json!({"error": message.clone(), "name": name}),
        );
        Err(anyhow!(message))
    }
}
