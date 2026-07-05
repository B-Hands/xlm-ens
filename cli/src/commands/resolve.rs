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

    if let Some(addr) = result.address.clone() {
        let mut human_lines = vec![format!("Name: {}\nAddress: {}", result.name, addr)];
        if let Some(ref resolver) = result.resolver {
            human_lines.push(format!("Resolver: {}", resolver));
        }
        human_lines.push(format!(
            "Resolved via wildcard: {}",
            if result.is_wildcard { "yes" } else { "no" }
        ));
        if let Some(expiry) = result.expires_at {
            human_lines.push(format!("Expires at: {}", expiry));
        }

        emit(
            output,
            &human_lines.join("\n"),
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
        Err(anyhow!(message))
    }
}
