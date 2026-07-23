#[cfg(test)]
#[allow(deprecated)]
mod tests {
    extern crate std;

    use soroban_sdk::{
        testutils::{Address as _, Events as _},
        Address, Env, String,
    };

    use crate::{SubdomainContract, SubdomainContractClient, DEFAULT_MAX_SUBDOMAINS_PER_PARENT};

    #[test]
    fn register_parent_emits_event() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        assert_eq!(env.events().all().events().len(), 1);
    }

    #[test]
    fn create_subdomain_emits_event() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);
        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &owner,
            &sub_owner,
            &100,
        );

        assert_eq!(fqdn, String::from_str(&env, "pay.timmy.xlm"));
        assert!(!env.events().all().events().is_empty());
    }

    #[test]
    fn transfer_emits_event() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);
        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &owner,
            &sub_owner,
            &100,
        );

        client.transfer(&fqdn, &sub_owner, &new_owner);

        assert!(!env.events().all().events().is_empty());
        assert_eq!(client.record(&fqdn).unwrap().owner, new_owner);
    }

    #[test]
    fn revoke_emits_event() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);
        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &owner,
            &sub_owner,
            &100,
        );

        client.revoke(&fqdn, &sub_owner);

        assert!(!env.events().all().events().is_empty());
        assert!(!client.exists(&fqdn));
    }

    #[test]
    fn add_and_remove_controller_emit_events() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let controller = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        client.add_controller(&parent, &owner, &controller);
        assert!(!env.events().all().events().is_empty());

        client.remove_controller(&parent, &owner, &controller);
        assert!(!env.events().all().events().is_empty());
    }

    #[test]
    fn stores_subdomain_records_in_contract_storage() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let controller = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);
        client.add_controller(&parent, &owner, &controller);

        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &controller,
            &sub_owner,
            &100,
        );

        assert_eq!(fqdn, String::from_str(&env, "pay.timmy.xlm"));
        assert!(client.exists(&fqdn));
        assert_eq!(client.record(&fqdn).unwrap().owner, sub_owner);
    }

    #[test]
    fn removes_controller_and_revokes_authority() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let controller = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        // Add controller
        client.add_controller(&parent, &owner, &controller);

        // Remove controller
        client.remove_controller(&parent, &owner, &controller);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.create(
                &String::from_str(&env, "pay"),
                &parent,
                &controller,
                &sub_owner,
                &100,
            );
        }));
        assert!(result.is_err(), "post-removal create should fail");
    }

    #[test]
    fn prevents_parent_takeover() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let intruder = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.register_parent(&parent, &intruder);
        }));

        assert!(result.is_err(), "intruder parent registration should fail");

        let parent_record = client.parent(&parent).unwrap();
        assert_eq!(
            parent_record.owner, owner,
            "original owner should be preserved"
        );
    }

    #[test]
    fn subdomain_owner_can_revoke() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let parent_owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &parent_owner);
        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &parent_owner,
            &sub_owner,
            &100,
        );

        assert!(client.exists(&fqdn));
        client.revoke(&fqdn, &sub_owner);
        assert!(!client.exists(&fqdn));
    }

    #[test]
    fn parent_owner_can_revoke() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let parent_owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &parent_owner);
        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &parent_owner,
            &sub_owner,
            &100,
        );

        assert!(client.exists(&fqdn));
        client.revoke(&fqdn, &parent_owner);
        assert!(!client.exists(&fqdn));
    }

    #[test]
    fn parent_controller_can_revoke() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let parent_owner = Address::generate(&env);
        let controller = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &parent_owner);
        client.add_controller(&parent, &parent_owner, &controller);

        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &controller,
            &sub_owner,
            &100,
        );

        assert!(client.exists(&fqdn));
        client.revoke(&fqdn, &controller);
        assert!(!client.exists(&fqdn));
    }

    #[test]
    fn unauthorized_caller_cannot_revoke() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let parent_owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let intruder = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &parent_owner);
        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &parent_owner,
            &sub_owner,
            &100,
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.revoke(&fqdn, &intruder);
        }));
        assert!(result.is_err(), "unauthorized revocation should fail");
        assert!(client.exists(&fqdn));
    }

    #[test]
    fn rejects_duplicate_parent_registration() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.register_parent(&parent, &owner);
        }));
        assert!(result.is_err(), "duplicate parent registration should fail");
    }

    #[test]
    fn rejects_unauthorized_subdomain_creation() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let intruder = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.create(
                &String::from_str(&env, "pay"),
                &parent,
                &intruder,
                &sub_owner,
                &100,
            );
        }));
        assert!(result.is_err(), "unauthorized create should fail");
    }

    #[test]
    fn rejects_unauthorized_controller_addition() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let intruder = Address::generate(&env);
        let controller = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.add_controller(&parent, &intruder, &controller);
        }));
        assert!(
            result.is_err(),
            "intruder should not be able to add a controller"
        );

        let parent_record = client.parent(&parent).unwrap();
        assert!(!parent_record.controllers.contains(&controller));
    }

    #[test]
    fn transfers_subdomain_ownership_and_queries_existence() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let new_sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        let fqdn = client.create(
            &String::from_str(&env, "pay"),
            &parent,
            &owner,
            &sub_owner,
            &100,
        );

        assert!(client.exists(&fqdn));
        assert_eq!(client.record(&fqdn).unwrap().owner, sub_owner);

        client.transfer(&fqdn, &sub_owner, &new_sub_owner);
        assert_eq!(client.record(&fqdn).unwrap().owner, new_sub_owner);

        let intruder = Address::generate(&env);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.transfer(&fqdn, &intruder, &new_sub_owner);
        }));
        assert!(result.is_err(), "unauthorized transfer should fail");

        assert_eq!(client.record(&fqdn).unwrap().owner, new_sub_owner);
    }

    #[test]
    fn version_is_exposed() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);
        assert_eq!(client.version(), 1);
    }

    #[test]
    fn subdomain_depth_limit_is_enforced() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);

        // Should succeed (depth 3)
        client.create(
            &String::from_str(&env, "a.b.c"),
            &parent,
            &owner,
            &sub_owner,
            &100,
        );

        // Should fail (depth 4)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.create(
                &String::from_str(&env, "a.b.c.d"),
                &parent,
                &owner,
                &sub_owner,
                &100,
            );
        }));
        assert!(result.is_err(), "subdomain depth > 3 should fail");
    }

    #[test]
    fn max_depth_can_be_configured_by_admin() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        let owner = Address::generate(&env);
        let sub_owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");

        client.register_parent(&parent, &owner);
        client.set_max_depth(&4);

        // Should succeed (depth 4)
        client.create(
            &String::from_str(&env, "a.b.c.d"),
            &parent,
            &owner,
            &sub_owner,
            &100,
        );

        // Should fail (depth 5)
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.create(
                &String::from_str(&env, "a.b.c.d.e"),
                &parent,
                &owner,
                &sub_owner,
                &100,
            );
        }));
        assert!(result.is_err(), "subdomain depth > 4 should fail");
    }

    #[test]
    fn default_max_subdomains_per_parent_is_enforced() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");
        client.initialize(&admin);
        client.register_parent(&parent, &owner);

        assert_eq!(
            client.get_max_subdomains_per_parent(),
            DEFAULT_MAX_SUBDOMAINS_PER_PARENT
        );
        for idx in 0..DEFAULT_MAX_SUBDOMAINS_PER_PARENT {
            client.create(
                &String::from_str(&env, &format!("sub{idx}")),
                &parent,
                &owner,
                &owner,
                &100,
            );
        }

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.create(
                &String::from_str(&env, "one-too-many"),
                &parent,
                &owner,
                &owner,
                &100,
            );
        }));
        assert!(
            result.is_err(),
            "creation beyond the default limit should fail"
        );
    }

    #[test]
    fn admin_can_adjust_max_subdomains_per_parent() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        client.initialize(&admin);

        client.set_max_subdomains_per_parent(&2);
        assert_eq!(client.get_max_subdomains_per_parent(), 2);
    }

    #[test]
    fn deleting_subdomain_frees_parent_slot() {
        let env = Env::default();
        env.mock_all_auths_allowing_non_root_auth();
        let contract_id = env.register_contract(None, SubdomainContract);
        let client = SubdomainContractClient::new(&env, &contract_id);
        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let parent = String::from_str(&env, "timmy.xlm");
        client.initialize(&admin);
        client.register_parent(&parent, &owner);
        client.set_max_subdomains_per_parent(&1);

        let first = client.create(
            &String::from_str(&env, "first"),
            &parent,
            &owner,
            &owner,
            &100,
        );
        client.delete(&first, &owner);

        let second = client.create(
            &String::from_str(&env, "second"),
            &parent,
            &owner,
            &owner,
            &101,
        );
        assert_eq!(second, String::from_str(&env, "second.timmy.xlm"));
    }
}
