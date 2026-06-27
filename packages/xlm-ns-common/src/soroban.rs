use soroban_sdk::{Env, String};

use crate::constants::{
    MAX_CHAIN_NAME_LENGTH, MAX_NAME_LENGTH, MAX_REGISTRATION_YEARS, MIN_NAME_LENGTH,
    MIN_REGISTRATION_YEARS,
};
use crate::errors::CommonError;

const XLM_SUFFIX_LEN: usize = 4;
const MAX_FQDN_LENGTH: usize = MAX_NAME_LENGTH + XLM_SUFFIX_LEN;

pub fn validate_label_soroban(label: &String) -> Result<(), CommonError> {
    let (bytes, len) = copy_bytes::<MAX_NAME_LENGTH>(label)?;
    validate_label_bytes(&bytes[..len])
}

pub fn validate_registration_years_soroban(years: u64) -> Result<(), CommonError> {
    if !(MIN_REGISTRATION_YEARS..=MAX_REGISTRATION_YEARS).contains(&years) {
        return Err(CommonError::InvalidRegistrationPeriod);
    }

    Ok(())
}

pub fn validate_chain_name_soroban(chain: &String) -> Result<(), CommonError> {
    let len = chain.len() as usize;
    if len == 0 {
        return Err(CommonError::EmptyChainName);
    }
    if len > MAX_CHAIN_NAME_LENGTH {
        return Err(CommonError::NameTooLong);
    }

    Ok(())
}

pub fn validate_base_name_soroban(name: &String) -> Result<(), CommonError> {
    let (bytes, len) = copy_bytes::<MAX_FQDN_LENGTH>(name)?;
    let dot_index = bytes[..len]
        .iter()
        .position(|byte| *byte == b'.')
        .ok_or(CommonError::MissingTld)?;

    if bytes[dot_index + 1..len].contains(&b'.') {
        return Err(CommonError::InvalidName);
    }

    validate_label_bytes(&bytes[..dot_index])?;
    if &bytes[dot_index + 1..len] != b"xlm" {
        return Err(CommonError::UnsupportedTld);
    }

    Ok(())
}

pub fn validate_fqdn_soroban(name: &String) -> Result<(), CommonError> {
    let (bytes, len) = copy_bytes::<MAX_FQDN_LENGTH>(name)?;
    let dot_index = bytes[..len]
        .iter()
        .rposition(|byte| *byte == b'.')
        .ok_or(CommonError::MissingTld)?;

    if &bytes[dot_index + 1..len] != b"xlm" {
        return Err(CommonError::UnsupportedTld);
    }

    for label in bytes[..dot_index].split(|b| *b == b'.') {
        validate_label_bytes(label)?;
    }

    Ok(())
}

pub fn extract_label_soroban(env: &Env, name: &String) -> Result<String, CommonError> {
    let (bytes, len) = copy_bytes::<MAX_FQDN_LENGTH>(name)?;
    let dot_index = bytes[..len]
        .iter()
        .position(|byte| *byte == b'.')
        .ok_or(CommonError::MissingTld)?;
    validate_fqdn_soroban(name)?;
    Ok(String::from_bytes(env, &bytes[..dot_index]))
}

pub fn build_xlm_name(env: &Env, label: &String) -> Result<String, CommonError> {
    let (bytes, len) = copy_bytes::<MAX_NAME_LENGTH>(label)?;
    validate_label_bytes(&bytes[..len])?;

    let mut fqdn = [0u8; MAX_FQDN_LENGTH];
    fqdn[..len].copy_from_slice(&bytes[..len]);
    fqdn[len] = b'.';
    fqdn[len + 1..len + XLM_SUFFIX_LEN].copy_from_slice(b"xlm");
    Ok(String::from_bytes(env, &fqdn[..len + XLM_SUFFIX_LEN]))
}

pub fn build_subdomain_name(
    env: &Env,
    label: &String,
    parent: &String,
) -> Result<String, CommonError> {
    let (label_bytes, label_len) = copy_bytes::<MAX_NAME_LENGTH>(label)?;
    validate_label_bytes(&label_bytes[..label_len])?;
    validate_fqdn_soroban(parent)?;

    let parent_len = parent.len() as usize;
    let mut parent_bytes = [0u8; MAX_FQDN_LENGTH];
    parent.copy_into_slice(&mut parent_bytes[..parent_len]);

    let total_len = label_len + 1 + parent_len;
    let mut full = [0u8; MAX_NAME_LENGTH + 1 + MAX_FQDN_LENGTH];
    full[..label_len].copy_from_slice(&label_bytes[..label_len]);
    full[label_len] = b'.';
    full[label_len + 1..total_len].copy_from_slice(&parent_bytes[..parent_len]);
    Ok(String::from_bytes(env, &full[..total_len]))
}

fn copy_bytes<const N: usize>(value: &String) -> Result<([u8; N], usize), CommonError> {
    let len = value.len() as usize;
    if len > N {
        return Err(CommonError::NameTooLong);
    }

    let mut bytes = [0u8; N];
    value.copy_into_slice(&mut bytes[..len]);
    Ok((bytes, len))
}

pub fn validate_label_bytes(bytes: &[u8]) -> Result<(), CommonError> {
    let len = bytes.len();
    if len < MIN_NAME_LENGTH {
        return Err(CommonError::NameTooShort);
    }
    if len > MAX_NAME_LENGTH {
        return Err(CommonError::NameTooLong);
    }
    if bytes.first() == Some(&b'-') || bytes.last() == Some(&b'-') {
        return Err(CommonError::InvalidLabelBoundary);
    }
    if !bytes
        .iter()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
    {
        return Err(CommonError::InvalidCharacters);
    }

    Ok(())
}
