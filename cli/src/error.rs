use crate::output::{emit_error, OutputFormat};
use anyhow::Error;
use chrono::Utc;
use dirs::home_dir;
use regex::Regex;
use serde_json::json;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use xlm_ns_sdk::errors::{ContractErrorCode, SdkError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorDomain {
    Registrar,
    Registry,
    Resolver,
    Subdomain,
    Bridge,
    Auction,
    Nft,
    General,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubjectKind {
    Name,
    Address,
    TokenId,
    Chain,
    File,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct ErrorContext {
    pub domain: ErrorDomain,
    pub subject: Option<String>,
    pub subject_kind: SubjectKind,
    pub command: &'static str,
}

#[derive(Clone, Debug)]
pub struct FriendlyError {
    pub summary: String,
    pub suggestion: String,
    pub docs: Vec<&'static str>,
    pub technical: String,
}

impl FriendlyError {
    pub fn human_message(&self, verbose: bool) -> String {
        let mut message = format!("Error: {}. Suggestion: {}.", self.summary, self.suggestion);
        if !self.docs.is_empty() {
            message.push_str(" Docs: ");
            message.push_str(&self.docs.join(", "));
            message.push('.');
        }
        if verbose {
            message.push_str(" Technical details: ");
            message.push_str(&self.technical);
            message.push('.');
        }
        message
    }

    pub fn json_payload(&self, verbose: bool) -> serde_json::Value {
        let mut payload = json!({
            "error": self.summary,
            "suggestion": self.suggestion,
            "docs": self.docs,
        });
        if verbose {
            payload["technical"] = json!(self.technical);
        }
        payload
    }
}

pub fn handle_error(err: &Error, output: OutputFormat, context: &ErrorContext, verbose: bool) {
    let report = classify_error(err, context);
    let human = report.human_message(verbose);
    emit_error(output, &human, report.json_payload(verbose));
    log_error(err, context, &report);
}

fn log_error(err: &Error, context: &ErrorContext, report: &FriendlyError) {
    let log_path = error_log_path();
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut file = match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(file) => file,
        Err(_) => return,
    };

    let timestamp = Utc::now().to_rfc3339();
    let subject = context.subject.as_deref().unwrap_or("[none]");
    let _ = writeln!(
        file,
        "[{timestamp}] command={} domain={:?} subject_kind={:?} subject={subject}",
        context.command, context.domain, context.subject_kind
    );
    let _ = writeln!(file, "summary={}", report.summary);
    let _ = writeln!(file, "suggestion={}", report.suggestion);
    if !report.docs.is_empty() {
        let _ = writeln!(file, "docs={}", report.docs.join(", "));
    }
    let _ = writeln!(file, "technical={:#?}", err);
    let _ = writeln!(file);
}

fn classify_error(err: &Error, context: &ErrorContext) -> FriendlyError {
    if let Some(sdk_error) = err.downcast_ref::<SdkError>() {
        return classify_sdk_error(sdk_error, context, Some(err));
    }

    let text = err.to_string();
    if let Some(report) = classify_text(&text, context, Some(err)) {
        return report;
    }

    FriendlyError {
        summary: "The command failed unexpectedly".to_string(),
        suggestion: "Run again with `--verbose` and check `~/.xlm-ens/error.log` for details"
            .to_string(),
        docs: vec!["docs/contract-specs.md"],
        technical: technical_chain(err),
    }
}

fn classify_sdk_error(
    sdk_error: &SdkError,
    context: &ErrorContext,
    err: Option<&Error>,
) -> FriendlyError {
    match sdk_error {
        SdkError::InvalidRequest(message) => classify_text(message, context, err).unwrap_or_else(|| FriendlyError {
            summary: format!("The request was invalid: {message}"),
            suggestion: "Check the command arguments and try again".to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, message),
        }),
        SdkError::Transport(message) => transport_error(message, err),
        SdkError::Ingestion(message) => FriendlyError {
            summary: "The RPC ingestion layer returned an error".to_string(),
            suggestion: "Verify the RPC endpoint is healthy and retry".to_string(),
            docs: vec!["docs/sdk-integration-tests.md"],
            technical: technical_chain_or(err, message),
        },
        SdkError::ContractError(code) => contract_error(*code, context, err),
        SdkError::NetworkPassphraseMismatch {
            configured,
            rpc_reported,
        } => FriendlyError {
            summary: "The configured network passphrase does not match the RPC server".to_string(),
            suggestion: "Double-check `--network` and `--network-passphrase`, then retry".to_string(),
            docs: vec!["docs/sdk-integration-tests.md"],
            technical: technical_chain_or(
                err,
                &format!("configured={configured:?}, rpc_reported={rpc_reported:?}"),
            ),
        },
        SdkError::TransactionPassphraseMismatch {
            configured,
            in_transaction,
        } => FriendlyError {
            summary: "The transaction passphrase does not match the configured network".to_string(),
            suggestion: "Rebuild the transaction against the correct network and submit again"
                .to_string(),
            docs: vec!["docs/sdk-integration-tests.md"],
            technical: technical_chain_or(
                err,
                &format!("configured={configured:?}, in_transaction={in_transaction:?}"),
            ),
        },
        SdkError::ContractInvocationFailed {
            operation,
            reason,
            tx_hash,
        } => {
            if let Some(report) = classify_text(reason, context, err) {
                return report;
            }
            FriendlyError {
                summary: format!("{operation} failed"),
                suggestion: "Run with `--verbose` and inspect the debug log for the contract reason"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_with_tx(err, reason, tx_hash.as_deref()),
            }
        }
        SdkError::SimulationFailed { operation, reason } => {
            if let Some(report) = classify_text(reason, context, err) {
                return report;
            }
            FriendlyError {
                summary: format!("{operation} simulation failed"),
                suggestion: "Check the input values, account balance, and signer permissions, then retry"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, reason),
            }
        }
        SdkError::InsufficientFee {
            operation,
            required,
            available,
        } => FriendlyError {
            summary: format!(
                "Insufficient balance for {operation}: need {required} stroops but only {available} are available"
            ),
            suggestion: "Fund the source account, then rerun the command".to_string(),
            docs: vec!["docs/sdk-quickstart.md"],
            technical: technical_chain_or(
                err,
                &format!("required={required}, available={available}"),
            ),
        },
        SdkError::TransactionTimeout {
            operation,
            ledger_submitted,
        } => FriendlyError {
            summary: format!("{operation} timed out after submission at ledger {ledger_submitted}"),
            suggestion: "Retry the command after the network catches up".to_string(),
            docs: vec!["docs/sdk-integration-tests.md"],
            technical: technical_chain_or(err, &format!("ledger_submitted={ledger_submitted}")),
        },
        SdkError::SigningFailed { operation, source } => FriendlyError {
            summary: format!("{operation} signing failed"),
            suggestion: "Recheck the signer profile or hardware wallet connection".to_string(),
            docs: vec!["README.md"],
            technical: technical_chain_or(err, &source.to_string()),
        },
        SdkError::RateLimitExceeded(details) => FriendlyError {
            summary: format!("The RPC rate limit was exceeded after {} retries", details.retries),
            suggestion: "Wait a moment, reduce the batch size, and retry".to_string(),
            docs: vec!["RATE_LIMITING_QUICK_REFERENCE.md"],
            technical: technical_chain_or(err, &details.to_string()),
        },
    }
}

