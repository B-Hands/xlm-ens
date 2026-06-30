use crate::config::NetworkConfig;
use crate::output::{emit, with_spinner, OutputFormat};
use crate::signer::SignerProfile;
use anyhow::Context;
use colored::Colorize;
use serde_json::json;
use std::io::{self, IsTerminal, Write};
use xlm_ns_common::validation::{validate_account_address, validate_label};
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::types::RegistrationRequest;

pub async fn run_register(
    config: NetworkConfig,
    output: OutputFormat,
    name: Option<String>,
    owner: Option<String>,
    signer: Option<SignerProfile>,
    interactive: bool,
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

    let signer_name = signer.as_ref().map(|s| s.name.clone());
    let signer_description = signer.as_ref().map(|s| s.describe());

    let tty_available =
        std::io::stdin().is_terminal() || std::env::var_os("XLM_NS_FORCE_INTERACTIVE").is_some();
    let need_prompt = interactive || name.is_none() || owner.is_none();
    let should_prompt = need_prompt && tty_available;
    if need_prompt && !tty_available && (name.is_none() || owner.is_none()) {
        return Err(anyhow::anyhow!(
            "interactive registration requires a TTY; provide both name and owner explicitly or run in a terminal"
        ));
    }

    let (label, owner) = if should_prompt {
        prompt_registration_inputs(name, owner)?
    } else {
        (
            normalize_label(&name.expect("name is required"))?,
            normalize_owner(&owner.expect("owner is required"))?,
        )
    };

    if interactive {
        let fqdn = format!("{label}.xlm");
        match client
            .get_registration(&fqdn)
            .await
            .context("Failed to check name availability")?
        {
            Some(_) => return Err(anyhow::anyhow!("name {fqdn} is already registered")),
            None => println!(
                "{}",
                format!("Availability check passed for {fqdn}.").green()
            ),
        }
    }

    let duration_years = 1;
    let quote = with_spinner(
        format!("Fetching registration quote for {label}.xlm"),
        output,
        client.quote_registration(label, duration_years),
    )
    .await
    .context("Failed to fetch registration quote")?;

    if interactive {
        println!("{}", format!("Registration quote for {label}.xlm:").bold());
        println!("  Registrar: {}", registrar_id.cyan());
        println!(
            "  Fee: {} {} (base {}, premium {}, network {})",
            quote.total_fee,
            quote.fee_currency,
            quote.fee_breakdown.base_fee,
            quote.fee_breakdown.premium_fee,
            quote.fee_breakdown.network_fee,
        );
        println!("  Duration: {duration_years} year(s)");
        println!("  Expiry: {}", quote.expires_at);

        let confirmed = prompt_confirm(&format!(
            "Proceed with payment of {} {} and register {}.xlm?",
            quote.total_fee, quote.fee_currency, label
        ))?;

        if !confirmed {
            return Err(anyhow::anyhow!("registration aborted by user"));
        }
    }

    let receipt = with_spinner(
        format!("Submitting registration for {label}.xlm"),
        output,
        client.register(RegistrationRequest {
            label: label.into(),
            owner: owner.into(),
            duration_years,
            signer: signer_name.clone(),
        }),
    )
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
        lines.push(format!(
            "SUCCESS: registered {} to {}",
            receipt.name, receipt.owner
        ));
        lines.push(format!(
            "  Fee paid: {} {}",
            receipt.fee_paid, quote.fee_currency
        ));
        lines.push(format!("  Expires at: {}", receipt.expires_at));
        lines.push(format!("  Status: {}", receipt.submission.status));
        lines.push(format!(
            "  Transaction Hash: {}",
            receipt.submission.tx_hash
        ));
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

fn prompt_registration_inputs(
    name: Option<String>,
    owner: Option<String>,
) -> anyhow::Result<(String, String)> {
    let name = prompt_text("Name to register", name, normalize_label)?;
    let owner = prompt_text("Owner address", owner, normalize_owner)?;

    Ok((name, owner))
}

fn prompt_text<F>(label: &str, existing: Option<String>, validate: F) -> anyhow::Result<String>
where
    F: Fn(&str) -> anyhow::Result<String>,
{
    loop {
        let default_value = existing
            .as_deref()
            .map(validate)
            .transpose()?
            .unwrap_or_default();

        let prompt = if default_value.is_empty() {
            format!("{}: ", label.bold())
        } else {
            format!("{} [{}]: ", label.bold(), default_value.cyan())
        };

        io::stderr()
            .write_all(prompt.as_bytes())
            .context("Failed to write prompt")?;
        io::stderr().flush().context("Failed to flush prompt")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read response")?;
        let value = input.trim();

        if value.is_empty() {
            if !default_value.is_empty() {
                return Ok(default_value);
            }
            io::stderr()
                .write_all(b"\nValue is required.\n")
                .context("Failed to write validation message")?;
            continue;
        }

        match validate(value) {
            Ok(valid) => return Ok(valid),
            Err(err) => {
                io::stderr()
                    .write_all(format!("\n{err}\n").as_bytes())
                    .context("Failed to write validation message")?;
            }
        }
    }
}

fn prompt_confirm(message: &str) -> anyhow::Result<bool> {
    loop {
        io::stderr()
            .write_all(format!("{} [y/N]: ", message.bold()).as_bytes())
            .context("Failed to write confirmation prompt")?;
        io::stderr()
            .flush()
            .context("Failed to flush confirmation prompt")?;

        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .context("Failed to read confirmation")?;
        let answer = input.trim().to_ascii_lowercase();
        match answer.as_str() {
            "y" | "yes" => return Ok(true),
            "" | "n" | "no" => return Ok(false),
            _ => {
                io::stderr()
                    .write_all(b"Please answer y or n.\n")
                    .context("Failed to write confirmation error")?;
            }
        }
    }
}

fn normalize_label(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    let label = trimmed.strip_suffix(".xlm").unwrap_or(trimmed);
    validate_label(label).map_err(|err| anyhow::anyhow!("invalid name: {err}"))?;
    Ok(label.to_string())
}

fn normalize_owner(value: &str) -> anyhow::Result<String> {
    let trimmed = value.trim();
    validate_account_address(trimmed)
        .map_err(|err| anyhow::anyhow!("invalid owner address: {err}"))?;
    Ok(trimmed.to_string())
}
