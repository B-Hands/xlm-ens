#![allow(clippy::items_after_test_module)]
mod commands;
mod config;
mod error;
mod export;
mod output;
mod signer;

use anyhow::Context;
use clap::{Parser, Subcommand};
use commands::completions::CompletionCommand;
use commands::watch::WatchCommand;
use config::{load_config, ContractKind, ContractOverrides, Network, ResolveOptions};
use output::{configure as configure_output, OutputFormat};
use signer::{load_profile, SignerProfile};
use std::path::PathBuf;
use std::process;

const BIN_NAME: &str = "xlm-ns";

#[derive(Parser)]
#[command(name = BIN_NAME)]
#[command(about = "XLM Name Service CLI", long_about = None)]
#[command(
    after_help = "Shell completions:\n  xlm-ns completions bash > ~/.local/share/bash-completion/completions/xlm-ns\n  xlm-ns completions zsh > ~/.local/share/zsh/site-functions/_xlm-ns\n  xlm-ns completions fish > ~/.config/fish/completions/xlm-ns.fish\n  xlm-ns completions install\n"
)]
struct Cli {
    /// Network to use (`testnet` or `mainnet`)
    #[arg(short, long, value_enum, default_value_t = Network::Testnet, global = true)]
    network: Network,

    /// Config file path. Falls back to `XLM_NS_CONFIG`, then the documented search path.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Output format. Use 'json' or 'csv' for machine-readable export suitable for piping or automation.
    #[arg(long, value_enum, default_value_t = OutputFormat::Human, global = true)]
    output: OutputFormat,

    /// Disable terminal colors and progress indicators.
    #[arg(long, global = true)]
    no_color: bool,

    /// Simulate the transaction without submitting it
    #[arg(long, global = true)]
    dry_run: bool,

    /// Override the Soroban RPC URL.
    #[arg(long, global = true)]
    rpc_url: Option<String>,

    /// Override the Soroban network passphrase.
    #[arg(long, global = true)]
    network_passphrase: Option<String>,

    #[arg(long, global = true)]
    registry_contract_id: Option<String>,

    #[arg(long, global = true)]
    registrar_contract_id: Option<String>,

    #[arg(long, global = true)]
    resolver_contract_id: Option<String>,

    #[arg(long, global = true)]
    auction_contract_id: Option<String>,

    #[arg(long, global = true)]
    bridge_contract_id: Option<String>,

    #[arg(long, global = true)]
    subdomain_contract_id: Option<String>,

    #[arg(long, global = true)]
    nft_contract_id: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Clone)]
enum MigrateCommands {
    /// Transform a JSON state file from one schema version to another.
    ///
    /// NOTE: storage rewriting is placeholder in this environment; only
    /// schema_version + file metadata may be updated.
    Transform {
        #[arg(long)]
        from_version: u32,
        #[arg(long)]
        to_version: u32,
        /// Input JSON file (exported contract state)
        #[arg(long)]
        in_file: PathBuf,
        /// Output JSON file
        #[arg(long)]
        out_file: PathBuf,
    },

    /// Verify two JSON state files entry-by-entry.
    Verify {
        /// Source JSON state file
        #[arg(long)]
        source_file: PathBuf,
        /// Target JSON state file
        #[arg(long)]
        target_file: PathBuf,
        /// When strict, exits non-zero on any mismatch.
        #[arg(long, default_value_t = false)]
        strict: bool,
    },

    /// Dry-run the transform without writing output.
    DryRun {
        #[arg(long)]
        from_version: u32,
        #[arg(long)]
        to_version: u32,
        #[arg(long)]
        in_file: PathBuf,
    },

    /// Generate rollback metadata for an upcoming upgrade.
    ///
    /// NOTE: WASM-hash extraction is stubbed in this environment.
    RollbackMetadata {
        #[arg(long)]
        contract_id: String,
        #[arg(long)]
        wasm_hash_out: PathBuf,
    },

    /// Storage export
    Export {
        contract_id: String,
        #[arg(long, default_value = "state.json")]
        out_file: PathBuf,
    },

