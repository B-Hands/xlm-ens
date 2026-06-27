use soroban_sdk::{testutils::Address as _, Address, Env, String};

use xlm_ns_registry::{NameState, RegistryContract, RegistryContractClient};
use xlm_ns_resolver::ResolverContract;
use xlm_ns_subdomain::SubdomainContract;

#[test]
fn subdomain_flow_covers_controller_delegation_transfer_and_resolution() {
    let env = Env::default();

    let subdomain_contract_id = env.register(SubdomainContract, ());
    let resolver_contract_id = env.register(ResolverContract, ());

    let subdomain = xlm_ns_subdomain::SubdomainContractClient::new(&env, &subdomain_contract_id);
    let resolver = xlm_ns_resolver::ResolverContractClient::new(&env, &resolver_contract_id);

    let parent_owner = Address::generate(&env);
    let controller = Address::generate(&env);
    let intruder = Address::generate(&env);
    let subdomain_owner = Address::generate(&env);
    let next_owner = Address::generate(&env);

    let parent = String::from_str(&env, "timmy.xlm");
    let label = String::from_str(&env, "pay");
    let fqdn = String::from_str(&env, "pay.timmy.xlm");
    let first_address = String::from_str(&env, "GABC");
    let second_address = String::from_str(&env, "GDEF");

    subdomain.register_parent(&parent, &parent_owner);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            subdomain.add_controller(&parent, &intruder, &controller);
        }))
        .is_err(),
        "non-owner should not be able to add a controller"
    );

    subdomain.add_controller(&parent, &parent_owner, &controller);

    let parent_record = subdomain.parent(&parent).unwrap();
    assert_eq!(parent_record.owner, parent_owner);
    assert!(parent_record.controllers.contains(&controller));

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            subdomain.create(&label, &parent, &intruder, &subdomain_owner, &100);
        }))
        .is_err(),
        "unauthorized caller should not be able to create a subdomain"
    );

    let created_name = subdomain.create(&label, &parent, &controller, &subdomain_owner, &101);
    assert_eq!(created_name, fqdn);
    assert!(subdomain.exists(&fqdn));

    let created_record = subdomain.record(&fqdn).unwrap();
    assert_eq!(created_record.parent, parent);
    assert_eq!(created_record.owner, subdomain_owner);
    assert_eq!(created_record.created_at, 101);

    resolver.set_record(&fqdn, &subdomain_owner, &first_address, &102);
    assert!(resolver.has_record(&fqdn));
    resolver.set_primary_name(&first_address, &subdomain_owner, &fqdn);

    let resolved_before_transfer = resolver.resolve(&fqdn).unwrap();
    assert_eq!(resolved_before_transfer.owner, subdomain_owner);
    assert_eq!(
        resolver.get_stellar_address(&fqdn),
        Some(first_address.clone())
    );
    assert_eq!(resolver.reverse(&first_address), Some(fqdn.clone()));

    // Transfer subdomain ownership, then update resolver ownership explicitly.
    subdomain.transfer(&fqdn, &subdomain_owner, &next_owner);
    resolver.transfer_record_owner(&fqdn, &subdomain_owner, &next_owner);

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Old resolver record owner (subdomain_owner) should no longer be able to transfer resolver record ownership
            resolver.transfer_record_owner(&fqdn, &subdomain_owner, &controller);
        }))
        .is_err(),
        "previous owner should not be able to transfer after ownership changes"
    );

    let transferred_record = subdomain.record(&fqdn).unwrap();
    assert_eq!(transferred_record.owner, next_owner);

    // Verify old subdomain owner cannot update resolver record
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            resolver.set_record(&fqdn, &subdomain_owner, &second_address, &103);
        }))
        .is_err(),
        "old subdomain owner should not be able to update resolver record after transfer"
    );

    // New subdomain owner can now update resolver record
    resolver.set_record(&fqdn, &next_owner, &second_address, &103);
    resolver.set_primary_name(&second_address, &next_owner, &fqdn);

    let resolved_after_transfer = resolver.resolve(&fqdn).unwrap();
    assert_eq!(resolved_after_transfer.owner, next_owner);
    assert_eq!(
        resolver.get_stellar_address(&fqdn),
        Some(second_address.clone())
    );
    assert_eq!(resolver.reverse(&second_address), Some(fqdn.clone()));

    // Test deletion of subdomain and its effect on resolver (none, resolver record persists)
    subdomain.delete(&fqdn, &next_owner);
    assert!(!subdomain.exists(&fqdn));
    // Resolver record should still exist, and only the last owner (next_owner) can modify it.
    assert!(resolver.has_record(&fqdn));
    assert_eq!(resolver.resolve(&fqdn).unwrap().owner, next_owner);
}

