use crate::config::NetworkConfig;
use crate::output::{emit, OutputFormat};
use crate::signer::SignerProfile;
use anyhow::Context;
use serde_json::json;
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::types::RegistrationRequest;

pub async fn run_register(
    config: NetworkConfig,
    output: OutputFormat,
    label: &str,
    owner: &str,
    signer: Option<SignerProfile>,
) -> anyhow::Result<()> {
    let registrar_id = config
        .registrar_contract_id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Registrar contract ID not configured"))?;

    let client = XlmNsClient::new(
        config.rpc_url.clone(),
        Some(config.network_passphrase.clone()),
        config.registry_contract_id.clone(),
        config.subdomain_contract_id.clone(),
        config.bridge_contract_id.clone(),
        config.auction_contract_id.clone(),
    )
    .with_registrar(registrar_id.clone());

    let duration_years = 1;
    let quote = client
        .quote_registration(label, duration_years)
        .await
        .context("Failed to fetch registration quote")?;

    let signer_name = signer.as_ref().map(|s| s.name.clone());
    let signer_description = signer.as_ref().map(|s| s.describe());

    let receipt = client
        .register(RegistrationRequest {
            label: label.into(),
            owner: owner.into(),
            duration_years,
            signer: signer_name.clone(),
        })
        .await
        .context("Failed to submit registration")?;

    let human = {
        let mut lines = vec![
            format!("Registration quote for {label}.xlm:"),
            format!("  Registrar: {registrar_id}"),
            format!(
                "  Fee: {} {} (base {}, premium {}, network {})",
                quote.total_fee,
                quote.fee_currency,
                quote.fee_breakdown.base_fee,
                quote.fee_breakdown.premium_fee,
                quote.fee_breakdown.network_fee,
            ),
            format!("  Duration: {duration_years} year(s)"),
            format!("  Expiry: {}", quote.expires_at),
        ];
        if let Some(desc) = signer_description {
            lines.push(format!("  Signer: {desc}"));
        }
        lines.push(String::new());
        lines.push(format!("SUCCESS: registered {} to {}", receipt.name, receipt.owner));
        lines.push(format!("  Fee paid: {} {}", receipt.fee_paid, quote.fee_currency));
        lines.push(format!("  Expires at: {}", receipt.expires_at));
        lines.push(format!("  Status: {}", receipt.submission.status));
        lines.push(format!("  Transaction Hash: {}", receipt.submission.tx_hash));
        lines.join("\n")
    };

    emit(
        output,
        &human,
        json!({
            "name": receipt.name,
            "owner": receipt.owner,
            "duration_years": receipt.duration_years,
            "registrar_contract_id": registrar_id,
            "fee_currency": quote.fee_currency,
            "fee_total": quote.total_fee,
            "fee_base": quote.fee_breakdown.base_fee,
            "fee_premium": quote.fee_breakdown.premium_fee,
            "fee_network": quote.fee_breakdown.network_fee,
            "quote_expires_at": quote.expires_at,
            "quote_grace_period_ends_at": quote.grace_period_ends_at,
            "receipt_fee_paid": receipt.fee_paid,
            "receipt_expires_at": receipt.expires_at,
            "submission_status": receipt.submission.status.to_string(),
            "transaction_hash": receipt.submission.tx_hash,
            "signer": signer_name,
            "network": config.network.as_str(),
        }),
    );

    Ok(())
}
