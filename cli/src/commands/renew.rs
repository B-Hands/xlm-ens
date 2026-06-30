use crate::config::NetworkConfig;
use crate::output::{emit, emit_error, with_spinner, OutputFormat};
use crate::signer::SignerProfile;
use anyhow::Context;
use serde_json::json;
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::types::RenewalRequest;

pub async fn run_renew(
    config: NetworkConfig,
    output: OutputFormat,
    name: &str,
    years: u64,
    signer: Option<SignerProfile>,
) -> anyhow::Result<()> {
    let client = XlmNsClient::new(
        config.rpc_url,
        Some(config.network_passphrase),
        config.registry_contract_id.clone(),
        config.subdomain_contract_id.clone(),
        config.bridge_contract_id.clone(),
        config.auction_contract_id.clone(),
    );

    let registration = client
        .get_registration(name)
        .await
        .context("Failed to fetch registration state")?;

    match registration {
        Some(_) => {
            let signer_description = signer.as_ref().map(|s| s.describe());
            let receipt = with_spinner(
                format!("Submitting renewal for {name}"),
                output,
                client.renew(RenewalRequest {
                    name: name.into(),
                    additional_years: years as u32,
                    signer: signer.as_ref().map(|s| s.name.clone()),
                }),
            )
            .await
            .context("Failed to renew name")?;

            let mut human_lines = Vec::new();
            if let Some(desc) = signer_description {
                human_lines.push(format!("  Signer: {desc}"));
            }
            human_lines.push(format!("SUCCESS: Renewed {name} for {years} year(s)"));
            human_lines.push(format!("  Fee Paid: {} XLM", receipt.fee_paid));
            human_lines.push(format!("  New Expiry: {}", receipt.new_expiry));
            human_lines.push(format!("  Status: {}", receipt.submission.status));
            human_lines.push(format!(
                "  Transaction Hash: {}",
                receipt.submission.tx_hash
            ));

            emit(
                output,
                &human_lines.join("\n"),
                json!({
                    "name": receipt.name,
                    "additional_years": receipt.additional_years,
                    "fee_paid": receipt.fee_paid,
                    "new_expiry": receipt.new_expiry,
                    "status": receipt.submission.status.to_string(),
                    "transaction_hash": receipt.submission.tx_hash,
                    "contract_id": receipt.submission.contract_id,
                    "signer": receipt.submission.signer,
                }),
            );
        }
        None => {
            let message = format!("Name '{}' is not registered and cannot be renewed.", name);
            emit_error(
                output,
                &message,
                json!({"error": message.clone(), "name": name}),
            );
            return Err(anyhow::anyhow!(message));
        }
    }

    Ok(())
}