#[test]
fn parent_expiry_purges_subdomains_and_allows_reregistration() {
    let env = Env::default();
    env.mock_all_auths();

    let registry_contract_id = env.register(RegistryContract, ());
    let subdomain_contract_id = env.register(SubdomainContract, ());

    let registry = RegistryContractClient::new(&env, &registry_contract_id);
    let subdomain = xlm_ns_subdomain::SubdomainContractClient::new(&env, &subdomain_contract_id);

    let admin = Address::generate(&env);
    let old_parent_owner = Address::generate(&env);
    let new_parent_owner = Address::generate(&env);
    let old_subdomain_owner = Address::generate(&env);
    let new_subdomain_owner = Address::generate(&env);
    let controller = Address::generate(&env);

    let parent = String::from_str(&env, "alice.xlm");
    let label = String::from_str(&env, "pay");
    let fqdn = String::from_str(&env, "pay.alice.xlm");

    let start = 1_000_000u64;
    let expiry = start + 1_000;
    let grace_end = expiry + 1_000;
    let renewed_expiry = grace_end + 1_000;
    let renewed_grace_end = renewed_expiry + 1_000;

    env.ledger().set_timestamp(start);

    registry.initialize(&admin).unwrap();
    subdomain.initialize(&admin).unwrap();
    subdomain
        .set_registry_contract(&registry_contract_id)
        .unwrap();

    registry
        .register(
            &parent,
            &old_parent_owner,
            &None::<String>,
            &None::<String>,
            &start,
            &expiry,
            &grace_end,
        )
        .unwrap();
    subdomain
        .register_parent(&parent, &old_parent_owner)
        .unwrap();
    subdomain
        .add_controller(&parent, &old_parent_owner, &controller)
        .unwrap();

    let first_fqdn = subdomain
        .create(
            &label,
            &parent,
            &controller,
            &old_subdomain_owner,
            &(start + 1),
        )
        .unwrap();
    assert_eq!(first_fqdn, fqdn);
    assert!(subdomain.exists(&fqdn));
    assert_eq!(subdomain.subdomains_for_parent(&parent).len(), 1);
    assert_eq!(
        subdomain.subdomains_for_owner(&old_subdomain_owner).len(),
        1
    );

    env.ledger().set_timestamp(expiry + 1);
    assert_eq!(
        registry.name_state(&parent, &(expiry + 1)),
        NameState::GracePeriod
    );

    assert!(
        subdomain.record(&fqdn).is_none(),
        "subdomain record should be hidden once the parent enters grace"
    );
    assert!(!subdomain.exists(&fqdn));
    assert!(subdomain.subdomains_for_parent(&parent).is_empty());
    assert!(subdomain
        .subdomains_for_owner(&old_subdomain_owner)
        .is_empty());

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            subdomain.transfer(&fqdn, &old_subdomain_owner, &new_subdomain_owner);
        }))
        .is_err(),
        "transfer attempts must fail once the parent is in grace"
    );

    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            subdomain.create(
                &label,
                &parent,
                &controller,
                &new_subdomain_owner,
                &(expiry + 2),
            );
        }))
        .is_err(),
        "creation must fail once the parent is in grace"
    );

    env.ledger().set_timestamp(grace_end + 1);
    assert_eq!(
        registry.name_state(&parent, &(grace_end + 1)),
        NameState::Claimable
    );
    assert!(subdomain.parent(&parent).is_none());

    registry
        .register(
            &parent,
            &new_parent_owner,
            &None::<String>,
            &None::<String>,
            &(grace_end + 1),
            &renewed_expiry,
            &renewed_grace_end,
        )
        .unwrap();
    subdomain
        .register_parent(&parent, &new_parent_owner)
        .unwrap();

    let recreated_fqdn = subdomain
        .create(
            &label,
            &parent,
            &new_parent_owner,
            &new_subdomain_owner,
            &(grace_end + 2),
        )
        .unwrap();

    assert_eq!(recreated_fqdn, fqdn);
    assert!(subdomain.exists(&fqdn));
    assert_eq!(subdomain.record(&fqdn).unwrap().owner, new_subdomain_owner);
    assert_eq!(subdomain.subdomains_for_parent(&parent).len(), 1);
    assert_eq!(
        subdomain.subdomains_for_owner(&new_subdomain_owner).len(),
        1
    );
    assert!(subdomain
        .subdomains_for_owner(&old_subdomain_owner)
        .is_empty());
}

#[test]
fn subdomain_transfer_attempts_fail_during_parent_grace_period() {
    let env = Env::default();
    env.mock_all_auths();

    let registry_contract_id = env.register(RegistryContract, ());
    let subdomain_contract_id = env.register(SubdomainContract, ());

    let registry = RegistryContractClient::new(&env, &registry_contract_id);
    let subdomain = xlm_ns_subdomain::SubdomainContractClient::new(&env, &subdomain_contract_id);

    let admin = Address::generate(&env);
    let parent_owner = Address::generate(&env);
    let subdomain_owner = Address::generate(&env);
    let next_owner = Address::generate(&env);

    let parent = String::from_str(&env, "bob.xlm");
    let fqdn = String::from_str(&env, "pay.bob.xlm");

    let start = 2_000_000u64;
    let expiry = start + 500;
    let grace_end = expiry + 500;

    env.ledger().set_timestamp(start);

    registry.initialize(&admin).unwrap();
    subdomain.initialize(&admin).unwrap();
    subdomain
        .set_registry_contract(&registry_contract_id)
        .unwrap();

    registry
        .register(
            &parent,
            &parent_owner,
            &None::<String>,
            &None::<String>,
            &start,
            &expiry,
            &grace_end,
        )
        .unwrap();
    subdomain.register_parent(&parent, &parent_owner).unwrap();
    subdomain
        .create(
            &String::from_str(&env, "pay"),
            &parent,
            &parent_owner,
            &subdomain_owner,
            &(start + 1),
        )
        .unwrap();

    env.ledger().set_timestamp(expiry + 1);
    assert_eq!(
        registry.name_state(&parent, &(expiry + 1)),
        NameState::GracePeriod
    );

    let transfer_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        subdomain.transfer(&fqdn, &subdomain_owner, &next_owner);
    }));
    assert!(
        transfer_result.is_err(),
        "subdomain transfer should fail while the parent is in grace"
    );

    let delete_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        subdomain.delete(&fqdn, &subdomain_owner);
    }));
    assert!(
        delete_result.is_err(),
        "subdomain deletion should fail while the parent is in grace"
    );

    let revoke_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        subdomain.revoke(&fqdn, &parent_owner);
    }));
    assert!(
        revoke_result.is_err(),
        "subdomain revocation should fail while the parent is in grace"
    );

    assert!(subdomain.record(&fqdn).is_none());
    assert!(!subdomain.exists(&fqdn));
}