fn classify_text(text: &str, context: &ErrorContext, err: Option<&Error>) -> Option<FriendlyError> {
    let lower = text.to_ascii_lowercase();

    if lower.contains("cannot be used with") {
        return Some(FriendlyError {
            summary: text.trim().trim_matches('.').to_string(),
            suggestion: "Remove the incompatible flag or switch to the matching command"
                .to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, text),
        });
    }

    if lower.contains("requires") && lower.contains("contract") {
        return Some(FriendlyError {
            summary: text.trim().trim_matches('.').to_string(),
            suggestion:
                "Set the missing contract ID with a flag, environment variable, or config file"
                    .to_string(),
            docs: vec!["docs/sdk-quickstart.md"],
            technical: technical_chain_or(err, text),
        });
    }

    if lower.contains("not configured") {
        return Some(FriendlyError {
            summary: text.trim().trim_matches('.').to_string(),
            suggestion:
                "Create or update your config file, or pass the missing value on the command line"
                    .to_string(),
            docs: vec!["docs/sdk-quickstart.md"],
            technical: technical_chain_or(err, text),
        });
    }

    if lower.contains("failed to deserialize") || lower.contains("failed to open file") {
        return Some(FriendlyError {
            summary: text.trim().trim_matches('.').to_string(),
            suggestion: "Check the file path and ensure the input is valid JSON or CSV".to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, text),
        });
    }

    if lower.contains("healthcheck reported a degraded status") {
        return Some(FriendlyError {
            summary: "The healthcheck found a degraded dependency".to_string(),
            suggestion: "Run `xlm-ns healthcheck --verbose` to see which check failed".to_string(),
            docs: vec!["docs/sdk-integration-tests.md"],
            technical: technical_chain_or(err, text),
        });
    }

    if let Some(code) = extract_contract_code(text) {
        return Some(contract_error(code, context, err));
    }

    if lower.contains("already registered") {
        return Some(contract_error_for_domain(
            context.domain,
            4,
            context,
            err,
            Some("That name is already registered"),
        ));
    }

    if lower.contains("no primary name") {
        return Some(FriendlyError {
            summary: format!("{} does not have a primary name set", subject_name(context)),
            suggestion: "Set a primary name record or inspect the registry entry for this address"
                .to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, text),
        });
    }

    if lower.contains("not registered") || lower.contains("not found") {
        return Some(contract_error_for_domain(
            context.domain,
            2,
            context,
            err,
            Some("The requested record was not found"),
        ));
    }

    if lower.contains("expired") {
        return Some(contract_error_for_domain(
            context.domain,
            3,
            context,
            err,
            Some("The name or record has expired"),
        ));
    }

    if lower.contains("insufficient balance") || lower.contains("insufficient fee") {
        return Some(FriendlyError {
            summary: "Insufficient balance to pay the required fee".to_string(),
            suggestion: "Fund the account with more XLM and retry".to_string(),
            docs: vec!["docs/sdk-quickstart.md"],
            technical: technical_chain_or(err, text),
        });
    }

    None
}

