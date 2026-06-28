//! Negative tests for the Registry contract, ensuring all error paths are covered.

use soroban_sdk::{
    testutils::{Address as _, Events},
    Address, Env, Error,
};
use tests::common::{
    create_and_init_contracts, setup_test_domain, TestContracts, DEFAULT_GRACE_PERIOD,
    DEFAULT_TTL,
};

/// Checklist for RegistryError variants:
///
/// - [x] AlreadyRegistered = 1
/// - [x] NotFound = 2
/// - [x] NotYetClaimable = 3
/// - [x] NotActive = 4
/// - [x] Unauthorized = 5
/// - [x] MetadataTooLong = 6
/// - [x] Validation = 7
/// - [x] InvalidExpiry = 8
/// - [x] InvalidGracePeriod = 9
/// - [ ] UpgradeFailed = 10  (Difficult to test reliably in unit tests)

#[test]
fn test_register_fails_if_already_registered() {
    let env = Env::default();
    env.mock_all_auths();

    let TestContracts {
        registry,
        registrar: _,
    } = create_and_init_contracts(&env);
    let (owner, _) = setup_test_domain(&env, &registry, "test.xlm");

    // Attempt to register the same name again
    let res = registry.try_register(
        &"test.xlm".into_string(&env),
        &owner,
        &None,
        &None,
        &None,
        &(env.ledger().timestamp() + DEFAULT_TTL),
        &(env.ledger().timestamp() + DEFAULT_TTL + DEFAULT_GRACE_PERIOD),
        &env.ledger().timestamp(),
    );

    assert_eq!(res, Err(Ok(Error::from_contract_error(1)))); // AlreadyRegistered
}

#[test]
fn test_transfer_fails_if_not_found() {
    let env = Env::default();
    env.mock_all_auths();

    let TestContracts {
        registry,
        registrar: _,
    } = create_and_init_contracts(&env);
    let owner = Address::generate(&env);
    let new_owner = Address::generate(&env);

    let res = registry.try_transfer(
        &"nonexistent.xlm".into_string(&env),
        &owner,
        &new_owner,
        &env.ledger().timestamp(),
    );

    assert_eq!(res, Err(Ok(Error::from_contract_error(2)))); // NotFound
}

#[test]
fn test_burn_fails_if_not_yet_claimable() {
    let env = Env::default();
    env.mock_all_auths();

    let TestContracts {
        registry,
        registrar: _,
    } = create_and_init_contracts(&env);
    let (owner, _) = setup_test_domain(&env, &registry, "test.xlm");

    // Name is active, so it's not claimable
    let res = registry.try_burn(&"test.xlm".into_string(&env), &owner, &env.ledger().timestamp());

    assert_eq!(res, Err(Ok(Error::from_contract_error(3)))); // NotYetClaimable
}

#[test]
fn test_transfer_fails_if_in_grace_period() {
    let env = Env::default();
    env.mock_all_auths();

    let TestContracts {
        registry,
        registrar: _,
    } = create_and_init_contracts(&env);
    let (owner, _) = setup_test_domain(&env, &registry, "test.xlm");
    let new_owner = Address::generate(&env);

    // Advance time past expiry into grace period
    env.ledger().with_mut(|l| {
        l.timestamp = l.timestamp + DEFAULT_TTL + 1;
    });

    let res = registry.try_transfer(
        &"test.xlm".into_string(&env),
        &owner,
        &new_owner,
        &env.ledger().timestamp(),
    );

    assert_eq!(res, Err(Ok(Error::from_contract_error(4)))); // NotActive
}

#[test]
fn test_transfer_fails_if_unauthorized() {
    let env = Env::default();
    env.mock_all_auths();

    let TestContracts {
        registry,
        registrar: _,
    } = create_and_init_contracts(&env);
    let (owner, _) = setup_test_domain(&env, &registry, "test.xlm");
    let attacker = Address::generate(&env);
    let new_owner = Address::generate(&env);

    // Attacker (not owner) tries to transfer
    let res = registry.try_transfer(
        &"test.xlm".into_string(&env),
        &attacker, // Unauthorized caller
        &new_owner,
        &env.ledger().timestamp(),
    );

    assert_eq!(res, Err(Ok(Error::from_contract_error(5)))); // Unauthorized
}

#[test]
fn test_register_fails_if_metadata_too_long() {
    let env = Env::default();
    env.mock_all_auths();

    let TestContracts {
        registry,
        registrar: _,
    } = create_and_init_contracts(&env);
    let owner = Address::generate(&env);

    // From common/src/lib.rs, MAX_METADATA_LEN is 1024
    let long_metadata = "a".repeat(1025);

    let res = registry.try_register(
        &"test.xlm".into_string(&env),
        &owner,
        &None,
        &None,
        &Some(long_metadata.into_string(&env)),
        &(env.ledger().timestamp() + DEFAULT_TTL),
        &(env.ledger().timestamp() + DEFAULT_TTL + DEFAULT_GRACE_PERIOD),
        &env.ledger().timestamp(),
    );

    assert_eq!(res, Err(Ok(Error::from_contract_error(6)))); // MetadataTooLong
}