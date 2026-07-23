#![cfg_attr(not(test), no_std)]
#![allow(deprecated)]
mod test;

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, Address,
    Bytes, BytesN, Env, Error, IntoVal, String, Symbol, Vec,
};
use xlm_ns_common::soroban::{validate_base_name_soroban, validate_fqdn_soroban};
use xlm_ns_common::RegistryEntry;

/// Mirrors `xlm_ns_registry::NameState` so we can avoid linking the registry
/// contract (which would cause duplicate WASM export symbols).
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
enum NameState {
    Missing,
    Active,
    GracePeriod,
    Claimable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ParentDomain {
    pub owner: Address,
    pub controllers: Vec<Address>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SubdomainRecord {
    pub parent: String,
    pub owner: Address,
    pub created_at: u64,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Parent(String),
    Subdomain(String),
    ParentSubdomains(String),
    OwnerSubdomains(Address),
    RegistryContract,
    Admin,
    ContractVersion,
    MaxDepth,
    MaxSubdomainsPerParent,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SubdomainError {
    Validation = 1,
    ParentNotFound = 2,
    AlreadyExists = 3,
    NotFound = 4,
    Unauthorized = 5,
    UpgradeFailed = 6,
    DepthLimitExceeded = 7,
    ParentSubdomainLimitReached = 8,
}

pub const CONTRACT_VERSION: u32 = 1;
pub const DEFAULT_MAX_SUBDOMAINS_PER_PARENT: u32 = 100;

#[contractevent]
pub struct ContractUpgraded {
    pub old_version: u32,
    pub new_version: u32,
    pub admin: Address,
}

#[contract]
pub struct SubdomainContract;

#[contractimpl]
impl SubdomainContract {
    pub fn version(_env: Env) -> u32 {
        CONTRACT_VERSION
    }

    pub fn initialize(env: Env, admin: Address) -> Result<(), SubdomainError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(SubdomainError::AlreadyExists);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::ContractVersion, &CONTRACT_VERSION);
        env.storage().persistent().set(&DataKey::MaxDepth, &3u32);
        env.storage().persistent().set(
            &DataKey::MaxSubdomainsPerParent,
            &DEFAULT_MAX_SUBDOMAINS_PER_PARENT,
        );
        Ok(())
    }

    pub fn set_registry_contract(
        env: Env,
        registry_contract: Address,
    ) -> Result<(), SubdomainError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SubdomainError::Unauthorized)?;
        admin.require_auth();
        env.storage()
            .instance()
            .set(&DataKey::RegistryContract, &registry_contract);
        Ok(())
    }

    pub fn get_version(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::ContractVersion)
            .unwrap_or(CONTRACT_VERSION)
    }

    pub fn set_max_depth(env: Env, depth: u32) -> Result<(), SubdomainError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SubdomainError::Unauthorized)?;
        admin.require_auth();
        env.storage().persistent().set(&DataKey::MaxDepth, &depth);
        Ok(())
    }

    pub fn max_depth(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::MaxDepth)
            .unwrap_or(3)
    }

    pub fn set_max_subdomains_per_parent(env: Env, limit: u32) -> Result<(), SubdomainError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SubdomainError::Unauthorized)?;
        admin.require_auth();
        env.storage()
            .persistent()
            .set(&DataKey::MaxSubdomainsPerParent, &limit);
        Ok(())
    }

    pub fn get_max_subdomains_per_parent(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::MaxSubdomainsPerParent)
            .unwrap_or(DEFAULT_MAX_SUBDOMAINS_PER_PARENT)
    }

    pub fn upgrade(
        env: Env,
        new_wasm_hash: BytesN<32>,
        _migration_data: Bytes,
    ) -> Result<(), SubdomainError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(SubdomainError::UpgradeFailed)?;
        admin.require_auth();

        env.deployer().update_current_contract_wasm(new_wasm_hash);

        Ok(())
    }

    /// Registers a parent domain to enable subdomain creation.
    ///
    /// Safe Bootstrap Path: The parent owner must register the parent domain
    /// exactly once. Subsequent attempts to register the same parent domain
    /// will be rejected to prevent unauthorized takeover of the parent namespace.
    pub fn register_parent(env: Env, parent: String, owner: Address) -> Result<(), SubdomainError> {
        owner.require_auth();
        validate_fqdn_soroban(&parent).map_err(|_| SubdomainError::Validation)?;
        validate_base_name_soroban(&parent).map_err(|_| SubdomainError::Validation)?;
        let key = DataKey::Parent(parent.clone());
        if let Some(registry) = get_registry_address(&env) {
            let now_unix = env.ledger().timestamp();
            match env.try_invoke_contract::<RegistryEntry, Error>(
                &registry,
                &Symbol::new(&env, "resolve"),
                (parent.clone(), now_unix).into_val(&env),
            ) {
                Ok(Ok(entry)) => {
                    if entry.owner != owner {
                        return Err(SubdomainError::Unauthorized);
                    }
                    if let Some(existing) = env.storage().persistent().get::<_, ParentDomain>(&key)
                    {
                        if existing.owner != entry.owner {
                            purge_parent_namespace(&env, &parent);
                        } else {
                            return Err(SubdomainError::AlreadyExists);
                        }
                    }
                }
                _ => {
                    if env.storage().persistent().has(&key) {
                        purge_parent_namespace(&env, &parent);
                    }
                    return Err(SubdomainError::ParentNotFound);
                }
            }
        } else if env.storage().persistent().has(&key) {
            return Err(SubdomainError::AlreadyExists);
        }
        let record = ParentDomain {
            owner,
            controllers: Vec::new(&env),
        };
        env.storage().persistent().set(&key, &record);
        env.events().publish(
            (symbol_short!("subdomain"), symbol_short!("prnt_reg")),
            (parent, record.owner.clone()),
        );
        Ok(())
    }

    pub fn add_controller(
        env: Env,
        parent: String,
        caller: Address,
        controller: Address,
    ) -> Result<(), SubdomainError> {
        caller.require_auth();
        let mut parent_record = get_parent(&env, &parent)?;
        if parent_record.owner != caller {
            return Err(SubdomainError::Unauthorized);
        }
        if !parent_record.controllers.contains(&controller) {
            parent_record.controllers.push_back(controller.clone());
            env.storage()
                .persistent()
                .set(&DataKey::Parent(parent.clone()), &parent_record);
            env.events().publish(
                (symbol_short!("subdomain"), symbol_short!("ctrl_add")),
                (parent, caller, controller),
            );
        }
        Ok(())
    }

    pub fn remove_controller(
        env: Env,
        parent: String,
        caller: Address,
        controller: Address,
    ) -> Result<(), SubdomainError> {
        caller.require_auth();
        let mut parent_record = get_parent(&env, &parent)?;
        if parent_record.owner != caller {
            return Err(SubdomainError::Unauthorized);
        }

        if let Some(index) = parent_record.controllers.first_index_of(&controller) {
            parent_record.controllers.remove(index);
            env.storage()
                .persistent()
                .set(&DataKey::Parent(parent.clone()), &parent_record);
            env.events().publish(
                (symbol_short!("subdomain"), symbol_short!("ctrl_rm")),
                (parent, caller, controller),
            );
        }

        Ok(())
    }

    pub fn create(
        env: Env,
        label: String,
        parent: String,
        caller: Address,
        owner: Address,
        now_unix: u64,
    ) -> Result<String, SubdomainError> {
        caller.require_auth();
        let parent_record = get_parent(&env, &parent)?;
        if parent_record.owner != caller && !parent_record.controllers.contains(&caller) {
            return Err(SubdomainError::Unauthorized);
        }

        let max_depth: u32 = env
            .storage()
            .persistent()
            .get(&DataKey::MaxDepth)
            .unwrap_or(3);
        let label_byte_count = label.len() as usize;
        let parent_byte_count = parent.len() as usize;
        let fqdn_byte_count = label_byte_count + 1 + parent_byte_count;
        let mut fqdn_bytes = [0u8; 256];

        let mut label_buf = [0u8; 256];
        label.copy_into_slice(&mut label_buf[..label_byte_count]);
        let mut parent_buf = [0u8; 256];
        parent.copy_into_slice(&mut parent_buf[..parent_byte_count]);

        if label_byte_count > 0 {
            let mut depth = 1u32;
            for &byte in &label_buf[..label_byte_count] {
                if byte == b'.' {
                    depth += 1;
                }
            }

            if depth > max_depth {
                return Err(SubdomainError::DepthLimitExceeded);
            }
        }

        fqdn_bytes[..label_byte_count].copy_from_slice(&label_buf[..label_byte_count]);
        fqdn_bytes[label_byte_count] = b'.';
        fqdn_bytes[label_byte_count + 1..fqdn_byte_count]
            .copy_from_slice(&parent_buf[..parent_byte_count]);

        let fqdn = String::from_bytes(&env, &fqdn_bytes[..fqdn_byte_count]);
        let key = DataKey::Subdomain(fqdn.clone());
        if env.storage().persistent().has(&key) {
            return Err(SubdomainError::AlreadyExists);
        }

        if get_parent_subdomains(&env, &parent).len()
            >= Self::get_max_subdomains_per_parent(env.clone())
        {
            return Err(SubdomainError::ParentSubdomainLimitReached);
        }

        let record = SubdomainRecord {
            parent: parent.clone(),
            owner: owner.clone(),
            created_at: now_unix,
        };
        env.storage().persistent().set(&key, &record);

        add_parent_subdomain(&env, &parent, &fqdn);
        add_owner_subdomain(&env, &owner, &fqdn);

        env.events().publish(
            (symbol_short!("subdomain"), symbol_short!("created")),
            (fqdn.clone(), parent, caller, owner),
        );

        Ok(fqdn)
    }

    pub fn transfer(
        env: Env,
        fqdn: String,
        caller: Address,
        new_owner: Address,
    ) -> Result<(), SubdomainError> {
        caller.require_auth();
        let mut record = get_subdomain(&env, &fqdn)?;
        if !ensure_parent_is_active(&env, &record.parent) {
            return Err(SubdomainError::ParentNotFound);
        }
        if record.owner != caller {
            return Err(SubdomainError::Unauthorized);
        }
        let old_owner = record.owner.clone();
        record.owner = new_owner.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Subdomain(fqdn.clone()), &record);

        remove_owner_subdomain(&env, &old_owner, &fqdn);
        add_owner_subdomain(&env, &new_owner, &fqdn);

        env.events().publish(
            (symbol_short!("subdomain"), symbol_short!("transfer")),
            (fqdn, old_owner, new_owner),
        );

        Ok(())
    }

    pub fn delete(env: Env, fqdn: String, caller: Address) -> Result<(), SubdomainError> {
        caller.require_auth();
        let record = get_subdomain(&env, &fqdn)?;
        if !ensure_parent_is_active(&env, &record.parent) {
            return Err(SubdomainError::ParentNotFound);
        }
        if record.owner != caller {
            let parent_record = get_parent(&env, &record.parent)?;
            if parent_record.owner != caller && !parent_record.controllers.contains(&caller) {
                return Err(SubdomainError::Unauthorized);
            }
        }

        remove_parent_subdomain(&env, &record.parent, &fqdn);
        remove_owner_subdomain(&env, &record.owner, &fqdn);

        env.events().publish(
            (symbol_short!("subdomain"), symbol_short!("deleted")),
            (fqdn.clone(), caller),
        );

        env.storage().persistent().remove(&DataKey::Subdomain(fqdn));

        Ok(())
    }

    /// Revokes a subdomain, removing it from storage.
    ///
    /// Deletion Semantics:
    /// - The current owner of the subdomain can delete it.
    /// - The owner or a delegated controller of the parent domain can revoke it
    ///   (e.g., to reclaim the namespace or enforce namespace rules).
    pub fn revoke(env: Env, fqdn: String, caller: Address) -> Result<(), SubdomainError> {
        caller.require_auth();
        let record = get_subdomain(&env, &fqdn)?;
        if !ensure_parent_is_active(&env, &record.parent) {
            return Err(SubdomainError::ParentNotFound);
        }

        let mut is_authorized = false;
        if record.owner == caller {
            is_authorized = true;
        } else if let Ok(parent_record) = get_parent(&env, &record.parent) {
            if parent_record.owner == caller || parent_record.controllers.contains(&caller) {
                is_authorized = true;
            }
        }

        if !is_authorized {
            return Err(SubdomainError::Unauthorized);
        }

        remove_parent_subdomain(&env, &record.parent, &fqdn);
        remove_owner_subdomain(&env, &record.owner, &fqdn);

        env.events().publish(
            (symbol_short!("subdomain"), symbol_short!("revoked")),
            (fqdn.clone(), caller),
        );

        env.storage().persistent().remove(&DataKey::Subdomain(fqdn));
        Ok(())
    }

    pub fn exists(env: Env, fqdn: String) -> bool {
        Self::record(env, fqdn).is_some()
    }

    pub fn parent(env: Env, parent: String) -> Option<ParentDomain> {
        if !ensure_parent_is_active(&env, &parent) {
            return None;
        }
        env.storage().persistent().get(&DataKey::Parent(parent))
    }

    pub fn record(env: Env, fqdn: String) -> Option<SubdomainRecord> {
        match env
            .storage()
            .persistent()
            .get::<_, SubdomainRecord>(&DataKey::Subdomain(fqdn.clone()))
        {
            None => None,
            Some(record) => {
                if ensure_parent_is_active(&env, &record.parent) {
                    Some(record)
                } else {
                    purge_subdomain_record(&env, &fqdn, &record);
                    None
                }
            }
        }
    }

    pub fn subdomains_for_parent(env: Env, parent: String) -> Vec<String> {
        if !ensure_parent_is_active(&env, &parent) {
            return Vec::new(&env);
        }
        get_parent_subdomains(&env, &parent)
    }

    pub fn subdomains_for_owner(env: Env, owner: Address) -> Vec<String> {
        get_owner_subdomains(&env, &owner)
    }
}