fn contract_error(
    code: ContractErrorCode,
    context: &ErrorContext,
    err: Option<&Error>,
) -> FriendlyError {
    match code {
        ContractErrorCode::NameNotFound => FriendlyError {
            summary: format!("{} was not found", subject_name(context)),
            suggestion:
                "Use `xlm-ns whois <name>` or `xlm-ns resolve <name>` to confirm the current record"
                    .to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, "contract error: NameNotFound"),
        },
        ContractErrorCode::NotOwner => FriendlyError {
            summary: format!("{} is owned by a different account", subject_name(context)),
            suggestion: "Use the current owner or the admin signer, then retry".to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, "contract error: NotOwner"),
        },
        ContractErrorCode::Expired => FriendlyError {
            summary: format!("{} has expired", subject_name(context)),
            suggestion: "Renew it during the grace period, or register it again if it is claimable"
                .to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, "contract error: Expired"),
        },
        ContractErrorCode::InvalidLabel => FriendlyError {
            summary: format!("{} is not a valid XLM name label", subject_name(context)),
            suggestion: "Use a lowercase label with only supported characters".to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, "contract error: InvalidLabel"),
        },
        ContractErrorCode::Other => FriendlyError {
            summary: "The contract returned an unknown error".to_string(),
            suggestion:
                "Run again with `--verbose` and inspect the log file for the contract reason"
                    .to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: technical_chain_or(err, "contract error: Other"),
        },
    }
}

