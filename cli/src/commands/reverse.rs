use crate::config::NetworkConfig;
use crate::output::{emit, emit_error, OutputFormat};
use anyhow::Context;
use serde_json::json;
use xlm_ns_sdk::client::XlmNsClient;

pub async fn run_reverse(
    config: NetworkConfig,
    output: OutputFormat,
    address: &str,
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
        .reverse_resolve(address)
        .await
        .context("Failed to perform reverse lookup")?;

    if let Some(name) = result.primary_name {
        let resolved_address = result.address.clone();
        emit(
            output,
            &format!("{} -> {}", resolved_address, name),
            json!({
                "address": resolved_address,
                "primary_name": name,
                "resolver": result.resolver,
            }),
        );
    } else {
        let message = format!("{} -> [NO PRIMARY NAME]", result.address);
        emit_error(output, &message, json!({"address": result.address}));
    }

    Ok(())
}
