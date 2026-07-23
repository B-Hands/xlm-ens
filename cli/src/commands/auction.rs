use crate::config::NetworkConfig;
use crate::output::{emit, print_human, with_spinner, OutputFormat};
use crate::signer::SignerProfile;
use anyhow::{anyhow, Context};
use colored::Colorize;
use serde_json::json;
use std::io::{self, Write};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::types::{
    AuctionCreateRequest, AuctionInfo, AuctionStatus, BidRequest, SimulationResult,
};

pub async fn run_create(
    config: NetworkConfig,
    output: OutputFormat,
    name: &str,
    reserve: u64,
    duration: u64,
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

    print_human(&format!("Creating auction for {name}..."));
    if let Some(ref s) = signer {
        print_human(&format!("  Signer: {}", s.describe()));
    }
    let treasury = signer
        .as_ref()
        .map(|s| s.public_address.clone())
        .unwrap_or_else(|| format!("G{}", "A".repeat(55)));

    let submission = with_spinner(
        format!("Submitting auction creation for {name}"),
        output,
        client.create_auction(AuctionCreateRequest {
            name: name.into(),
            asset: "XLM".to_string(),
            treasury,
            reserve_price: reserve,
            duration_seconds: duration,
            signer: signer.as_ref().map(|s| s.name.clone()),
        }),
    )
    .await
    .context("Failed to create auction")?;

    print_human(&format!(
        "SUCCESS: auction created for {name}\n  Reserve: {reserve} XLM\n  Duration: {duration}s\n  Transaction Hash: {}",
        submission.tx_hash
    ));

    Ok(())
}

pub async fn run_bid(
    config: NetworkConfig,
    output: OutputFormat,
    name: &str,
    amount: u64,
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

    print_human(&format!("Placing bid of {amount} XLM on {name}..."));
    if let Some(ref s) = signer {
        print_human(&format!("  Signer: {}", s.describe()));
    }

    let submission = with_spinner(
        format!("Submitting bid for {name}"),
        output,
        client.bid_auction(BidRequest {
            name: name.into(),
            amount,
            signer: signer.as_ref().map(|s| s.name.clone()),
        }),
    )
    .await
    .context("Failed to place bid")?;

    print_human(&format!(
        "SUCCESS: bid placed on {name}\n  Transaction Hash: {}",
        submission.tx_hash
    ));

    Ok(())
}

/// Run the guided auction bidding flow. Non-human output and `--no-interactive`
/// require an explicit amount so the command remains safe for scripts.
pub async fn run_bid_interactive(
    config: NetworkConfig,
    output: OutputFormat,
    name: &str,
    amount: Option<u64>,
    signer: Option<SignerProfile>,
    no_interactive: bool,
    watch: bool,
) -> anyhow::Result<()> {
    let client = auction_client(&config);
    let auction = with_spinner(
        format!("Fetching auction status for {name}"),
        output,
        client.get_auction(name),
    )
    .await
    .context("Failed to fetch auction state")?
    .ok_or_else(|| anyhow!("No active auction found for '{name}'"))?;

    if auction.status != AuctionStatus::Active {
        return Err(anyhow!(
            "auction '{}' is {}; it is not accepting bids",
            name,
            auction.status
        ));
    }

    let interactive = !no_interactive && output == OutputFormat::Human;
    if !interactive && amount.is_none() {
        return Err(anyhow!(
            "--amount is required with --no-interactive or machine-readable output"
        ));
    }
    if interactive {
        print_human(&format_auction_status(&auction));
    }

    let amount = if interactive {
        prompt_bid_amount(amount, auction.reserve_price)?
    } else {
        amount.expect("amount is required outside interactive mode")
    };
    if amount == 0 {
        return Err(anyhow!("bid amount must be greater than zero"));
    }

    let signer_name = signer.as_ref().map(|profile| profile.name.clone());
    let request = BidRequest {
        name: name.to_string(),
        amount,
        signer: signer_name.clone(),
    };
    let simulation = with_spinner(
        format!("Simulating bid for {name}"),
        output,
        client.simulate_bid_auction(&request),
    )
    .await
    .context("Failed to simulate bid")?;

    if interactive {
        print_human(&format_simulation(&simulation));
        if !prompt_confirm(&format!("Submit a bid of {amount} XLM on {name}?"))? {
            return Err(anyhow!("auction bid aborted by user"));
        }
    }

    let submission = with_spinner(
        format!("Submitting bid for {name}"),
        output,
        client.bid_auction(request),
    )
    .await
    .context("Failed to place bid")?;

    let final_auction = if watch {
        Some(watch_for_completion(&client, auction.clone(), output).await?)
    } else {
        None
    };
    let human = format!(
        "SUCCESS: bid placed on {name}\n  Bid: {amount} XLM\n  Estimated network fee: {} stroops\n  Transaction Hash: {}",
        simulation.fee_estimate, submission.tx_hash
    );
    emit(
        output,
        &human,
        json!({
            "name": name,
            "amount": amount,
            "auction": auction_json(&final_auction.unwrap_or_else(|| auction.clone())),
            "simulation": simulation_json(&simulation),
            "transaction_hash": submission.tx_hash,
            "submission_status": submission.status.to_string(),
            "watch_requested": watch,
            "signer": signer_name,
        }),
    );
    Ok(())
}