fn contract_error_for_domain(
    domain: ErrorDomain,
    code: u32,
    context: &ErrorContext,
    err: Option<&Error>,
    default_summary: Option<&str>,
) -> FriendlyError {
    match domain {
        ErrorDomain::Registrar => match code {
            1 => FriendlyError {
                summary: format!("{} needs a higher registration fee", subject_name(context)),
                suggestion:
                    "Fund the account, then rerun `xlm-ns quote <name> 1` to verify the cost"
                        .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 1"),
            },
            2 => FriendlyError {
                summary: format!("{} is not registered", subject_name(context)),
                suggestion: "Use `xlm-ns whois <name>` to inspect the current record".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 2"),
            },
            3 => FriendlyError {
                summary: format!(
                    "{} is not renewable in its current state",
                    subject_name(context)
                ),
                suggestion: "Renew it during the grace period, or wait until it becomes claimable"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 3"),
            },
            4 => FriendlyError {
                summary: format!("{} is already registered", subject_name(context)),
                suggestion: "Use `xlm-ns whois <name>` to inspect the current owner and expiry"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 4"),
            },
            5 => FriendlyError {
                summary: format!("{} is reserved", subject_name(context)),
                suggestion:
                    "Choose another label or remove the reservation if you control the admin"
                        .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 5"),
            },
            6 => FriendlyError {
                summary: "The signer is not authorized for this registrar action".to_string(),
                suggestion: "Switch to the owner or admin signer profile and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 6"),
            },
            7 => FriendlyError {
                summary: format!("{} failed validation", subject_name(context)),
                suggestion: "Check the label format and try again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 7"),
            },
            8 => FriendlyError {
                summary: format!(
                    "{} is claimable rather than renewable",
                    subject_name(context)
                ),
                suggestion: "Register it as a new name instead of renewing it".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 8"),
            },
            9 => FriendlyError {
                summary: "The registrar is not initialized".to_string(),
                suggestion: "Deploy or initialize the registrar contract, then retry".to_string(),
                docs: vec!["docs/sdk-quickstart.md"],
                technical: technical_chain_or(err, "contract error 9"),
            },
            10 => FriendlyError {
                summary: "The registrar was already initialized".to_string(),
                suggestion: "Skip initialization and use the existing contract state".to_string(),
                docs: vec!["docs/sdk-quickstart.md"],
                technical: technical_chain_or(err, "contract error 10"),
            },
            11 => FriendlyError {
                summary: "You hit the registrar rate limit".to_string(),
                suggestion: "Wait for the window to reset, or spread registrations out more evenly"
                    .to_string(),
                docs: vec!["RATE_LIMITING_QUICK_REFERENCE.md"],
                technical: technical_chain_or(err, "contract error 11"),
            },
            12 => FriendlyError {
                summary: "The registrar upgrade failed".to_string(),
                suggestion: "Verify the admin signer and wasm hash, then retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 12"),
            },
            _ => unknown_contract_error(context, err, default_summary),
        },
        ErrorDomain::Registry => match code {
            1 => FriendlyError {
                summary: format!("{} is already registered", subject_name(context)),
                suggestion: "Use `xlm-ns whois <name>` to inspect the current owner".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 1"),
            },
            2 => FriendlyError {
                summary: format!("{} was not found in the registry", subject_name(context)),
                suggestion: "Check the name spelling or try `xlm-ns resolve <name>`".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 2"),
            },
            3 => FriendlyError {
                summary: format!("{} is not yet claimable", subject_name(context)),
                suggestion: "Wait until the grace period ends, then try again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 3"),
            },
            4 => FriendlyError {
                summary: format!("{} is not active right now", subject_name(context)),
                suggestion: "Renew the name or wait until it becomes active again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 4"),
            },
            5 => FriendlyError {
                summary: "The registry action is unauthorized".to_string(),
                suggestion: "Use the current owner or admin signer profile".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 5"),
            },
            6 => FriendlyError {
                summary: "The metadata URI is too long".to_string(),
                suggestion: "Shorten the metadata URI and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 6"),
            },
            7 => FriendlyError {
                summary: format!("{} failed validation", subject_name(context)),
                suggestion: "Check the name format and registry inputs".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 7"),
            },
            8 => FriendlyError {
                summary: "The expiry timestamp is invalid".to_string(),
                suggestion: "Use a future expiry time and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 8"),
            },
            9 => FriendlyError {
                summary: "The grace period value is invalid".to_string(),
                suggestion: "Choose a grace period within the supported bounds".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 9"),
            },
            10 => FriendlyError {
                summary: "The registry upgrade failed".to_string(),
                suggestion: "Verify the admin signer and try the upgrade again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 10"),
            },
            11 => FriendlyError {
                summary: format!("{} is locked", subject_name(context)),
                suggestion: "Wait for the lock to expire or ask the admin to remove it".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 11"),
            },
            _ => unknown_contract_error(context, err, default_summary),
        },
        ErrorDomain::Resolver => match code {
            1 => FriendlyError {
                summary: format!("{} failed validation", subject_name(context)),
                suggestion: "Check the name, text key, and resolver inputs".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 1"),
            },
            2 => FriendlyError {
                summary: format!("{} has no resolver record", subject_name(context)),
                suggestion: "Use `xlm-ns whois <name>` or register a resolver record first"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 2"),
            },
            3 => FriendlyError {
                summary: "The resolver action is unauthorized".to_string(),
                suggestion: "Use the owner signer profile and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 3"),
            },
            4 => FriendlyError {
                summary: "Too many text records are attached to this name".to_string(),
                suggestion: "Remove some records or split them across fewer updates".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 4"),
            },
            5 => FriendlyError {
                summary: "The resolver is not initialized".to_string(),
                suggestion: "Deploy or initialize the resolver contract, then retry".to_string(),
                docs: vec!["docs/sdk-quickstart.md"],
                technical: technical_chain_or(err, "contract error 5"),
            },
            6 => FriendlyError {
                summary: "The text record value is too long".to_string(),
                suggestion: "Shorten the text record value and try again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 6"),
            },
            7 => FriendlyError {
                summary: "The requested chain is not supported".to_string(),
                suggestion: "Pick one of the supported chains and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 7"),
            },
            8 => FriendlyError {
                summary: "The text record key is invalid".to_string(),
                suggestion: "Use a normalized, lowercase key".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 8"),
            },
            9 => FriendlyError {
                summary: "The batch payload is too large".to_string(),
                suggestion: "Split the request into smaller batches".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 9"),
            },
            10 => FriendlyError {
                summary: "The resolver upgrade failed".to_string(),
                suggestion: "Verify the admin signer and wasm hash, then retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 10"),
            },
            _ => unknown_contract_error(context, err, default_summary),
        },
        ErrorDomain::Subdomain => match code {
            1 => FriendlyError {
                summary: format!("{} failed validation", subject_name(context)),
                suggestion: "Check the parent domain and label format".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 1"),
            },
            2 => FriendlyError {
                summary: format!("{} has no registered parent domain", subject_name(context)),
                suggestion: "Register the parent domain first, then retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 2"),
            },
            3 => FriendlyError {
                summary: format!("{} already exists", subject_name(context)),
                suggestion: "Choose another subdomain label".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 3"),
            },
            4 => FriendlyError {
                summary: format!("{} was not found", subject_name(context)),
                suggestion: "Double-check the subdomain name and parent domain".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 4"),
            },
            5 => FriendlyError {
                summary: "The subdomain action is unauthorized".to_string(),
                suggestion: "Use the parent owner or authorized controller signer".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 5"),
            },
            6 => FriendlyError {
                summary: "The subdomain upgrade failed".to_string(),
                suggestion: "Verify the admin signer and try again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 6"),
            },
            7 => FriendlyError {
                summary: "The requested subdomain exceeds the allowed depth".to_string(),
                suggestion: "Use a shorter subdomain path".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 7"),
            },
            _ => unknown_contract_error(context, err, default_summary),
        },
        ErrorDomain::Bridge => match code {
            1 => FriendlyError {
                summary: format!("{} failed validation", subject_name(context)),
                suggestion: "Check the chain name and resolver address format".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 1"),
            },
            2 => FriendlyError {
                summary: format!("{} is not a supported chain", subject_name(context)),
                suggestion: "Use one of the registered chains or add a new supported-chain entry"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 2"),
            },
            3 => FriendlyError {
                summary: "The bridge upgrade failed".to_string(),
                suggestion: "Verify the admin signer and wasm hash, then retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 3"),
            },
            4 => FriendlyError {
                summary: "The bridge action is unauthorized".to_string(),
                suggestion: "Use the admin signer profile and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 4"),
            },
            5 => FriendlyError {
                summary: format!("{} was not found", subject_name(context)),
                suggestion: "Check the chain or route name and try again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 5"),
            },
            6 => FriendlyError {
                summary: format!("{} already exists", subject_name(context)),
                suggestion: "Choose a different chain name or update the existing route"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 6"),
            },
            _ => unknown_contract_error(context, err, default_summary),
        },
        ErrorDomain::Auction => match code {
            1 => FriendlyError {
                summary: format!("{} failed validation", subject_name(context)),
                suggestion: "Check the auction name, timestamps, and reserve price".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 1"),
            },
            2 => FriendlyError {
                summary: format!("{} already has an auction", subject_name(context)),
                suggestion: "Inspect the current auction or choose a different name".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 2"),
            },
            3 => FriendlyError {
                summary: format!("{} has no auction", subject_name(context)),
                suggestion: "Create an auction first, then retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 3"),
            },
            4 => FriendlyError {
                summary: format!("{} is already closed", subject_name(context)),
                suggestion: "Settle the auction or choose another name".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 4"),
            },
            5 => FriendlyError {
                summary: format!("{} has not started yet", subject_name(context)),
                suggestion: "Wait for the auction start time and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 5"),
            },
            6 => FriendlyError {
                summary: format!("{} has not ended yet", subject_name(context)),
                suggestion:
                    "Wait for the end time before settling or inspecting settlement results"
                        .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 6"),
            },
            7 => FriendlyError {
                summary: "The auction was already settled".to_string(),
                suggestion: "Inspect the final result or choose another auction".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 7"),
            },
            8 => FriendlyError {
                summary: "The bid is invalid".to_string(),
                suggestion: "Increase the bid amount and make sure it clears the reserve price"
                    .to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 8"),
            },
            9 => FriendlyError {
                summary: "The auction upgrade failed".to_string(),
                suggestion: "Verify the admin signer and wasm hash, then retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 9"),
            },
            10 => FriendlyError {
                summary: "A reentrancy guard blocked this auction operation".to_string(),
                suggestion: "Retry the operation after the in-flight transaction completes"
                    .to_string(),
                docs: vec!["docs/security/reentrancy-audit.md"],
                technical: technical_chain_or(err, "contract error 10"),
            },
            _ => unknown_contract_error(context, err, default_summary),
        },
        ErrorDomain::Nft => match code {
            1 => FriendlyError {
                summary: format!("{} is already minted", subject_name(context)),
                suggestion: "Use the existing token or choose a different token id".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 1"),
            },
            2 => FriendlyError {
                summary: format!("{} was not found", subject_name(context)),
                suggestion: "Check the token id and try again".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 2"),
            },
            3 => FriendlyError {
                summary: "You are not authorized to inspect or modify this NFT".to_string(),
                suggestion: "Use the token owner or admin signer profile and retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 3"),
            },
            4 => FriendlyError {
                summary: "The NFT upgrade failed".to_string(),
                suggestion: "Verify the admin signer and wasm hash, then retry".to_string(),
                docs: vec!["docs/contract-specs.md"],
                technical: technical_chain_or(err, "contract error 4"),
            },
            5 => FriendlyError {
                summary: "The NFT contract is not initialized".to_string(),
                suggestion: "Deploy or initialize the NFT contract, then retry".to_string(),
                docs: vec!["docs/sdk-quickstart.md"],
                technical: technical_chain_or(err, "contract error 5"),
            },
            _ => unknown_contract_error(context, err, default_summary),
        },
        ErrorDomain::General => unknown_contract_error(context, err, default_summary),
    }
}

