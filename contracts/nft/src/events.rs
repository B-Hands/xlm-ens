use soroban_sdk::{symbol_short, Address, Env, String};

/// Emitted when a new token is minted.
/// topics : ("mint", admin: Address, to: Address)
/// data   : token_id: String
pub fn mint(env: &Env, admin: Address, to: Address, token_id: String) {
    env.events()
        .publish((symbol_short!("mint"), admin, to), token_id);
}

/// Emitted when an approval is granted.
/// topics : ("approve", owner: Address, spender: Address)
/// data   : token_id: String
pub fn approve(env: &Env, owner: Address, spender: Address, token_id: String) {
    env.events()
        .publish((symbol_short!("approve"), owner, spender), token_id);
}

/// Emitted when an approval is cleared (set to None).
/// topics : ("appr_clr", owner: Address)
/// data   : token_id: String
pub fn approve_clear(env: &Env, owner: Address, token_id: String) {
    env.events()
        .publish((symbol_short!("appr_clr"), owner), token_id);
}

/// Emitted when ownership is transferred.
/// topics : ("transfer", from: Address, to: Address)
/// data   : token_id: String
pub fn transfer(env: &Env, from: Address, to: Address, token_id: String) {
    env.events()
        .publish((symbol_short!("transfer"), from, to), token_id);
}

pub fn burn(env: &Env, owner: Address, token_id: String) {
    env.events()
        .publish((symbol_short!("burn"), owner), token_id);
}
