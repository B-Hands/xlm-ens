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
        let human = {
            let mut lines = vec![format!("Name: {}", result.name), format!("Address: {addr}")];
            if let Some(resolver) = result.resolver.clone() {
                lines.push(format!("Resolver: {resolver}"));
            }
            if let Some(expiry) = result.expires_at {
                lines.push(format!("Expires at: {expiry}"));
            }
            lines.join("\n")
        };
        emit(
            output,
            &human,
            json!({
                "name": result.name,
                "address": addr,
                "resolver": result.resolver,
                "expires_at": result.expires_at,
            }),
        );
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