fn unknown_contract_error(
    _context: &ErrorContext,
    err: Option<&Error>,
    default_summary: Option<&str>,
) -> FriendlyError {
    FriendlyError {
        summary: default_summary
            .unwrap_or("The contract returned an unknown error")
            .to_string(),
        suggestion: "Run again with `--verbose` and inspect the log file for the full reason"
            .to_string(),
        docs: vec!["docs/contract-specs.md"],
        technical: technical_chain_or(err, "contract error"),
    }
}

fn transport_error(message: &str, err: Option<&Error>) -> FriendlyError {
    let lower = message.to_ascii_lowercase();
    if lower.contains("429") || lower.contains("too many requests") || lower.contains("rate limit")
    {
        return FriendlyError {
            summary: "The RPC server rate-limited the request".to_string(),
            suggestion: "Wait a moment, reduce request frequency, and try again".to_string(),
            docs: vec!["RATE_LIMITING_QUICK_REFERENCE.md"],
            technical: technical_chain_or(err, message),
        };
    }
    if lower.contains("timeout") || lower.contains("timed out") {
        return FriendlyError {
            summary: "The RPC request timed out".to_string(),
            suggestion: "Check connectivity and retry the command".to_string(),
            docs: vec!["docs/sdk-integration-tests.md"],
            technical: technical_chain_or(err, message),
        };
    }
    if lower.contains("connection refused")
        || lower.contains("connection reset")
        || lower.contains("network unreachable")
        || lower.contains("dns")
    {
        return FriendlyError {
            summary: "The RPC server could not be reached".to_string(),
            suggestion: "Verify the RPC URL and network connectivity, then retry".to_string(),
            docs: vec!["docs/sdk-quickstart.md"],
            technical: technical_chain_or(err, message),
        };
    }

    FriendlyError {
        summary: "The RPC transport failed".to_string(),
        suggestion: "Retry the command or check the RPC endpoint".to_string(),
        docs: vec!["docs/sdk-quickstart.md"],
        technical: technical_chain_or(err, message),
    }
}

