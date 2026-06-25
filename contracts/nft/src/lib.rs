#![cfg_attr(not(test), no_std)]
mod events;
mod test;

use soroban_sdk::{
<<<<<<< HEAD
    contract, contracterror, contractevent, contractimpl, contracttype, symbol_short, Address,
    Bytes, Env, String, Vec,
=======
    contract, contracterror, contractevent, contractimpl, contracttype, Address, Bytes, BytesN,
    Env, IntoVal, String, Symbol, Vec,
>>>>>>> upstream/main
};
use xlm_ns_common::RegistryEntry;

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TokenRecord {
    pub owner: Address,
    pub approved: Option<Address>,
    pub metadata_uri: Option<String>,
    pub expires_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct NameRecord {
    pub registration_date: u64,
    pub expiry_date: u64,
    pub target_address: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct TokenMetadata {
    pub token_id: String,
    pub owner: Address,
    pub registration_date: u64,
    pub expiry_date: u64,
    pub is_expired: bool,
    pub target_address: Option<String>,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Token(String),
    TokenIds,
    OwnerTokens(Address),
    Admin,
    ContractVersion,
    NameData(String),
    Registry,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NftError {
    AlreadyMinted = 1,
    NotFound = 2,
    Unauthorized = 3,
    UpgradeFailed = 4,
    NotInitialized = 5,
}

pub const CONTRACT_VERSION: u32 = 1;

#[contractevent]
#[contracttype]
pub struct ContractUpgraded {
    pub old_version: u32,
    pub new_version: u32,
    pub admin: Address,
}

#[contract]
pub struct NftContract;

#[contractimpl]
impl NftContract {
    pub fn version(_env: Env) -> u32 {
        CONTRACT_VERSION
    }

    pub fn initialize(env: Env, admin: Address) -> Result<(), NftError> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(NftError::AlreadyMinted);
        }
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage()
            .persistent()
            .set(&DataKey::ContractVersion, &CONTRACT_VERSION);
        Ok(())
    }

    pub fn get_version(env: Env) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::ContractVersion)
            .unwrap_or(CONTRACT_VERSION)
    }

    pub fn upgrade(env: Env, new_wasm_hash: Bytes, migration_data: Bytes) -> Result<(), NftError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NftError::UpgradeFailed)?;
        admin.require_auth();

        let current_version = Self::get_version(env.clone());
        let target_version = decode_target_version(&migration_data);

        for v in current_version..target_version {
            migrate(v, v + 1, &migration_data);
        }

        env.storage()
            .persistent()
            .set(&DataKey::ContractVersion, &target_version);

        env.events().publish(
            (symbol_short!("nft"), symbol_short!("upgraded")),
            ContractUpgraded {
                old_version: current_version,
                new_version: target_version,
                admin,
            },
        );

<<<<<<< HEAD
        env.deployer().update_current_contract_wasm(new_wasm_hash.to_bytes());
=======
        env.deployer()
            .update_current_contract_wasm(new_wasm_hash.to_bytes());
        env.deployer().update_current_contract_wasm(new_wasm_hash);
