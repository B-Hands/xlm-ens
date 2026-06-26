use std::env;
use std::fmt;

/// Resolved signer material for a write command.
///
/// A `SignerProfile` carries the public identity (profile name and public
/// address) that will be attached to outbound requests. The secret material
/// used to sign transactions is deliberately *not* represented here: the
/// secret is only ever read from environment variables at the moment the
/// underlying SDK performs the signing, so it never travels through
/// command-line arguments, logs, or process tables.
#[derive(Debug, Clone)]
pub struct SignerProfile {
    pub name: String,
    pub public_address: String,
    pub source: SignerSource,
}

#[derive(Debug, Clone)]
pub enum SignerSource {
    Environment {
        public_var: String,
        secret_var: String,
    },
}

#[derive(Debug, Clone)]
pub enum SignerError {
    MissingPublic { var: String },
    MissingSecret { var: String },
    EmptyProfileName,
}

impl fmt::Display for SignerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingPublic { var } => write!(
                f,
                "signer profile is missing public address; expected env var {var}"
            ),
            Self::MissingSecret { var } => write!(
                f,
                "signer profile is missing signing material; expected env var {var}"
            ),
            Self::EmptyProfileName => write!(f, "signer profile name must not be empty"),
        }
    }
}

impl std::error::Error for SignerError {}

/// Load a signer profile by name.
///
/// Given a profile name like `treasury`, this looks up:
///   - `XLM_NS_SIGNER_TREASURY_PUBLIC`  (public Stellar address, displayed)
///   - `XLM_NS_SIGNER_TREASURY_SECRET`  (seed phrase or secret key, never displayed)
///
/// The secret variable is checked for presence only — its value is never
/// captured into the returned struct, so it cannot be accidentally printed
/// by callers.
pub fn load_profile(name: &str) -> Result<SignerProfile, SignerError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(SignerError::EmptyProfileName);
    }

    let slug = trimmed.to_ascii_uppercase().replace('-', "_");
    let public_var = format!("XLM_NS_SIGNER_{slug}_PUBLIC");
    let secret_var = format!("XLM_NS_SIGNER_{slug}_SECRET");

    let public_address = env::var(&public_var).map_err(|_| SignerError::MissingPublic {
        var: public_var.clone(),
    })?;
    if env::var(&secret_var).is_err() {
        return Err(SignerError::MissingSecret { var: secret_var });
    }

    Ok(SignerProfile {
        name: trimmed.to_string(),
        public_address,
        source: SignerSource::Environment {
            public_var,
            secret_var,
        },
    })
}

impl SignerProfile {
    /// Short, safe description for log output — never includes secret data.
    pub fn describe(&self) -> String {
        match &self.source {
            SignerSource::Environment {
                public_var,
                secret_var,
            } => format!(
                "profile '{name}' -> {addr} (public env: {public_var}, secret env: {secret_var})",
                name = self.name,
                addr = self.public_address
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SigningKey {
    pub keypair: stellar_sdk::SecretKey,
    pub public_address: String,
}

#[derive(Debug, Clone)]
pub enum SigningKeyError {
    InvalidSecret,
}

impl fmt::Display for SigningKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSecret => write!(f, "invalid secret key"),
        }
    }
}

impl std::error::Error for SigningKeyError {}

pub fn load_signing_key(secret: &str) -> Result<SigningKey, SigningKeyError> {
    let keypair = stellar_sdk::SecretKey::from_str(secret)
        .map_err(|_| SigningKeyError::InvalidSecret)?;
    let public_address = keypair.public_key().to_string();
    Ok(SigningKey {
        keypair,
        public_address,
    })
}