fn extract_contract_code(text: &str) -> Option<u32> {
    static CONTRACT_ERROR_RE: OnceLock<Regex> = OnceLock::new();
    let re = CONTRACT_ERROR_RE.get_or_init(|| {
        Regex::new(r"(?i)contract error[: ]+(\d+)").expect("contract error regex should compile")
    });
    re.captures(text)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<u32>().ok())
}

fn technical_chain(err: &Error) -> String {
    err.chain()
        .map(|cause| cause.to_string())
        .collect::<Vec<_>>()
        .join(" | ")
}

fn technical_chain_or(err: Option<&Error>, fallback: &str) -> String {
    err.map(technical_chain)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn technical_with_tx(err: Option<&Error>, reason: &str, tx_hash: Option<&str>) -> String {
    let mut pieces = vec![reason.to_string()];
    if let Some(tx_hash) = tx_hash {
        pieces.push(format!("tx_hash={tx_hash}"));
    }
    if let Some(err) = err {
        pieces.push(technical_chain(err));
    }
    pieces.join(" | ")
}

fn subject_name(context: &ErrorContext) -> String {
    let raw = context.subject.as_deref().unwrap_or("the requested name");
    match context.subject_kind {
        SubjectKind::Name => {
            if raw.contains('.') {
                raw.to_string()
            } else {
                format!("{raw}.xlm")
            }
        }
        _ => raw.to_string(),
    }
}

fn error_log_path() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".xlm-ens")
        .join("error.log")
}

