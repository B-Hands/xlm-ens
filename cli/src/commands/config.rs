use crate::config::{config_template, load_config, Network, ResolveOptions};
use crate::output::{emit, OutputFormat};
use serde_json::json;
use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

fn resolve_path(path: Option<PathBuf>) -> PathBuf {
    path.unwrap_or_else(|| PathBuf::from(".xlm-ns.toml"))
}

fn parse_network(network: &str) -> anyhow::Result<Network> {
    Network::parse(network)
        .ok_or_else(|| anyhow::anyhow!("invalid network '{network}' (expected testnet or mainnet)"))
}

fn write_template(path: &Path, network: Network, force: bool) -> anyhow::Result<()> {
    if path.exists() && !force {
        return Err(anyhow::anyhow!(
            "config file {} already exists (pass --force to overwrite)",
            path.display()
        ));
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
        }
    }

    let mut file = fs::File::create(path)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", path.display()))?;
    file.write_all(config_template(network).as_bytes())
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))?;
    Ok(())
}

fn open_editor(path: &Path) -> anyhow::Result<()> {
    let editor = env::var("VISUAL")
        .ok()
        .or_else(|| env::var("EDITOR").ok())
        .unwrap_or_else(|| "vi".to_string());

    let status = Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|err| anyhow::anyhow!("failed to launch editor '{editor}': {err}"))?;

    if !status.success() {
        return Err(anyhow::anyhow!(
            "editor '{editor}' exited with status {status}"
        ));
    }

    Ok(())
}

pub async fn run_init(path: Option<PathBuf>, network: &str, force: bool) -> anyhow::Result<()> {
    let network = parse_network(network)?;
    let path = resolve_path(path);
    write_template(&path, network, force)?;

    println!("Wrote config template to {}", path.display());
    Ok(())
}

pub async fn run_edit(path: Option<PathBuf>, network: &str) -> anyhow::Result<()> {
    let network = parse_network(network)?;
    let path = resolve_path(path);
    if !path.exists() {
        write_template(&path, network, false)?;
    }

    open_editor(&path)?;
    Ok(())
}

pub async fn run_validate(
    path: Option<PathBuf>,
    network: &str,
    output: OutputFormat,
    _fix: bool,
) -> anyhow::Result<()> {
    let network = parse_network(network)?;
    let config = load_config(
        network,
        ResolveOptions {
            config_path: path,
            ..ResolveOptions::default()
        },
    )?;

    let validation = crate::commands::validate::run(&config).await;
    let mut failures = 0;

    if output == OutputFormat::Human {
        println!("Validating configuration for {}...", network.as_str());
        for result in &validation.contract_id_format {
            if result.status == crate::commands::validate::ValidationStatus::Fail {
                failures += 1;
            }
            println!("[{}] {}", result.status, result.name);
        }
        if validation.rpc_connectivity.status == crate::commands::validate::ValidationStatus::Fail {
            failures += 1;
        }
        println!(
            "[{}] {}",
            validation.rpc_connectivity.status, validation.rpc_connectivity.name
        );
        if validation.network_passphrase.status == crate::commands::validate::ValidationStatus::Fail
        {
            failures += 1;
        }
        println!(
            "[{}] {}",
            validation.network_passphrase.status, validation.network_passphrase.name
        );
        if validation.signing_key.status == crate::commands::validate::ValidationStatus::Fail {
            failures += 1;
        }
        println!(
            "[{}] {}",
            validation.signing_key.status, validation.signing_key.name
        );

        if failures > 0 {
            println!("\nValidation failed with {failures} errors.");
            std::process::exit(1);
        } else {
            println!("\nConfiguration is valid.");
        }
    } else {
        let mut results = validation.contract_id_format;
        results.push(validation.rpc_connectivity);
        results.push(validation.network_passphrase);
        results.push(validation.signing_key);

        let ok = results
            .iter()
            .all(|r| r.status == crate::commands::validate::ValidationStatus::Pass);

        emit(
            output,
            "Validation results",
            json!({
                "ok": ok,
                "results": results,
            }),
        );

        if !ok {
            std::process::exit(1);
        }
    }

    Ok(())
}

pub async fn run_show(
    path: Option<PathBuf>,
    network: &str,
    output: OutputFormat,
) -> anyhow::Result<()> {
    let network = parse_network(network)?;
    let config = load_config(
        network,
        ResolveOptions {
            config_path: path,
            ..ResolveOptions::default()
        },
    )?;

    emit(
        output,
        "Current configuration",
        json!({
            "network": config.network.as_str(),
            "config_path": config.config_path,
            "rpc_url": config.rpc_url,
            "network_passphrase": "[REDACTED]",
            "registry_contract_id": config.registry_contract_id,
            "registrar_contract_id": config.registrar_contract_id,
            "resolver_contract_id": config.resolver_contract_id,
            "auction_contract_id": config.auction_contract_id,
            "bridge_contract_id": config.bridge_contract_id,
            "subdomain_contract_id": config.subdomain_contract_id,
            "nft_contract_id": config.nft_contract_id,
        }),
    );

    Ok(())
}