    /// Storage import
    Import {
        contract_id: String,
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Migrate Soroban contract storage safely (transform/verify; export/import stubbed)
    Migrate {
        #[command(subcommand)]
        command: MigrateCommands,
    },

    /// Register a new name.
    Register {
        /// Name to register
        name: Option<String>,
        /// Owner address
        owner: Option<String>,
        /// Launch the guided registration flow.
        #[arg(long)]
        interactive: bool,
        /// Signer profile to use for submission
        #[arg(long)]
        signer: Option<String>,
    },
    /// Resolve a name to an address.
    Resolve {
        /// Name to resolve
        name: String,
    },
    /// Reverse-resolve an address to its primary name.
    #[command(alias = "reverse-lookup")]
    ReverseResolve {
        /// Address to reverse-resolve
        address: String,
    },
    /// Read or mutate resolver text records.
    #[command(subcommand)]
    Text(TextCommand),
    /// Transfer ownership of a name.
    Transfer {
        /// Name to transfer
        name: String,
        /// New owner address
        new_owner: String,
        /// Signer profile to use for submission
        #[arg(long)]
        signer: Option<String>,
    },
    /// Renew a name registration.
    Renew {
        /// Name to renew
        name: String,
        /// Additional years to renew for
        #[arg(default_value_t = 1)]
        years: u64,
        /// Signer profile to use for submission
        #[arg(long)]
        signer: Option<String>,
    },
    /// Manage auctions for names
    #[command(subcommand)]
    Auction(AuctionCommands),
    /// Generate a shell completion script.
    #[command(
        subcommand,
        long_about = "Generate or install shell completions for bash, zsh, and fish.\n\nUse `xlm-ns completions bash|zsh|fish` to print a completion script to stdout.\nUse `xlm-ns completions install` to install into the standard per-shell user directory."
    )]
    Completions(CompletionCommand),
    /// Bridge management commands.
    #[command(subcommand)]
    Bridge(BridgeCommands),
    /// Subdomain management commands
    #[command(subcommand)]
    Subdomain(SubdomainCommands),
    /// Inspect NFT ownership metadata.
    #[command(subcommand)]
    Nft(NftCommands),
    /// Manage configuration files and validation.
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Watch for name expirations
    #[command(subcommand)]
    Watch(WatchCommand),
    /// Show registration details for a single name.
    Whois {
        /// Name to inspect
        name: String,
    },
    /// List names owned by an address.
    Portfolio {
        /// Owner address to inspect
        owner: String,
        /// Number of names to fetch per RPC request.
        #[arg(long = "batch-size", default_value_t = 50)]
        batch_size: usize,
        /// Maximum number of names to return.
        #[arg(long)]
        limit: Option<usize>,
        /// Fetch a single 1-based page instead of the whole portfolio.
        #[arg(long)]
        page: Option<usize>,
    },
    /// Scan a portfolio for names approaching expiry, in grace period, or claimable.
    ///
    /// Categorizes owned names by urgency (Claimable, Grace Period, Warning, OK) and
    /// reports renewal cost estimates. Use `--auto-renew` to submit renewal
    /// transactions for Warning/Grace Period names (combine with the global
    /// `--dry-run` flag to simulate without submitting).
    RenewalCheck {
        /// Owner address to inspect
        owner: String,
        /// Warn when a name has fewer than this many days remaining before expiry.
        #[arg(long = "warn-days", default_value_t = 30)]
        warn_days: u32,
        /// Submit renewal transactions for Warning/Grace Period names.
        #[arg(long)]
        auto_renew: bool,
        /// Number of names to fetch per RPC request.
        #[arg(long = "batch-size", default_value_t = 50)]
        batch_size: usize,
        /// Maximum number of names to return.
        #[arg(long)]
        limit: Option<usize>,
        /// Fetch a single 1-based page instead of the whole portfolio.
        #[arg(long)]
        page: Option<usize>,
    },
    /// Fetch a registration price quote without submitting a transaction (read-only).
    ///
    /// Use this to inspect the full fee breakdown and lifecycle timestamps before
    /// deciding whether to register a name.
    Quote {
        /// Name label to quote (without the .xlm suffix)
        name: String,
        /// Number of years to quote for
        #[arg(default_value_t = 1)]
        years: u32,
    },
    /// Perform bulk operations from a file.
    #[command(subcommand)]
    Bulk(BulkCommands),
    /// Check whether a name is available for registration (read-only).
    ///
    /// Outputs the availability status: available, active, grace-period, or claimable.
    /// No transaction is submitted.
    Availability {
        /// Name to check (e.g. `alice.xlm` or just `alice`)
        name: String,
    },
    /// Verify RPC connectivity, network passphrase, and configured contract IDs (read-only).
    ///
    /// Exits with a non-zero status when any check fails so the command can be
    /// used in health-probe scripts and CI pipelines.
    Healthcheck,
}