>>>>>>> upstream/main

        Ok(())
    }

    pub fn set_registry(env: Env, admin: Address, registry: Address) -> Result<(), NftError> {
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NftError::NotInitialized)?;
        if stored_admin != admin {
            return Err(NftError::Unauthorized);
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Registry, &registry);
        Ok(())
    }

    pub fn mint(
        env: Env,
        token_id: String,
        owner: Address,
        metadata_uri: Option<String>,
        expires_at: u64,
    ) -> Result<(), NftError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NftError::Unauthorized)?;
        admin.require_auth();

        let key = DataKey::Token(token_id.clone());
        if env.storage().persistent().has(&key) {
            return Err(NftError::AlreadyMinted);
        }
        let record = TokenRecord {
            owner: owner.clone(),
            approved: None,
            metadata_uri,
            expires_at,
        };
        env.storage().persistent().set(&key, &record);
        append_token_id(&env, &token_id);
        add_owner_token(&env, &owner, &token_id);

        if let Ok(registry) = get_registry(&env) {
            let now_unix = env.ledger().timestamp();
            let entry = env.invoke_contract::<RegistryEntry>(
                &registry,
                &Symbol::new(&env, "resolve"),
                (token_id.clone(), now_unix).into_val(&env),
            );
            let name_record = NameRecord {
                registration_date: entry.registered_at,
                expiry_date: entry.expires_at,
                target_address: entry.target_address,
            };
            env.storage()
                .persistent()
                .set(&DataKey::NameData(token_id.clone()), &name_record);
        }

        events::mint(&env, owner.clone(), owner, token_id);
        Ok(())
    }

    pub fn metadata(env: Env, token_id: String, now_unix: u64) -> Option<TokenMetadata> {
        let token: TokenRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Token(token_id.clone()))?;
        let name_record: NameRecord = env
            .storage()
            .persistent()
            .get(&DataKey::NameData(token_id.clone()))?;
        let is_expired = now_unix > name_record.expiry_date;
        Some(TokenMetadata {
            token_id,
            owner: token.owner,
            registration_date: name_record.registration_date,
            expiry_date: name_record.expiry_date,
            is_expired,
            target_address: name_record.target_address,
        })
    }

    pub fn refresh_name_data(env: Env, token_id: String) -> Result<(), NftError> {
        let registry = get_registry(&env)?;
        if !env
            .storage()
            .persistent()
            .has(&DataKey::Token(token_id.clone()))
        {
            return Err(NftError::NotFound);
        }
        let now_unix = env.ledger().timestamp();
        let entry = env.invoke_contract::<RegistryEntry>(
            &registry,
            &Symbol::new(&env, "resolve"),
            (token_id.clone(), now_unix).into_val(&env),
        );
        let name_record = NameRecord {
            registration_date: entry.registered_at,
            expiry_date: entry.expires_at,
            target_address: entry.target_address,
        };
        env.storage()
            .persistent()
            .set(&DataKey::NameData(token_id), &name_record);
        Ok(())
    }

    pub fn approve(
        env: Env,
        token_id: String,
        caller: Address,
        approved: Address,
    ) -> Result<(), NftError> {
        let mut record = get_token(&env, &token_id)?;
        if record.owner != caller {
            return Err(NftError::Unauthorized);
        }
        record.approved = Some(approved.clone());
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id.clone()), &record);
        events::approve(&env, caller, approved, token_id);
        Ok(())
    }

    pub fn approve_clear(env: Env, token_id: String, caller: Address) -> Result<(), NftError> {
        let mut record = get_token(&env, &token_id)?;
        if record.owner != caller {
            return Err(NftError::Unauthorized);
        }
        record.approved = None;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id.clone()), &record);
        events::approve_clear(&env, caller, token_id);
        Ok(())
    }

    pub fn transfer(
        env: Env,
        token_id: String,
        caller: Address,
        new_owner: Address,
    ) -> Result<(), NftError> {
        let mut record = get_token(&env, &token_id)?;
        if record.owner != caller && record.approved.as_ref() != Some(&caller) {
            return Err(NftError::Unauthorized);
        }
        let previous_owner = record.owner.clone();
        record.owner = new_owner.clone();
        record.approved = None;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id.clone()), &record);
        reindex_owner_token(&env, &previous_owner, &record.owner, &token_id);
        events::transfer(&env, previous_owner, new_owner, token_id);
        Ok(())
    }

    pub fn transfer_from(
        env: Env,
        spender: Address,
        owner: Address,
        recipient: Address,
        token_id: String,
    ) -> Result<(), NftError> {
        let mut record = get_token(&env, &token_id)?;
        if record.owner != owner || record.approved.as_ref() != Some(&spender) {
            return Err(NftError::Unauthorized);
        }
        record.owner = recipient.clone();
        record.approved = None;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id.clone()), &record);
        reindex_owner_token(&env, &owner, &record.owner, &token_id);
        events::transfer(&env, owner, recipient, token_id);
        Ok(())
    }

    pub fn owner_of(env: Env, token_id: String) -> Option<Address> {
        env.storage()
            .persistent()
            .get::<_, TokenRecord>(&DataKey::Token(token_id))
            .map(|record| record.owner)
    }

    pub fn token(env: Env, token_id: String) -> Option<TokenRecord> {
        env.storage().persistent().get(&DataKey::Token(token_id))
    }

    pub fn balance_of(env: Env, owner: Address) -> u32 {
        owner_tokens(&env, &owner).len()
    }

    pub fn total_supply(env: Env) -> u32 {
        token_ids(&env).len()
    }

    pub fn token_by_index(env: Env, index: u32) -> Option<String> {
        token_ids(&env).get(index)
    }

    pub fn token_of_owner_by_index(env: Env, owner: Address, index: u32) -> Option<String> {
        owner_tokens(&env, &owner).get(index)
    }

    pub fn token_uri(env: Env, token_id: String) -> Option<String> {
        env.storage()
            .persistent()
            .get::<_, TokenRecord>(&DataKey::Token(token_id))
            .and_then(|record| record.metadata_uri)
    }

    pub fn burn(env: Env, token_id: String) -> Result<(), NftError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NftError::Unauthorized)?;
        admin.require_auth();

        let record = get_token(&env, &token_id)?;
        env.storage()
            .persistent()
            .remove(&DataKey::Token(token_id.clone()));
        remove_owner_token(&env, &record.owner, &token_id);

        let mut all_tokens = token_ids(&env);
        if let Some(index) = all_tokens.iter().position(|t| t == token_id) {
            all_tokens.remove(index as u32);
        }
        env.storage()
            .persistent()
            .set(&DataKey::TokenIds, &all_tokens);

        events::burn(&env, admin, token_id);
        Ok(())
    }

    pub fn sync_expiry(
        env: Env,
        token_id: String,
        new_expiry: u64,
    ) -> Result<(), NftError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NftError::Unauthorized)?;
        admin.require_auth();

        let mut record = get_token(&env, &token_id)?;
        record.expires_at = new_expiry;
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id.clone()), &record);
        Ok(())
    }

    pub fn sync_owner(
        env: Env,
        token_id: String,
        new_owner: Address,
    ) -> Result<(), NftError> {
        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(NftError::Unauthorized)?;
        admin.require_auth();

        let mut record = get_token(&env, &token_id)?;
        let old_owner = record.owner.clone();
        record.owner = new_owner.clone();
        env.storage()
            .persistent()
            .set(&DataKey::Token(token_id.clone()), &record);
        reindex_owner_token(&env, &old_owner, &new_owner, &token_id);
        events::transfer(&env, old_owner, new_owner, token_id);
        Ok(())
    }
}

