#[cfg(test)]
mod tests {
    extern crate std;

    use soroban_sdk::{
        testutils::{Address as _, Events as _, Ledger as _},
        Address, Env, IntoVal, String,
    };

    use crate::{
        inject_stale_index_entry, ContractPaused, ContractUnpaused, NameState, RegistryContract,
        RegistryContractClient, RegistryError,
    };

    struct TimeHelper {
        pub now: u64,
    }

    impl TimeHelper {
        pub fn new() -> Self {
            Self { now: 100_000 }
        }
        pub fn future(&self, seconds: u64) -> u64 {
            self.now + seconds
        }
        pub fn past(&self, seconds: u64) -> u64 {
            self.now.saturating_sub(seconds)
        }
    }

    #[test]
    fn stores_registry_entries_in_persistent_storage() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let target = Some(String::from_str(&env, "GABC"));

        let time = TimeHelper::new();
        let expires_at = time.future(1_000);
        let grace_period_ends_at = time.future(2_000);

        client.register(
            &name,
            &owner,
            &target,
            &None::<String>,
            &time.now,
            &expires_at,
            &grace_period_ends_at,
        );

        let transfer_time = time.future(10);
        client.transfer(&name, &owner, &next_owner, &transfer_time);

        let resolved = client.resolve(&name, &transfer_time);
        assert_eq!(resolved.owner, next_owner);
        assert_eq!(client.names_for_owner(&next_owner).len(), 1);
    }

    #[test]
    fn pause_blocks_mutations_but_keeps_queries_available() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(100_000);
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "paused.xlm");
        let time = TimeHelper::new();

        client.initialize(&admin);
        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        client.pause();
        assert!(client.is_paused());
        assert!(matches!(
            client.try_register(
                &String::from_str(&env, "blocked-registration.xlm"),
                &owner,
                &None::<String>,
                &None::<String>,
                &time.now,
                &time.future(1_000),
                &time.future(2_000),
            ),
            Err(Ok(RegistryError::ContractPaused))
        ));
        assert!(matches!(
            client.try_transfer(&name, &owner, &next_owner, &time.future(10)),
            Err(Ok(RegistryError::ContractPaused))
        ));
        assert!(matches!(
            client.try_renew(
                &name,
                &owner,
                &time.future(1_500),
                &time.future(2_500),
                &time.now
            ),
            Err(Ok(RegistryError::ContractPaused))
        ));
        assert!(matches!(
            client.try_set_resolver(&name, &owner, &None::<String>, &time.now),
            Err(Ok(RegistryError::ContractPaused))
        ));
        assert!(matches!(
            client.try_set_target_address(&name, &owner, &None::<String>, &time.now),
            Err(Ok(RegistryError::ContractPaused))
        ));
        assert!(matches!(
            client.try_set_metadata(&name, &owner, &None::<String>, &time.now),
            Err(Ok(RegistryError::ContractPaused))
        ));
        assert!(matches!(
            client.try_burn(&name, &owner, &time.now),
            Err(Ok(RegistryError::ContractPaused))
        ));

        assert_eq!(client.resolve(&name, &time.now).owner, owner);
        assert_eq!(client.name_state(&name, &time.now), NameState::Active);
        assert_eq!(client.names_for_owner(&owner).len(), 1);

        client.unpause();
        assert!(!client.is_paused());
        client.transfer(&name, &owner, &next_owner, &time.future(10));
        assert_eq!(client.resolve(&name, &time.future(10)).owner, next_owner);
    }

    #[test]
    fn pause_and_unpause_emit_timestamped_events() {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(123);
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        client.pause();
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        soroban_sdk::symbol_short!("contract"),
                        soroban_sdk::symbol_short!("paused"),
                    )
                        .into_val(&env),
                    ContractPaused {
                        admin: admin.clone(),
                        timestamp: 123,
                    }
                    .into_val(&env),
                ),
            ]
        );
        client.unpause();
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    contract_id,
                    (
                        soroban_sdk::symbol_short!("contract"),
                        soroban_sdk::symbol_short!("unpaused"),
                    )
                        .into_val(&env),
                    ContractUnpaused {
                        admin,
                        timestamp: 123,
                    }
                    .into_val(&env),
                ),
            ]
        );
    }

    #[test]
    fn name_state_distinguishes_lifecycle_phases() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "phase.xlm");
        let time = TimeHelper::new();
        let expires_at = time.future(1_000);
        let grace_period_ends_at = time.future(2_000);

        // Missing before any registration exists.
        assert_eq!(client.name_state(&name, &time.now), NameState::Missing);

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &expires_at,
            &grace_period_ends_at,
        );

        // Active up to and including the expiry instant.
        assert_eq!(
            client.name_state(&name, &time.future(500)),
            NameState::Active
        );
        assert_eq!(client.name_state(&name, &expires_at), NameState::Active);
        // Grace period between expiry and the grace-period end (inclusive).
        assert_eq!(
            client.name_state(&name, &time.future(1_500)),
            NameState::GracePeriod
        );
        assert_eq!(
            client.name_state(&name, &grace_period_ends_at),
            NameState::GracePeriod
        );
        // Claimable strictly after the grace period ends.
        assert_eq!(
            client.name_state(&name, &(grace_period_ends_at + 1)),
            NameState::Claimable
        );
    }

    #[test]
    fn rejects_registration_with_expiry_before_now() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        let result = client.try_register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.past(1),
            &time.future(100),
        );

        assert!(matches!(result, Err(Ok(RegistryError::InvalidExpiry))));
    }

    #[test]
    fn rejects_registration_with_grace_period_before_expiry() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();
        let expires_at = time.future(100);

        let result = client.try_register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &expires_at,
            &time.future(99),
        );

        assert!(matches!(result, Err(Ok(RegistryError::InvalidGracePeriod))));
    }

    #[test]
    fn rejects_renewal_with_malformed_lifecycle_timestamps() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();
        let expires_at = time.future(100);
        let grace_ends_at = time.future(200);

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &expires_at,
            &grace_ends_at,
        );

        let invalid_expiry =
            client.try_renew(&name, &owner, &time.past(1), &grace_ends_at, &time.now);
        assert!(matches!(
            invalid_expiry,
            Err(Ok(RegistryError::InvalidExpiry))
        ));

        let invalid_grace_period = client.try_renew(
            &name,
            &owner,
            &time.future(150),
            &time.future(149),
            &time.now,
        );
        assert!(matches!(
            invalid_grace_period,
            Err(Ok(RegistryError::InvalidGracePeriod))
        ));
    }

    #[test]
    fn rejects_renewal_that_reduces_expiry_or_grace_period() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();
        let expires_at = time.future(100);
        let grace_ends_at = time.future(200);

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &expires_at,
            &grace_ends_at,
        );

        let reduced_expiry =
            client.try_renew(&name, &owner, &time.future(50), &grace_ends_at, &time.now);
        assert!(matches!(
            reduced_expiry,
            Err(Ok(RegistryError::InvalidExpiry))
        ));

        let reduced_grace =
            client.try_renew(&name, &owner, &expires_at, &time.future(150), &time.now);
        assert!(matches!(
            reduced_grace,
            Err(Ok(RegistryError::InvalidGracePeriod))
        ));

        let new_expires_at = time.future(200);
        let new_grace_ends_at = time.future(300);
        client.renew(
            &name,
            &owner,
            &new_expires_at,
            &new_grace_ends_at,
            &time.now,
        );
        let entry = client.resolve(&name, &time.now);
        assert_eq!(entry.expires_at, new_expires_at);
        assert_eq!(entry.grace_period_ends_at, new_grace_ends_at);
    }

    #[test]
    fn threat_unauthorized_actor_cannot_register_without_auth() {
        let env = Env::default();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.register(
                &name,
                &owner,
                &None::<String>,
                &None::<String>,
                &time.now,
                &time.future(1_000),
                &time.future(2_000),
            );
        }));

        assert!(result.is_err(), "registration without auth should fail");
        assert!(matches!(
            client.try_resolve(&name, &100),
            Err(Ok(RegistryError::NotFound))
        ));
    }

    #[test]
    fn threat_unauthorized_actor_cannot_transfer_without_auth() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        env.set_auths(&[]);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.transfer(&name, &owner, &next_owner, &time.future(10));
        }));

        assert!(result.is_err(), "transfer without auth should fail");
        let resolved = client.resolve(&name, &time.future(10));
        assert_eq!(resolved.owner, owner);
        assert_eq!(client.names_for_owner(&next_owner).len(), 0);
    }

    #[test]
    fn threat_actor_cannot_transfer_unowned_name() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        let result = client.try_transfer(&name, &attacker, &next_owner, &time.future(10));
        assert!(matches!(result, Err(Ok(RegistryError::Unauthorized))));
    }

    #[test]
    fn threat_actor_cannot_set_resolver_for_unowned_name() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        let resolver = Some(Address::generate(&env).to_string());
        let result = client.try_set_resolver(&name, &attacker, &resolver, &time.future(10));
        assert!(matches!(result, Err(Ok(RegistryError::Unauthorized))));
    }

    #[test]
    fn threat_actor_cannot_set_target_address_for_unowned_name() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        let target = Some(String::from_str(&env, "target_address"));
        let result = client.try_set_target_address(&name, &attacker, &target, &time.future(10));
        assert!(matches!(result, Err(Ok(RegistryError::Unauthorized))));
    }

    #[test]
    fn threat_actor_cannot_set_metadata_for_unowned_name() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        let metadata = Some(String::from_str(&env, "ipfs://hash"));
        let result = client.try_set_metadata(&name, &attacker, &metadata, &time.future(10));
        assert!(matches!(result, Err(Ok(RegistryError::Unauthorized))));
    }

    #[test]
    fn threat_actor_cannot_renew_unowned_name() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let attacker = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        let result = client.try_renew(
            &name,
            &attacker,
            &time.future(1500),
            &time.future(2500),
            &time.future(10),
        );
        assert!(matches!(result, Err(Ok(RegistryError::Unauthorized))));
    }

    #[test]
    fn declares_that_admin_recovery_is_not_supported() {
        let env = Env::default();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        assert!(!client.supports_admin_recovery());
    }

    // ---- issue #303: owner-index audit helper ----

    #[test]
    fn audit_owner_index_is_empty_for_consistent_state() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "audit-ok.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        let stale = client.audit_owner_index(&owner);
        assert_eq!(
            stale.len(),
            0,
            "expected no stale entries after normal registration"
        );
    }

    #[test]
    fn audit_owner_index_is_empty_after_transfer() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "audit-xfer.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );
        client.transfer(&name, &owner, &next_owner, &time.future(10));

        // Both old and new owner indices must be consistent after transfer.
        assert_eq!(
            client.audit_owner_index(&owner).len(),
            0,
            "previous owner index should be clean after transfer"
        );
        assert_eq!(
            client.audit_owner_index(&next_owner).len(),
            0,
            "new owner index should be clean after transfer"
        );
    }

    #[test]
    fn audit_owner_index_is_empty_for_unknown_owner() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let stranger = Address::generate(&env);
        let stale = client.audit_owner_index(&stranger);
        assert_eq!(
            stale.len(),
            0,
            "unknown owner should have an empty audit result"
        );
    }

    #[test]
    fn audit_owner_index_detects_stale_index_entry() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let phantom = String::from_str(&env, "phantom.xlm");

        // Inject an index entry that has no backing registry entry, simulating
        // the kind of inconsistency that a failed storage migration could leave.
        env.as_contract(&contract_id, || {
            inject_stale_index_entry(&env, &owner, &phantom);
        });

        let stale = client.audit_owner_index(&owner);
        assert_eq!(stale.len(), 1, "expected exactly one stale entry");
        assert_eq!(stale.get(0).unwrap(), phantom);
    }

    // ---- issue #304: transfer event schema ----
    //
    // The transfer event schema is:
    //   topics: ("name", "transfer")
    //   data:   (name: String, old_owner: Address, new_owner: Address)
    //
    // Indexers that subscribe to the registry contract can rely on this layout
    // to track ownership changes.

    #[test]
    fn transfer_emits_event_with_name_old_owner_and_new_owner() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "event-xfer.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(1_000),
            &time.future(2_000),
        );

        client.transfer(&name, &owner, &next_owner, &time.future(10));

        // Verify the exact event schema so indexers can depend on a stable layout.
        use soroban_sdk::IntoVal;
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        soroban_sdk::symbol_short!("name"),
                        soroban_sdk::symbol_short!("transfer"),
                    )
                        .into_val(&env),
                    (name.clone(), owner.clone(), next_owner.clone()).into_val(&env),
                )
            ]
        );
    }

    #[test]
    fn transfer_event_is_emitted_once_per_transfer() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let second = Address::generate(&env);
        let third = Address::generate(&env);
        let name = String::from_str(&env, "multi-xfer.xlm");
        let time = TimeHelper::new();

        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &time.future(10_000),
            &time.future(20_000),
        );

        // Verify each transfer individually — Soroban's testutils scope events
        // per invocation, so we check the schema after each call.
        use soroban_sdk::IntoVal;

        client.transfer(&name, &owner, &second, &time.future(10));
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        soroban_sdk::symbol_short!("name"),
                        soroban_sdk::symbol_short!("transfer"),
                    )
                        .into_val(&env),
                    (name.clone(), owner.clone(), second.clone()).into_val(&env),
                ),
            ],
            "first transfer event should carry (name, old_owner, new_owner)"
        );

        client.transfer(&name, &second, &third, &time.future(20));
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        soroban_sdk::symbol_short!("name"),
                        soroban_sdk::symbol_short!("transfer"),
                    )
                        .into_val(&env),
                    (name.clone(), second.clone(), third.clone()).into_val(&env),
                ),
            ],
            "second transfer event should carry (name, old_owner, new_owner)"
        );
    }

    #[test]
    fn dispute_lock_blocks_mutations_but_not_resolution_and_expires_automatically() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let dispute_admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "locked.xlm");
        let time = TimeHelper::new();
        let expires_at = time.future(1_000);
        let grace_ends_at = time.future(2_000);

        client.initialize(&admin);
        client.set_dispute_admin(&dispute_admin);
        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &expires_at,
            &grace_ends_at,
        );

        let lock_reason = String::from_str(&env, "dispute review");
        let locked_until = time.future(50);
        client.lock_name(
            &name,
            &dispute_admin,
            &locked_until,
            &lock_reason,
            &time.now,
        );

        let resolved = client.resolve(&name, &time.future(10));
        assert_eq!(
            resolved.owner, owner,
            "resolution should stay live while locked"
        );

        assert!(matches!(
            client.try_transfer(&name, &owner, &next_owner, &time.future(10)),
            Err(Ok(RegistryError::Locked))
        ));
        assert!(matches!(
            client.try_set_metadata(
                &name,
                &owner,
                &Some(String::from_str(&env, "ipfs://new-metadata")),
                &time.future(10)
            ),
            Err(Ok(RegistryError::Locked))
        ));
        assert!(matches!(
            client.try_renew(
                &name,
                &owner,
                &time.future(1_500),
                &time.future(2_500),
                &time.future(10)
            ),
            Err(Ok(RegistryError::Locked))
        ));

        client.transfer(&name, &owner, &next_owner, &time.future(60));
        let resolved_after_expiry = client.resolve(&name, &time.future(60));
        assert_eq!(resolved_after_expiry.owner, next_owner);
    }

    #[test]
    fn dispute_lock_unlock_emits_events_and_allows_early_release() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let dispute_admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let next_owner = Address::generate(&env);
        let name = String::from_str(&env, "unlock.xlm");
        let time = TimeHelper::new();
        let expires_at = time.future(1_000);
        let grace_ends_at = time.future(2_000);

        client.initialize(&admin);
        client.set_dispute_admin(&dispute_admin);
        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &expires_at,
            &grace_ends_at,
        );

        let lock_reason = String::from_str(&env, "ownership dispute");
        let locked_until = time.future(500);

        use soroban_sdk::IntoVal;

        client.lock_name(
            &name,
            &dispute_admin,
            &locked_until,
            &lock_reason,
            &time.now,
        );
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        soroban_sdk::symbol_short!("name"),
                        soroban_sdk::symbol_short!("lck_apld"),
                    )
                        .into_val(&env),
                    crate::LockApplied {
                        name: name.clone(),
                        locked_until,
                        lock_reason: lock_reason.clone(),
                        admin: dispute_admin.clone(),
                    }
                    .into_val(&env),
                ),
            ]
        );

        client.unlock_name(&name, &dispute_admin);
        assert_eq!(
            env.events().all(),
            soroban_sdk::vec![
                &env,
                (
                    contract_id.clone(),
                    (
                        soroban_sdk::symbol_short!("name"),
                        soroban_sdk::symbol_short!("lck_rmvd"),
                    )
                        .into_val(&env),
                    crate::LockRemoved {
                        name: name.clone(),
                        locked_until,
                        lock_reason: lock_reason.clone(),
                        admin: dispute_admin.clone(),
                    }
                    .into_val(&env),
                ),
            ]
        );

        client.transfer(&name, &owner, &next_owner, &time.future(10));
        let resolved = client.resolve(&name, &time.future(10));
        assert_eq!(resolved.owner, next_owner);
    }

    #[test]
    fn non_dispute_admin_cannot_lock_or_unlock_names() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(RegistryContract, ());
        let client = RegistryContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        let dispute_admin = Address::generate(&env);
        let attacker = Address::generate(&env);
        let owner = Address::generate(&env);
        let name = String::from_str(&env, "governed.xlm");
        let time = TimeHelper::new();
        let expires_at = time.future(1_000);
        let grace_ends_at = time.future(2_000);

        client.initialize(&admin);
        client.set_dispute_admin(&dispute_admin);
        client.register(
            &name,
            &owner,
            &None::<String>,
            &None::<String>,
            &time.now,
            &expires_at,
            &grace_ends_at,
        );

        let reason = String::from_str(&env, "dispute");
        let lock_attempt =
            client.try_lock_name(&name, &attacker, &time.future(10), &reason, &time.now);
        assert!(matches!(lock_attempt, Err(Ok(RegistryError::Unauthorized))));

        let unlock_attempt = client.try_unlock_name(&name, &attacker);
        assert!(matches!(
            unlock_attempt,
            Err(Ok(RegistryError::Unauthorized))
        ));
    }
}
