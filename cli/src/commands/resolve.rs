use crate::config::NetworkConfig;
use crate::output::{emit, emit_error, with_spinner, OutputFormat};
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

    let result = with_spinner(
        format!("Resolving {name}"),
        output,
        client.resolve(name),
    )
    .await
    .context("Failed to resolve name")?;

    if let Some(addr) = result.address {
        crate::output::print_human(&format!("Name: {}\nAddress: {}", result.name, addr));
        if let Some(resolver) = result.resolver {
            crate::output::print_human(&format!("Resolver: {}", resolver));
        }
        crate::output::print_human(&format!(
            "Resolved via wildcard: {}",
            if result.is_wildcard { "yes" } else { "no" }
        ));
        if let Some(expiry) = result.expires_at {
            crate::output::print_human(&format!("Expires at: {}", expiry));
        }
        Ok(())
    } else {
        let message = format!("Name '{}' not found or has no resolution", name);
        Err(anyhow!(message))
    }
}