#[derive(Subcommand, Clone)]
pub enum BulkCommands {
    /// Bulk register names from a file.
    Register {
        /// Path to the file containing the names to register.
        #[arg(long)]
        file: PathBuf,
    },
    /// Bulk renew names from a file.
    Renew {
        /// Path to the file containing the names to renew.
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Subcommand, Clone)]
enum AuctionCommands {
    /// Create a new auction for a name
    Create {
        /// Name to auction
        name: String,
        /// Reserve price in XLM
        #[arg(long, default_value_t = 0)]
        reserve: u64,
        /// Auction duration in seconds
        #[arg(long, default_value_t = 86400)]
        duration: u64,
        /// Signer profile
        #[arg(long)]
        signer: Option<String>,
    },
    /// Place a bid on an active auction
    Bid {
        /// Name under auction
        name: String,
        /// Bid amount in XLM
        amount: u64,
        /// Signer profile
        #[arg(long)]
        signer: Option<String>,
    },
    /// Inspect the state of an auction
    Inspect {
        /// Name to inspect
        name: String,
    },
    /// Settle a completed auction
    Settle {
        /// Name to settle
        name: String,
        /// Signer profile
        #[arg(long)]
        signer: Option<String>,
    },
    /// Export all text records for a name.
    Export {
        /// Name to export text records for
        name: String,
        /// File to write records to (JSON). Prints to stdout if omitted.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Import text records for a name from a file.
    Import {
        /// Name to import text records to
        name: String,
        /// File containing records to import (JSON)
        file: PathBuf,
        /// Signer profile
        #[arg(long)]
        signer: Option<String>,
    },
}

#[derive(Subcommand, Clone)]
enum SubdomainCommands {
    /// Register a parent domain for subdomain management
    /// This enables the parent domain owner to create and manage subdomains
    RegisterParent {
        /// Parent domain name (e.g., example.xlm)
        parent: String,
        /// Owner address for the parent domain
        owner: String,
    },
    /// Add a controller to a parent domain
    /// Controllers can create subdomains under the parent domain
    AddController {
        /// Parent domain name
        parent: String,
        /// Controller address to add (must be called by parent owner)
        controller: String,
    },
    /// Create a subdomain under a registered parent
    /// Can be called by parent owner or authorized controllers
    Create {
        /// Subdomain label (e.g., 'sub' for sub.example.xlm)
        label: String,
        /// Parent domain name
        parent: String,
        /// Owner address for the new subdomain
        owner: String,
    },
    /// Transfer ownership of a subdomain
    /// Can only be called by the current subdomain owner
    Transfer {
        /// Full subdomain name (e.g., sub.example.xlm)
        fqdn: String,
        /// New owner address
        new_owner: String,
    },
}

#[derive(Subcommand, Clone)]
enum BridgeCommands {
    /// Register a bridge route for a supported chain
    Register {
        /// Chain name (base, ethereum, arbitrum)
        chain: String,
    },
    /// Inspect bridge route for a chain
    Inspect {
        /// Chain name to inspect
        chain: String,
    },
    /// Generate payload for cross-chain resolution
    Payload {
        /// Name to resolve
        name: String,
        /// Target chain
        chain: String,
    },
    /// Publish bridge payload test vectors for EVM resolver consumption
    TestVectors,
}

#[derive(Subcommand, Clone)]
enum NftCommands {
    /// Inspect the owner and metadata for a token id.
    Inspect { token_id: String },
}

#[derive(Subcommand, Clone)]
enum ConfigCommands {
    /// Create a config file template.
    Init {
        /// Config file path. Defaults to the CLI search path's first entry.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Network profile to render into the template.
        #[arg(long, value_enum, default_value_t = Network::Testnet)]
        network: Network,
        /// Overwrite an existing file.
        #[arg(long)]
        force: bool,
    },
    /// Open the config file in the user's editor.
    Edit {
        /// Config file path. Defaults to the CLI search path's first entry.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Network profile to render when creating a new file.
        #[arg(long, value_enum, default_value_t = Network::Testnet)]
        network: Network,
    },
    /// Validate a config file without invoking any contract RPCs.
    Validate {
        /// Config file path. Falls back to the configured search path.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Network to validate against.
        #[arg(long, value_enum, default_value_t = Network::Testnet)]
        network: Network,
        /// Interactively prompt to correct invalid values.
        #[arg(long)]
        fix: bool,
    },
    /// Show the current configuration.
    Show {
        /// Config file path. Falls back to the configured search path.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Network to show the configuration for.
        #[arg(long, value_enum, default_value_t = Network::Testnet)]
        network: Network,
    },
}

#[derive(Subcommand, Clone)]
enum TextCommand {
    /// Read a text record value for a name.
    Get { name: String, key: String },
    /// Write a text record value on a name.
    Set {
        name: String,
        key: String,
        value: Option<String>,
        #[arg(long)]
        signer: Option<String>,
    },
}

fn resolve_signer(name: Option<String>) -> anyhow::Result<Option<SignerProfile>> {
    let name = match name {
        Some(n) => n,
        None => return Ok(None),
    };
    load_profile(&name)
        .map(Some)
        .context("failed to load signer profile")
}

async fn run_with_cli(cli: Cli) -> anyhow::Result<()> {
    configure_output(cli.no_color);

    if let Commands::Completions(command) = cli.command.clone() {
        commands::completions::run_completion_command::<Cli>(command, BIN_NAME)?;
        return Ok(());
    }

    let network = cli.network;

    let contract_overrides = ContractOverrides {
        registry_contract_id: cli.registry_contract_id.clone(),
        registrar_contract_id: cli.registrar_contract_id.clone(),
        resolver_contract_id: cli.resolver_contract_id.clone(),
        auction_contract_id: cli.auction_contract_id.clone(),
        bridge_contract_id: cli.bridge_contract_id.clone(),
        subdomain_contract_id: cli.subdomain_contract_id.clone(),
        nft_contract_id: cli.nft_contract_id.clone(),
    };

    let config = load_config(
        network,
        ResolveOptions {
            config_path: cli.config.clone(),
            rpc_url: cli.rpc_url.clone(),
            network_passphrase: cli.network_passphrase.clone(),
            contract_overrides: contract_overrides.clone(),
        },
    )
    .context("failed to load configuration")?;

    if !cli.dry_run {
        if let Err(err) = validate_contract_policy(&cli.command, &contract_overrides, &config) {
            return Err(anyhow::anyhow!(err));
        }
    }

    match cli.command {
        Commands::Migrate { command } => match command {
            MigrateCommands::Transform {
                from_version,
                to_version,
                in_file,
                out_file,
            } => {
                commands::migrate::run_transform(
                    cli.output,
                    config.clone(),
                    from_version,
                    to_version,
                    in_file,
                    out_file,
                    cli.dry_run,
                )
                .await
            }
            MigrateCommands::DryRun {
                from_version,
                to_version,
                in_file,
            } => {
                commands::migrate::run_transform(
                    cli.output,
                    config.clone(),
                    from_version,
                    to_version,
                    in_file,
                    // out_file is unused in dry_run; still required by signature.
                    PathBuf::from("/dev/null"),
                    true,
                )
                .await
            }
            MigrateCommands::Verify {
                source_file,
                target_file,
                strict,
            } => {
                commands::migrate::run_verify(
                    cli.output,
                    config.clone(),
                    source_file,
                    target_file,
                    strict,
                )
                .await
            }
            MigrateCommands::RollbackMetadata {
                contract_id,
                wasm_hash_out,
            } => {
                commands::migrate::run_rollback_metadata(cli.output, contract_id, wasm_hash_out)
                    .await
            }
            MigrateCommands::Export {
                contract_id,
                out_file,
            } => {
                commands::migrate::run_export(cli.output, cli.dry_run, contract_id, out_file).await
            }
            MigrateCommands::Import { contract_id, file } => {
                commands::migrate::run_import(cli.output, cli.dry_run, contract_id, file).await
            }
        },
        Commands::Register {
            name,
            owner,
            interactive,
            signer,
        } => {
            commands::register::run_register(
                config,
                cli.output,
                name,
                owner,
                resolve_signer(signer)?,
                interactive,
            )
            .await
        }
        Commands::Resolve { name } => {
            commands::resolve::run_resolve(config, cli.output, &name).await
        }
        Commands::ReverseResolve { address } => {
            commands::reverse::run_reverse(config, cli.output, &address).await
        }
        Commands::Text(sub) => match sub {
            TextCommand::Get { name, key } => {
                commands::text::run_get(config, cli.output, &name, &key).await
            }
            TextCommand::Set {
                name,
                key,
                value,
                signer,
            } => {
                commands::text::run_set(
                    config,
                    cli.output,
                    &name,
                    &key,
                    value,
                    resolve_signer(signer)?,
                )
                .await
            }
        },
        Commands::Transfer {
            name,
            new_owner,
            signer,
        } => {
            commands::transfer::run_transfer(
                config,
                cli.output,
                &name,
                &new_owner,
                resolve_signer(signer)?,
            )
            .await
        }
        Commands::Renew {
            name,
            years,
            signer,
        } => {
            commands::renew::run_renew(config, cli.output, &name, years, resolve_signer(signer)?)
                .await
        }
        Commands::Auction(sub) => match sub {
            AuctionCommands::Create {
                name,
                reserve,
                duration,
                signer,
            } => {
                commands::auction::run_create(
                    config,
                    cli.output,
                    &name,
                    reserve,
                    duration,
                    resolve_signer(signer)?,
                )
                .await
            }
            AuctionCommands::Bid {
                name,
                amount,
                signer,
            } => {
                commands::auction::run_bid(
                    config,
                    cli.output,
                    &name,
                    amount,
                    resolve_signer(signer)?,
                )
                .await
            }
            AuctionCommands::Inspect { name } => {
                commands::auction::run_inspect(config, &name).await
            }
            AuctionCommands::Settle { name, signer } => {
                commands::auction::run_settle(config, cli.output, &name, resolve_signer(signer)?)
                    .await
            }
            AuctionCommands::Export { .. } | AuctionCommands::Import { .. } => Err(
                anyhow::anyhow!("auction text import/export is not implemented"),
            ),
        },
        Commands::Bridge(command) => match command {
            BridgeCommands::Register { chain } => {
                commands::bridge::run_register_chain(config, cli.output, &chain).await
            }
            BridgeCommands::Inspect { chain } => {
                commands::bridge::run_inspect_route(config, &chain).await
            }
            BridgeCommands::Payload { name, chain } => {
                commands::bridge::run_generate_payload(config, cli.output, &name, &chain).await
            }
            BridgeCommands::TestVectors => Err(anyhow::anyhow!(
                "bridge test vector export is not implemented"
            )),
        },
        Commands::Subdomain(command) => match command {
            SubdomainCommands::RegisterParent { parent, owner } => {
                commands::subdomain::run_register_parent(config, cli.output, &parent, &owner).await
            }
            SubdomainCommands::AddController { parent, controller } => {
                commands::subdomain::run_add_controller(config, cli.output, &parent, &controller)
                    .await
            }
            SubdomainCommands::Create {
                label,
                parent,
                owner,
            } => {
                commands::subdomain::run_create_subdomain(
                    config, cli.output, &label, &parent, &owner,
                )
                .await
            }
            SubdomainCommands::Transfer { fqdn, new_owner } => {
                commands::subdomain::run_transfer_subdomain(config, cli.output, &fqdn, &new_owner)
                    .await
            }
        },
        Commands::Nft(command) => match command {
            NftCommands::Inspect { token_id } => {
                commands::nft::run_inspect(config, cli.output, &token_id).await
            }
        },
        Commands::Config(command) => match command {
            ConfigCommands::Init {
                path,
                network,
                force,
            } => commands::config::run_init(path.or(cli.config.clone()), network, force).await,
            ConfigCommands::Edit { path, network } => {
                commands::config::run_edit(path.or(cli.config.clone()), network).await
            }
            ConfigCommands::Validate { path, network, fix } => {
                commands::config::run_validate(
                    path.or(cli.config.clone()),
                    network,
                    cli.output,
                    fix,
                )
                .await
            }
            ConfigCommands::Show { path, network } => {
                commands::config::run_show(path.or(cli.config.clone()), network, cli.output).await
            }
        },
        Commands::Whois { name } => commands::whois::run_whois(config, cli.output, &name).await,
        Commands::Portfolio {
            owner,
            batch_size,
            limit,
            page,
        } => {
            let options = commands::portfolio::PortfolioOptions {
                batch_size,
                limit,
                page,
            };
            commands::portfolio::run_portfolio(config, cli.output, &owner, options).await
        }
        Commands::RenewalCheck {
            owner,
            warn_days,
            auto_renew,
            batch_size,
            limit,
            page,
        } => {
            let options = commands::portfolio::PortfolioOptions {
                batch_size,
                limit,
                page,
            };
            commands::renewal_check::run_renewal_check(
                config,
                cli.output,
                cli.dry_run,
                &owner,
                warn_days,
                auto_renew,
                options,
            )
            .await
        }
        Commands::Quote { name, years } => {
            commands::quote::run_quote(config, cli.output, &name, years).await
        }
        Commands::Availability { name } => {
            commands::quote::run_availability(config, cli.output, &name).await
        }
        Commands::Healthcheck => commands::healthcheck::run_healthcheck(config, cli.output).await,
        Commands::Bulk(sub) => match sub {
            BulkCommands::Register { file } => {
                commands::bulk::run_bulk_register(config, &file, cli.dry_run).await
            }
            BulkCommands::Renew { file } => {
                commands::bulk::run_bulk_renew(config, &file, cli.dry_run).await
            }
        },
        Commands::Completions(_) => unreachable!("handled above"),
        Commands::Watch(sub) => commands::watch::run(config, sub).await,
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let context = error_context(&cli.command);
    let output = cli.output;
    if let Err(e) = run_with_cli(cli).await {
        error::handle_error(&e, output, &context, false);
        process::exit(1);
    }
}

fn error_context(command: &Commands) -> error::ErrorContext {
    match command {
        Commands::Register { name, .. } => error::ErrorContext {
            domain: error::ErrorDomain::Registrar,
            subject: name.clone(),
            subject_kind: error::SubjectKind::Name,
            command: "register",
        },
        Commands::Resolve { name } => error::ErrorContext {
            domain: error::ErrorDomain::Resolver,
            subject: Some(name.clone()),
            subject_kind: error::SubjectKind::Name,
            command: "resolve",
        },
        Commands::ReverseResolve { address } => error::ErrorContext {
            domain: error::ErrorDomain::Resolver,
            subject: Some(address.clone()),
            subject_kind: error::SubjectKind::Address,
            command: "reverse-resolve",
        },
        Commands::Text(sub) => match sub {
            TextCommand::Get { name, .. } | TextCommand::Set { name, .. } => error::ErrorContext {
                domain: error::ErrorDomain::Resolver,
                subject: Some(name.clone()),
                subject_kind: error::SubjectKind::Name,
                command: "text",
            },
        },
        Commands::Transfer { name, .. } => error::ErrorContext {
            domain: error::ErrorDomain::Registry,
            subject: Some(name.clone()),
            subject_kind: error::SubjectKind::Name,
            command: "transfer",
        },
        Commands::Renew { name, .. } => error::ErrorContext {
            domain: error::ErrorDomain::Registrar,
            subject: Some(name.clone()),
            subject_kind: error::SubjectKind::Name,
            command: "renew",
        },
        Commands::Auction(sub) => match sub {
            AuctionCommands::Create { name, .. }
            | AuctionCommands::Bid { name, .. }
            | AuctionCommands::Inspect { name }
            | AuctionCommands::Settle { name, .. }
            | AuctionCommands::Export { name, .. }
            | AuctionCommands::Import { name, .. } => error::ErrorContext {
                domain: error::ErrorDomain::Auction,
                subject: Some(name.clone()),
                subject_kind: error::SubjectKind::Name,
                command: "auction",
            },
        },
        Commands::Bridge(sub) => match sub {
            BridgeCommands::Register { chain }
            | BridgeCommands::Inspect { chain }
            | BridgeCommands::Payload { chain, .. } => error::ErrorContext {
                domain: error::ErrorDomain::Bridge,
                subject: Some(chain.clone()),
                subject_kind: error::SubjectKind::Chain,
                command: "bridge",
            },
            BridgeCommands::TestVectors => error::ErrorContext {
                domain: error::ErrorDomain::Bridge,
                subject: None,
                subject_kind: error::SubjectKind::Unknown,
                command: "bridge",
            },
        },
        Commands::Subdomain(sub) => match sub {
            SubdomainCommands::RegisterParent { parent, .. } => error::ErrorContext {
                domain: error::ErrorDomain::Subdomain,
                subject: Some(parent.clone()),
                subject_kind: error::SubjectKind::Name,
                command: "subdomain",
            },
            SubdomainCommands::AddController { parent, .. } => error::ErrorContext {
                domain: error::ErrorDomain::Subdomain,
                subject: Some(parent.clone()),
                subject_kind: error::SubjectKind::Name,
                command: "subdomain",
            },
            SubdomainCommands::Create { label, parent, .. } => error::ErrorContext {
                domain: error::ErrorDomain::Subdomain,
                subject: Some(format!("{label}.{parent}")),
                subject_kind: error::SubjectKind::Name,
                command: "subdomain",
            },
            SubdomainCommands::Transfer { fqdn, .. } => error::ErrorContext {
                domain: error::ErrorDomain::Subdomain,
                subject: Some(fqdn.clone()),
                subject_kind: error::SubjectKind::Name,
                command: "subdomain",
            },
        },
        Commands::Nft(NftCommands::Inspect { token_id }) => error::ErrorContext {
            domain: error::ErrorDomain::Nft,
            subject: Some(token_id.clone()),
            subject_kind: error::SubjectKind::TokenId,
            command: "nft",
        },
        Commands::Config(_) => error::ErrorContext {
            domain: error::ErrorDomain::General,
            subject: None,
            subject_kind: error::SubjectKind::Unknown,
            command: "config",
        },
        Commands::Watch(_) => error::ErrorContext {
            domain: error::ErrorDomain::Registry,
            subject: None,
            subject_kind: error::SubjectKind::Unknown,
            command: "watch",
        },
        Commands::Whois { name } => error::ErrorContext {
            domain: error::ErrorDomain::Registry,
            subject: Some(name.clone()),
            subject_kind: error::SubjectKind::Name,
            command: "whois",
        },
        Commands::Portfolio { owner, .. } => error::ErrorContext {
            domain: error::ErrorDomain::Registry,
            subject: Some(owner.clone()),
            subject_kind: error::SubjectKind::Address,
            command: "portfolio",
        },
        Commands::Quote { name, .. } => error::ErrorContext {
            domain: error::ErrorDomain::Registrar,
            subject: Some(name.clone()),
            subject_kind: error::SubjectKind::Name,
            command: "quote",
        },
        Commands::RenewalCheck { owner, .. } => error::ErrorContext {
            domain: error::ErrorDomain::Registry,
            subject: Some(owner.clone()),
            subject_kind: error::SubjectKind::Address,
            command: "renewal-check",
        },
        Commands::Bulk(sub) => match sub {
            BulkCommands::Register { file } => error::ErrorContext {
                domain: error::ErrorDomain::Registrar,
                subject: Some(file.display().to_string()),
                subject_kind: error::SubjectKind::File,
                command: "bulk register",
            },
            BulkCommands::Renew { file } => error::ErrorContext {
                domain: error::ErrorDomain::Registrar,
                subject: Some(file.display().to_string()),
                subject_kind: error::SubjectKind::File,
                command: "bulk renew",
            },
        },
        Commands::Availability { name } => error::ErrorContext {
            domain: error::ErrorDomain::Registry,
            subject: Some(name.clone()),
            subject_kind: error::SubjectKind::Name,
            command: "availability",
        },
        Commands::Healthcheck => error::ErrorContext {
            domain: error::ErrorDomain::General,
            subject: None,
            subject_kind: error::SubjectKind::Unknown,
            command: "healthcheck",
        },
        Commands::Completions(_) => error::ErrorContext {
            domain: error::ErrorDomain::General,
            subject: None,
            subject_kind: error::SubjectKind::Unknown,
            command: "completions",
        },
        Commands::Migrate { .. } => error::ErrorContext {
            domain: error::ErrorDomain::General,
            subject: None,
            subject_kind: error::SubjectKind::Unknown,
            command: "migrate",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::{Network, NetworkConfig};

    fn config_with_all_contracts() -> NetworkConfig {
        NetworkConfig {
            network: Network::Testnet,
            rpc_url: "https://soroban-testnet.stellar.org".to_string(),
            network_passphrase: "Test SDF Network ; September 2015".to_string(),
            registry_contract_id: Some("REGISTRY111".to_string()),
            registrar_contract_id: Some("REGISTRAR111".to_string()),
            resolver_contract_id: Some("RESOLVER111".to_string()),
            auction_contract_id: Some("AUCTION111".to_string()),
            bridge_contract_id: Some("BRIDGE111".to_string()),
            subdomain_contract_id: Some("SUBDOMAIN111".to_string()),
            nft_contract_id: Some("NFT111".to_string()),
            config_path: None,
        }
    }

    fn config_with_no_contracts() -> NetworkConfig {
        NetworkConfig {
            network: Network::Testnet,
            rpc_url: "https://soroban-testnet.stellar.org".to_string(),
            network_passphrase: "Test SDF Network ; September 2015".to_string(),
            registry_contract_id: None,
            registrar_contract_id: None,
            resolver_contract_id: None,
            auction_contract_id: None,
            bridge_contract_id: None,
            subdomain_contract_id: None,
            nft_contract_id: None,
            config_path: None,
        }
    }

    // --- register ---

    #[test]
    fn register_rejects_irrelevant_resolver_flag() {
        let cmd = Commands::Register {
            name: Some("test.xlm".to_string()),
            owner: Some("GDRA111".to_string()),
            interactive: false,
            signer: None,
        };
        let overrides = ContractOverrides {
            resolver_contract_id: Some("RESOLVER111".to_string()),
            ..Default::default()
        };
        let result = validate_contract_policy(&cmd, &overrides, &config_with_all_contracts());
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("resolver-contract-id"),
            "expected resolver-contract-id in: {msg}"
        );
        assert!(msg.contains("register"), "expected 'register' in: {msg}");
    }

    #[test]
    fn register_accepts_registry_flag() {
        let cmd = Commands::Register {
            name: Some("test.xlm".to_string()),
            owner: Some("GDRA111".to_string()),
            interactive: false,
            signer: None,
        };
        let overrides = ContractOverrides {
            registry_contract_id: Some("REGISTRY111".to_string()),
            ..Default::default()
        };
        let result = validate_contract_policy(&cmd, &overrides, &config_with_all_contracts());
        assert!(
            result.is_ok(),
            "registry-contract-id should be allowed for register"
        );
    }

    #[test]
    fn register_fails_when_registrar_is_missing() {
        let cmd = Commands::Register {
            name: Some("test.xlm".to_string()),
            owner: Some("GDRA111".to_string()),
            interactive: false,
            signer: None,
        };
        let result = validate_contract_policy(
            &cmd,
            &ContractOverrides::default(),
            &config_with_no_contracts(),
        );
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("registrar-contract-id"),
            "expected registrar-contract-id in: {msg}"
        );
    }

    #[test]
    fn register_passes_with_only_registrar_flag() {
        let cmd = Commands::Register {
            name: Some("test.xlm".to_string()),
            owner: Some("GDRA111".to_string()),
            interactive: false,
            signer: None,
        };
        let overrides = ContractOverrides {
            registrar_contract_id: Some("REGISTRAR111".to_string()),
            ..Default::default()
        };
        let mut cfg = config_with_no_contracts();
        cfg.registrar_contract_id = Some("REGISTRAR111".to_string());
        let result = validate_contract_policy(&cmd, &overrides, &cfg);
        assert!(result.is_ok(), "unexpected error: {:?}", result.err());
    }

    // --- resolve ---

    #[test]
    fn resolve_accepts_registry_flag() {
        let cmd = Commands::Resolve {
            name: "test.xlm".to_string(),
        };
        let overrides = ContractOverrides {
            registry_contract_id: Some("REGISTRY111".to_string()),
            ..Default::default()
        };
        let result = validate_contract_policy(&cmd, &overrides, &config_with_all_contracts());
        assert!(
            result.is_ok(),
            "registry-contract-id should be allowed for resolve"
        );
    }

    #[test]
    fn resolve_fails_when_resolver_is_missing() {
        let cmd = Commands::Resolve {
            name: "test.xlm".to_string(),
        };
        let result = validate_contract_policy(
            &cmd,
            &ContractOverrides::default(),
            &config_with_no_contracts(),
        );
        assert!(result.is_err());
    }

    // --- transfer ---

    #[test]
    fn transfer_rejects_irrelevant_registrar_flag() {
        let cmd = Commands::Transfer {
            name: "test.xlm".to_string(),
            new_owner: "GDRANEW".to_string(),
            signer: None,
        };
        let overrides = ContractOverrides {
            registrar_contract_id: Some("REGISTRAR111".to_string()),
            ..Default::default()
        };
        let result = validate_contract_policy(&cmd, &overrides, &config_with_all_contracts());
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("registrar-contract-id"),
            "expected registrar-contract-id in: {msg}"
        );
        assert!(msg.contains("transfer"), "expected 'transfer' in: {msg}");
    }

    #[test]
    fn transfer_fails_when_registry_is_missing() {
        let cmd = Commands::Transfer {
            name: "test.xlm".to_string(),
            new_owner: "GDRANEW".to_string(),
            signer: None,
        };
        let result = validate_contract_policy(
            &cmd,
            &ContractOverrides::default(),
            &config_with_no_contracts(),
        );
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("registry-contract-id"),
            "expected registry-contract-id in: {msg}"
        );
    }

