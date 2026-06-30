use crate::config::NetworkConfig;
use crate::signer::load_signing_key;
use futures::future::join_all;
use regex::Regex;
use reqwest::Client;
use serde::Serialize;
use std::env;
use std::fmt;

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct Validation {
    pub contract_id_format: Vec<ValidationResult>,
    pub rpc_connectivity: ValidationResult,
    pub network_passphrase: ValidationResult,
    pub signing_key: ValidationResult,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub name: String,
    pub status: ValidationStatus,
    pub message: String,
}

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub enum ValidationStatus {
    Pass,
    Fail,
}

impl fmt::Display for ValidationStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationStatus::Pass => write!(f, "PASS"),
            ValidationStatus::Fail => write!(f, "FAIL"),
        }
    }
}

pub async fn run(config: &NetworkConfig) -> Validation {
    let contract_id_format = validate_contract_id_formats(config);
    let (rpc_connectivity, network_passphrase) =
        validate_rpc_connectivity_and_network_passphrase(config).await;
    let signing_key = validate_signing_key(config).await;

    Validation {
        contract_id_format,
        rpc_connectivity,
        network_passphrase,
        signing_key,
    }
}

fn validate_contract_id_formats(config: &NetworkConfig) -> Vec<ValidationResult> {
    let re = Regex::new(r"^C[A-Z0-9]{55}$").unwrap();
    let contract_ids = config.all_contract_ids();
    let mut results = Vec::new();

    for (name, id) in contract_ids {
        let (status, message) = if let Some(id) = id {
            if re.is_match(id) {
                (ValidationStatus::Pass, String::new())
            } else {
                (ValidationStatus::Fail, format!("invalid format for {name}"))
            }
        } else {
            (ValidationStatus::Fail, format!("{name} not configured"))
        };

        results.push(ValidationResult {
            name: name.to_string(),
            status,
            message,
        });
    }

    results
}

async fn validate_rpc_connectivity_and_network_passphrase(
    config: &NetworkConfig,
) -> (ValidationResult, ValidationResult) {
    let client = Client::new();
    let rpc_url = &config.rpc_url;

    let rpc_connectivity = match client.get(rpc_url).send().await {
        Ok(_) => ValidationResult {
            name: "RPC Connectivity".to_string(),
            status: ValidationStatus::Pass,
            message: String::new(),
        },
        Err(e) => ValidationResult {
            name: "RPC Connectivity".to_string(),
            status: ValidationStatus::Fail,
            message: e.to_string(),
        },
    };

    let network_passphrase = if rpc_connectivity.status == ValidationStatus::Pass {
        // This is a placeholder. In a real scenario, we would make a request
        // to the RPC endpoint to get the network passphrase.
        if config.network_passphrase == "Test SDF Network ; September 2015" {
            ValidationResult {
                name: "Network Passphrase".to_string(),
                status: ValidationStatus::Pass,
                message: String::new(),
            }
        } else {
            ValidationResult {
                name: "Network Passphrase".to_string(),
                status: ValidationStatus::Fail,
                message: "incorrect network passphrase".to_string(),
            }
        }
    } else {
        ValidationResult {
            name: "Network Passphrase".to_string(),
            status: ValidationStatus::Fail,
            message: "cannot check network passphrase due to RPC connectivity failure".to_string(),
        }
    };

    (rpc_connectivity, network_passphrase)
}

async fn validate_signing_key(_config: &NetworkConfig) -> ValidationResult {
    if let Ok(secret) = env::var("XLM_NS_SIGNER_USER_SECRET") {
        if let Ok(signing_key) = load_signing_key(&secret) {
            // In a real scenario, we would make a request to the RPC endpoint
            // to get the account balance.
            if signing_key.public_address.starts_with('G') {
                ValidationResult {
                    name: "Signing Key".to_string(),
                    status: ValidationStatus::Pass,
                    message: String::new(),
                }
            } else {
                ValidationResult {
                    name: "Signing Key".to_string(),
                    status: ValidationStatus::Fail,
                    message: "invalid public key".to_string(),
                }
            }
        } else {
            ValidationResult {
                name: "Signing Key".to_string(),
                status: ValidationStatus::Fail,
                message: "invalid secret key".to_string(),
            }
        }
    } else {
        ValidationResult {
            name: "Signing Key".to_string(),
            status: ValidationStatus::Fail,
            message: "XLM_NS_SIGNER_USER_SECRET not set".to_string(),
        }
    }
}