pub async fn run_inspect(config: NetworkConfig, name: &str) -> anyhow::Result<()> {
    let client = XlmNsClient::new(
        config.rpc_url,
        Some(config.network_passphrase),
        config.registry_contract_id.clone(),
        config.subdomain_contract_id.clone(),
        config.bridge_contract_id.clone(),
        config.auction_contract_id.clone(),
    );

    let auction = client
        .get_auction(name)
        .await
        .context("Failed to fetch auction state")?
        .ok_or_else(|| anyhow!("No active auction found for '{}'", name))?;

    print_human(&format!(
        "Auction for {}:\n  Status: {}\n  Owner: {}\n  Reserve Price: {} XLM\n  Highest Bid: {} XLM",
        auction.name, auction.status, auction.owner, auction.reserve_price, auction.highest_bid
    ));
    if let Some(bidder) = auction.highest_bidder {
        print_human(&format!("  Highest Bidder: {}", bidder));
    }
    print_human(&format!("  Ends at: {}", auction.ends_at));

    Ok(())
}

fn auction_client(config: &NetworkConfig) -> XlmNsClient {
    XlmNsClient::new(
        config.rpc_url.clone(),
        Some(config.network_passphrase.clone()),
        config.registry_contract_id.clone(),
        config.subdomain_contract_id.clone(),
        config.bridge_contract_id.clone(),
        config.auction_contract_id.clone(),
    )
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn format_auction_status(auction: &AuctionInfo) -> String {
    let remaining = auction.ends_at.saturating_sub(now_unix());
    format!(
        "Auction status for {}:\n  Status: {}\n  Reserve price: {} XLM\n  Current highest bid: {} XLM\n  Number of bids: {}\n  Time remaining: {}s\n  Ends at: {}",
        auction.name,
        auction.status,
        auction.reserve_price,
        auction.highest_bid,
        auction.bid_count,
        remaining,
        auction.ends_at,
    )
}

fn format_simulation(simulation: &SimulationResult) -> String {
    format!(
        "Bid transaction simulation:\n  Success: {}\n  Estimated network fee: {} stroops\n  Required authorizations: {}",
        simulation.success,
        simulation.fee_estimate,
        if simulation.auth_addresses.is_empty() {
            "none".to_string()
        } else {
            simulation.auth_addresses.join(", ")
        }
    )
}

fn prompt_bid_amount(default: Option<u64>, reserve: u64) -> anyhow::Result<u64> {
    loop {
        let prompt = match default {
            Some(amount) => format!("Bid amount in XLM [{}]: ", amount.to_string().cyan()),
            None => "Bid amount in XLM: ".bold().to_string(),
        };
        io::stderr().write_all(prompt.as_bytes())?;
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let value = input.trim();
        let amount = if value.is_empty() {
            match default {
                Some(amount) => amount,
                None => {
                    eprintln!("Bid amount is required.");
                    continue;
                }
            }
        } else {
            match value.parse::<u64>() {
                Ok(0) => {
                    eprintln!("Bid amount must be greater than zero.");
                    continue;
                }
                Ok(amount) => amount,
                Err(_) => {
                    eprintln!("Enter a whole-number bid amount.");
                    continue;
                }
            }
        };
        if amount < reserve {
            eprintln!("Warning: this bid is below the reserve price of {reserve} XLM.");
        }
        return Ok(amount);
    }
}

fn prompt_confirm(message: &str) -> anyhow::Result<bool> {
    loop {
        write!(io::stderr(), "{} [y/N]: ", message.bold())?;
        io::stderr().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => return Ok(true),
            "" | "n" | "no" => return Ok(false),
            _ => eprintln!("Please answer y or n."),
        }
    }
}

async fn watch_for_completion(
    client: &XlmNsClient,
    mut auction: AuctionInfo,
    output: OutputFormat,
) -> anyhow::Result<AuctionInfo> {
    while auction.status == AuctionStatus::Active {
        let remaining = auction.ends_at.saturating_sub(now_unix());
        if output == OutputFormat::Human {
            print_human(&format!("Watching auction: {remaining}s remaining..."));
        }
        tokio::time::sleep(Duration::from_secs(remaining.clamp(1, 10))).await;
        auction = client
            .get_auction(&auction.name)
            .await?
            .ok_or_else(|| anyhow!("auction '{}' disappeared while watching", auction.name))?;
    }
    if output == OutputFormat::Human {
        print_human(&format!(
            "Auction completed:\n{}",
            format_auction_status(&auction)
        ));
    }
    Ok(auction)
}

fn auction_json(auction: &AuctionInfo) -> serde_json::Value {
    json!({
        "name": auction.name,
        "status": auction.status.to_string(),
        "reserve_price": auction.reserve_price,
        "highest_bid": auction.highest_bid,
        "highest_bidder": auction.highest_bidder,
        "bid_count": auction.bid_count,
        "ends_at": auction.ends_at,
        "time_remaining_seconds": auction.ends_at.saturating_sub(now_unix()),
    })
}

fn simulation_json(simulation: &SimulationResult) -> serde_json::Value {
    json!({
        "success": simulation.success,
        "estimated_network_fee_stroops": simulation.fee_estimate,
        "required_authorizations": simulation.auth_addresses,
        "error": simulation.error,
    })
}

pub async fn run_settle(
    config: NetworkConfig,
    output: OutputFormat,
    name: &str,
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

    print_human(&format!("Settling auction for {name}..."));
    if let Some(ref s) = signer {
        print_human(&format!("  Signer: {}", s.describe()));
    }

    let submission = with_spinner(
        format!("Submitting settlement for {name}"),
        output,
        client.settle_auction(name, signer.as_ref().map(|s| s.name.clone())),
    )
    .await
    .context("Failed to settle auction")?;

    print_human(&format!(
        "SUCCESS: auction settled for {name}\n  Transaction Hash: {}",
        submission.tx_hash
    ));

    Ok(())
}
//