fn get_parent(env: &Env, parent: &String) -> Result<ParentDomain, SubdomainError> {
    if !ensure_parent_is_active(env, parent) {
        return Err(SubdomainError::ParentNotFound);
    }
    env.storage()
        .persistent()
        .get(&DataKey::Parent(parent.clone()))
        .ok_or(SubdomainError::ParentNotFound)
}

fn get_subdomain(env: &Env, fqdn: &String) -> Result<SubdomainRecord, SubdomainError> {
    env.storage()
        .persistent()
        .get(&DataKey::Subdomain(fqdn.clone()))
        .ok_or(SubdomainError::NotFound)
}

fn get_registry_address(env: &Env) -> Option<Address> {
    env.storage()
        .instance()
        .get::<_, Address>(&DataKey::RegistryContract)
}

fn ensure_parent_is_active(env: &Env, parent: &String) -> bool {
    match get_registry_address(env) {
        None => true,
        Some(registry) => {
            let now_unix = env.ledger().timestamp();
            match env.try_invoke_contract::<NameState, Error>(
                &registry,
                &Symbol::new(env, "name_state"),
                (parent.clone(), now_unix).into_val(env),
            ) {
                Ok(Ok(NameState::Active)) => true,
                _ => {
                    purge_parent_namespace(env, parent);
                    false
                }
            }
        }
    }
}