impl fmt::Display for ErrorDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ErrorDomain::Registrar => "registrar",
            ErrorDomain::Registry => "registry",
            ErrorDomain::Resolver => "resolver",
            ErrorDomain::Subdomain => "subdomain",
            ErrorDomain::Bridge => "bridge",
            ErrorDomain::Auction => "auction",
            ErrorDomain::Nft => "nft",
            ErrorDomain::General => "general",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(domain: ErrorDomain, subject: Option<&str>, kind: SubjectKind) -> ErrorContext {
        ErrorContext {
            domain,
            subject: subject.map(|value| value.to_string()),
            subject_kind: kind,
            command: "test",
        }
    }

    #[test]
    fn sdk_contract_name_not_found_is_actionable() {
        let err = anyhow::anyhow!(SdkError::ContractError(ContractErrorCode::NameNotFound));
        let report = classify_error(
            &err,
            &context(ErrorDomain::Registry, Some("alice.xlm"), SubjectKind::Name),
        );

        assert!(report.summary.contains("alice.xlm"));
        assert!(report.suggestion.contains("whois"));
    }

    #[test]
    fn raw_contract_code_maps_to_domain_specific_message() {
        let err = anyhow::anyhow!("simulation failed: contract error 4");
        let report = classify_error(
            &err,
            &context(ErrorDomain::Registrar, Some("alice"), SubjectKind::Name),
        );

        assert!(report.summary.contains("already registered"));
        assert!(report.suggestion.contains("whois"));
    }

    #[test]
    fn friendly_message_includes_verbose_details() {
        let report = FriendlyError {
            summary: "Example failure".to_string(),
            suggestion: "Try again".to_string(),
            docs: vec!["docs/contract-specs.md"],
            technical: "technical details".to_string(),
        };

        let human = report.human_message(true);
        assert!(human.contains("Error: Example failure."));
        assert!(human.contains("Suggestion: Try again."));
        assert!(human.contains("Technical details: technical details."));
    }
}