fn get_registry(env: &Env) -> Result<Address, NftError> {
    env.storage()
        .instance()
        .get(&DataKey::Registry)
        .ok_or(NftError::NotInitialized)
}

fn get_token(env: &Env, token_id: &String) -> Result<TokenRecord, NftError> {
    env.storage()
        .persistent()
        .get(&DataKey::Token(token_id.clone()))
        .ok_or(NftError::NotFound)
}

fn token_ids(env: &Env) -> Vec<String> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenIds)
        .unwrap_or(Vec::new(env))
}

fn owner_tokens(env: &Env, owner: &Address) -> Vec<String> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerTokens(owner.clone()))
        .unwrap_or(Vec::new(env))
}

fn append_token_id(env: &Env, token_id: &String) {
    let mut token_ids = token_ids(env);
    token_ids.push_back(token_id.clone());
    env.storage()
        .persistent()
        .set(&DataKey::TokenIds, &token_ids);
}

fn add_owner_token(env: &Env, owner: &Address, token_id: &String) {
    let key = DataKey::OwnerTokens(owner.clone());
    let mut tokens = owner_tokens(env, owner);
    if !tokens.contains(token_id) {
        tokens.push_back(token_id.clone());
        env.storage().persistent().set(&key, &tokens);
    }
}

fn remove_owner_token(env: &Env, owner: &Address, token_id: &String) {
    let key = DataKey::OwnerTokens(owner.clone());
    let tokens = owner_tokens(env, owner);
    let mut filtered = Vec::new(env);
    for existing in tokens.iter() {
        if existing != *token_id {
            filtered.push_back(existing);
        }
    }
    env.storage().persistent().set(&key, &filtered);
}

fn reindex_owner_token(
    env: &Env,
    previous_owner: &Address,
    new_owner: &Address,
    token_id: &String,
) {
    if previous_owner == new_owner {
        return;
    }

    remove_owner_token(env, previous_owner, token_id);
    add_owner_token(env, new_owner, token_id);
}

fn migrate(from_version: u32, to_version: u32, _data: &Bytes) {
    let _ = (from_version, to_version);
}

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
