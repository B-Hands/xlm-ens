use crate::config::NetworkConfig;
use crate::output::{emit, OutputFormat};
use crate::signer::SignerProfile;
use anyhow::Context;
use serde_json::json;
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::types::TransferRequest;

pub async fn run_transfer(
    config: NetworkConfig,
    output: OutputFormat,
    name: &str,
    new_owner: &str,
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

    let mut human_lines = vec![format!("Initiating transfer of {name} to {new_owner}...")];
    if let Some(ref s) = signer {
        human_lines.push(format!("  Signer: {}", s.describe()));
    }

    let submission = client
        .transfer(TransferRequest {
            name: name.into(),
            new_owner: new_owner.into(),
            signer: signer.as_ref().map(|s| s.name.clone()),
        })
        .await
        .context("Failed to submit transfer")?;

    let verified_owner = match client.get_registration(name).await {
        Ok(Some(reg)) => reg.address,
        _ => None,
    };

    human_lines.push(format!("SUCCESS: {name} ownership transferred to {new_owner}"));
    human_lines.push(format!("  Status: {}", submission.status));
    human_lines.push(format!("  Transaction Hash: {}", submission.tx_hash));
    match &verified_owner {
        Some(addr) => human_lines.push(format!("Verified: Current owner is now {addr}")),
        None => human_lines.push(String::from(
            "Warning: Could not verify ownership change immediately.",
        )),
    }

    emit(
        output,
        &human_lines.join("\n"),
        json!({
            "name": name,
            "new_owner": new_owner,
            "status": submission.status.to_string(),
            "transaction_hash": submission.tx_hash,
            "contract_id": submission.contract_id,
            "signer": submission.signer,
            "verified_owner": verified_owner,
        }),
    );

    Ok(())
}