fn purge_parent_namespace(env: &Env, parent: &String) {
    let subdomains = get_parent_subdomains(env, parent);
    for fqdn in subdomains.iter() {
        if let Some(record) = env
            .storage()
            .persistent()
            .get::<_, SubdomainRecord>(&DataKey::Subdomain(fqdn.clone()))
        {
            purge_subdomain_record(env, &fqdn, &record);
        }
    }
    env.storage()
        .persistent()
        .remove(&DataKey::ParentSubdomains(parent.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::Parent(parent.clone()));
}

fn purge_subdomain_record(env: &Env, fqdn: &String, record: &SubdomainRecord) {
    remove_parent_subdomain(env, &record.parent, fqdn);
    remove_owner_subdomain(env, &record.owner, fqdn);
    env.storage()
        .persistent()
        .remove(&DataKey::Subdomain(fqdn.clone()));
}

fn get_parent_subdomains(env: &Env, parent: &String) -> Vec<String> {
    env.storage()
        .persistent()
        .get(&DataKey::ParentSubdomains(parent.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn add_parent_subdomain(env: &Env, parent: &String, fqdn: &String) {
    let mut subdomains = get_parent_subdomains(env, parent);
    if !subdomains.contains(fqdn) {
        subdomains.push_back(fqdn.clone());
        env.storage()
            .persistent()
            .set(&DataKey::ParentSubdomains(parent.clone()), &subdomains);
    }
}

fn remove_parent_subdomain(env: &Env, parent: &String, fqdn: &String) {
    let mut subdomains = get_parent_subdomains(env, parent);
    if let Some(index) = subdomains.first_index_of(fqdn) {
        subdomains.remove(index);
        env.storage()
            .persistent()
            .set(&DataKey::ParentSubdomains(parent.clone()), &subdomains);
    }
}

fn get_owner_subdomains(env: &Env, owner: &Address) -> Vec<String> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerSubdomains(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn add_owner_subdomain(env: &Env, owner: &Address, fqdn: &String) {
    let mut subdomains = get_owner_subdomains(env, owner);
    if !subdomains.contains(fqdn) {
        subdomains.push_back(fqdn.clone());
        env.storage()
            .persistent()
            .set(&DataKey::OwnerSubdomains(owner.clone()), &subdomains);
    }
}

fn remove_owner_subdomain(env: &Env, owner: &Address, fqdn: &String) {
    let mut subdomains = get_owner_subdomains(env, owner);
    if let Some(index) = subdomains.first_index_of(fqdn) {
        subdomains.remove(index);
        env.storage()
            .persistent()
            .set(&DataKey::OwnerSubdomains(owner.clone()), &subdomains);
    }
}

#[allow(dead_code)]
fn migrate(from_version: u32, to_version: u32, _data: &Bytes) {
    let _ = (from_version, to_version);
}

#[allow(dead_code)]
fn decode_target_version(data: &Bytes) -> u32 {
    if data.len() < 4 {
        return CONTRACT_VERSION + 1;
    }
    let b0 = data.get(0).unwrap_or(0);
    let b1 = data.get(1).unwrap_or(0);
    let b2 = data.get(2).unwrap_or(0);
    let b3 = data.get(3).unwrap_or(0);
    u32::from_be_bytes([b0, b1, b2, b3])
}