    // --- quote ---

    #[test]
    fn quote_accepts_registry_flag() {
        let cmd = Commands::Quote {
            name: "test".to_string(),
            years: 1,
        };
        let overrides = ContractOverrides {
            registry_contract_id: Some("REGISTRY111".to_string()),
            ..Default::default()
        };
        let result = validate_contract_policy(&cmd, &overrides, &config_with_all_contracts());
        assert!(
            result.is_ok(),
            "registry-contract-id should be allowed for quote"
        );
    }

    // --- availability ---

    #[test]
    fn availability_rejects_irrelevant_registrar_flag() {
        let cmd = Commands::Availability {
            name: "test.xlm".to_string(),
        };
        let overrides = ContractOverrides {
            registrar_contract_id: Some("REGISTRAR111".to_string()),
            ..Default::default()
        };
        let result = validate_contract_policy(&cmd, &overrides, &config_with_all_contracts());
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(
            msg.contains("registrar-contract-id"),
            "expected registrar-contract-id in: {msg}"
        );
    }

    #[test]
    fn availability_passes_with_no_contracts_configured() {
        let cmd = Commands::Availability {
            name: "test.xlm".to_string(),
        };
        let result = validate_contract_policy(
            &cmd,
            &ContractOverrides::default(),
            &config_with_no_contracts(),
        );
        assert!(
            result.is_ok(),
            "availability needs no required contracts: {:?}",
            result.err()
        );
    }

