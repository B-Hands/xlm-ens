pub mod constants;
pub mod errors;
pub mod soroban;
pub mod types;
pub mod validation;

pub use constants::{
    DEFAULT_TTL_SECONDS, GRACE_PERIOD_SECONDS, MAX_CHAIN_NAME_LENGTH, MAX_METADATA_URI_LENGTH,
    MAX_NAME_LENGTH, MAX_REGISTRATION_YEARS, MAX_TEXT_RECORD_VALUE_LENGTH, MAX_TEXT_RECORDS,
    MIN_NAME_LENGTH, MIN_REGISTRATION_YEARS, YEAR_SECONDS,
};
pub use errors::CommonError;
#[cfg(feature = "soroban")]
pub use types::RegistryEntry;
pub use types::{NameHash, NameRecord, Tld};
pub use validation::{
    parse_fqdn, validate_chain_name, validate_label, validate_owner, validate_registration_years,
};