    // --- healthcheck ---

    #[test]
    fn healthcheck_allows_all_contract_flags() {
        let cmd = Commands::Healthcheck;
        let overrides = ContractOverrides {
            registry_contract_id: Some("REGISTRY111".to_string()),
            registrar_contract_id: Some("REGISTRAR111".to_string()),
            resolver_contract_id: Some("RESOLVER111".to_string()),
            auction_contract_id: Some("AUCTION111".to_string()),
            bridge_contract_id: Some("BRIDGE111".to_string()),
            subdomain_contract_id: Some("SUBDOMAIN111".to_string()),
            nft_contract_id: Some("NFT111".to_string()),
        };
        let result = validate_contract_policy(&cmd, &overrides, &config_with_no_contracts());
        assert!(
            result.is_ok(),
            "healthcheck should accept any contract flag: {:?}",
            result.err()
        );
    }

    #[test]
    fn healthcheck_passes_with_no_contracts_configured() {
        let cmd = Commands::Healthcheck;
        let result = validate_contract_policy(
            &cmd,
            &ContractOverrides::default(),
            &config_with_no_contracts(),
        );
        assert!(
            result.is_ok(),
            "healthcheck requires no contracts: {:?}",
            result.err()
        );
    }
}

fn validate_contract_policy(
    command: &Commands,
    overrides: &ContractOverrides,
    config: &config::NetworkConfig,
) -> Result<(), String> {
    let (command_name, allowed, required): (&str, &[ContractKind], &[ContractKind]) = match command
    {
        Commands::Register { .. } => (
            "register",
            &[ContractKind::Registrar, ContractKind::Registry],
            &[ContractKind::Registrar],
        ),
        Commands::Resolve { .. } => (
            "resolve",
            &[ContractKind::Resolver, ContractKind::Registry],
            &[ContractKind::Resolver],
        ),
        Commands::ReverseResolve { .. } => (
            "reverse-resolve",
            &[ContractKind::Resolver, ContractKind::Registry],
            &[ContractKind::Resolver],
        ),
        Commands::Text(_) => (
            "text",
            &[ContractKind::Resolver, ContractKind::Registry],
            &[ContractKind::Resolver],
        ),
        Commands::Transfer { .. } => (
            "transfer",
            &[ContractKind::Registry],
            &[ContractKind::Registry],
        ),
        Commands::Renew { .. } => (
            "renew",
            &[ContractKind::Registrar, ContractKind::Registry],
            &[ContractKind::Registrar],
        ),
        Commands::Auction(_) => (
            "auction",
            &[ContractKind::Auction, ContractKind::Registry],
            &[ContractKind::Auction],
        ),
        Commands::Completions(_) => ("completions", &[], &[]),
        Commands::Bridge(_) => (
            "bridge",
            &[ContractKind::Bridge, ContractKind::Registry],
            &[ContractKind::Bridge],
        ),
        Commands::Subdomain(_) => (
            "subdomain",
            &[ContractKind::Subdomain, ContractKind::Registry],
            &[ContractKind::Subdomain],
        ),
        Commands::Nft(_) => (
            "nft",
            &[ContractKind::Nft, ContractKind::Registry],
            &[ContractKind::Nft],
        ),
        Commands::Config(_) => ("config", &[], &[]),
        Commands::Watch(_) => ("watch", &[ContractKind::Registry], &[]),
        Commands::Whois { .. } => (
            "whois",
            &[ContractKind::Registry, ContractKind::Resolver],
            &[ContractKind::Registry],
        ),
        Commands::Portfolio { .. } => (
            "portfolio",
            &[ContractKind::Registry, ContractKind::Resolver],
            &[ContractKind::Registry],
        ),
        // Registrar is only required at runtime when `--auto-renew` is set
        // (checked in `run_renewal_check`), so it's allowed but not required here.
        Commands::RenewalCheck { .. } => (
            "renewal-check",
            &[
                ContractKind::Registry,
                ContractKind::Resolver,
                ContractKind::Registrar,
            ],
            &[ContractKind::Registry],
        ),
        // Quote and Availability are read-only; registrar is needed for pricing.
        Commands::Quote { .. } => (
            "quote",
            &[ContractKind::Registrar, ContractKind::Registry],
            &[ContractKind::Registrar],
        ),
        Commands::Availability { .. } => ("availability", &[ContractKind::Registry], &[]),
        // Healthcheck is purely informational: all contract flags are allowed
        // (they are reflected in the output) and none are required.
        Commands::Migrate { .. } => ("migrate", &[], &[]),
        Commands::Healthcheck => (
            "healthcheck",
            &[
                ContractKind::Registry,
                ContractKind::Registrar,
                ContractKind::Resolver,
                ContractKind::Auction,
                ContractKind::Bridge,
                ContractKind::Subdomain,
                ContractKind::Nft,
            ],
            &[],
        ),
        Commands::Bulk(sub) => match sub {
            BulkCommands::Register { .. } => (
                "bulk register",
                &[ContractKind::Registrar, ContractKind::Registry],
                &[ContractKind::Registrar],
            ),
            BulkCommands::Renew { .. } => (
                "bulk renew",
                &[ContractKind::Registrar, ContractKind::Registry],
                &[ContractKind::Registrar],
            ),
        },
    };

    for kind in overrides.provided_kinds() {
        if !allowed.contains(&kind) {
            return Err(format!(
                "`--{}` cannot be used with `{command_name}`",
                kind.flag_name()
            ));
        }
    }

    for kind in required {
        if config.contract_id(*kind).is_none() {
            return Err(format!(
                "`{command_name}` requires {}. Set `--{}`, `{}`, or the config file value.",
                kind.display_name(),
                kind.flag_name(),
                kind.env_var()
            ));
        }
    }

    Ok(())
}
