#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::Address as _, Address, Env, String, Vec};
    use xlm_ns_common::{MAX_TEXT_RECORD_VALUE_LENGTH, MAX_TEXT_RECORDS};

    use crate::{ResolverContract, ResolverContractClient};

    #[test]
    fn persists_forward_reverse_and_primary_resolution_records() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let address = String::from_str(&env, "GABC");

        client.set_record(&name, &owner, &address, &100);
        client.set_text_record(
            &name,
            &owner,
            &String::from_str(&env, "com.twitter"),
            &String::from_str(&env, "@timmy"),
            &101,
        );
        client.set_primary_name(&address, &owner, &name);

        let record = client.resolve(&name).unwrap();
        assert_eq!(record.owner, owner);
        assert_eq!(
            record
                .addresses
                .get(String::from_str(&env, "stellar")),
            Some(address.clone())
        );
        assert_eq!(
            record
                .text_records
                .get(String::from_str(&env, "com.twitter")),
            Some(String::from_str(&env, "@timmy"))
        );
        assert_eq!(record.updated_at, 101);
        assert_eq!(client.reverse(&String::from_str(&env, "GABC")), Some(name));
    }

    #[test]
    fn removes_forward_reverse_and_primary_records() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let address = String::from_str(&env, "GABC");

        client.set_record(&name, &owner, &address, &100);
        client.set_primary_name(&address, &owner, &name);
        client.remove_record(&name, &owner);

        assert_eq!(client.resolve(&name), None);
        assert_eq!(client.reverse(&address), None);
    }

    #[test]
    fn rejects_text_record_updates_from_non_owner() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let intruder = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let address = String::from_str(&env, "GABC");

        client.set_record(&name, &owner, &address, &100);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.set_text_record(
                &name,
                &intruder,
                &String::from_str(&env, "com.twitter"),
                &String::from_str(&env, "@timmy"),
                &101,
            );
        }));

        assert!(result.is_err(), "non-owner text update should fail");
        let stored = client.resolve(&name).unwrap();
        assert_eq!(stored.text_records.len(), 0);
    }

    #[test]
    fn rejects_record_removal_from_non_owner() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let intruder = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let address = String::from_str(&env, "GABC");

        client.set_record(&name, &owner, &address, &100);

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.remove_record(&name, &intruder);
        }));

        assert!(result.is_err(), "non-owner record removal should fail");
        assert!(client.resolve(&name).is_some());
        assert_eq!(client.reverse(&address), Some(name));
    }

    #[test]
    fn enforces_text_record_limit_but_allows_updating_existing_key_at_limit() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let address = String::from_str(&env, "GABC");

        client.set_record(&name, &owner, &address, &100);

        for idx in 0..MAX_TEXT_RECORDS {
            client.set_text_record(
                &name,
                &owner,
                &String::from_str(&env, &format!("key-{idx}")),
                &String::from_str(&env, &format!("value-{idx}")),
                &(101 + idx as u64),
            );
        }

        client.set_text_record(
            &name,
            &owner,
            &String::from_str(&env, "key-0"),
            &String::from_str(&env, "updated"),
            &500,
        );

        let updated_record = client.resolve(&name).unwrap();
        assert_eq!(updated_record.text_records.len(), MAX_TEXT_RECORDS as u32);
        assert_eq!(
            updated_record
                .text_records
                .get(String::from_str(&env, "key-0")),
            Some(String::from_str(&env, "updated"))
        );
        assert_eq!(updated_record.updated_at, 500);

        let overflow = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.set_text_record(
                &name,
                &owner,
                &String::from_str(&env, "overflow"),
                &String::from_str(&env, "value"),
                &501,
            );
        }));

        assert!(
            overflow.is_err(),
            "adding a new key past the limit should fail"
        );
    }

    #[test]
    fn reverse_lookup_prefers_primary_name_when_present() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let first_name = String::from_str(&env, "timmy.xlm");
        let second_name = String::from_str(&env, "pay.timmy.xlm");
        let address = String::from_str(&env, "GABC");

        client.set_record(&first_name, &owner, &address, &100);
        client.set_record(&second_name, &owner, &address, &101);

        assert_eq!(client.reverse(&address), Some(second_name.clone()));

        client.set_primary_name(&address, &owner, &first_name);
        assert_eq!(client.reverse(&address), Some(first_name));
    }

    // Issue #316: Test primary-name cleanup when resolver addresses change
    #[test]
    fn removes_old_primary_mappings_when_address_changes() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let old_address = String::from_str(&env, "GABC");
        let new_address = String::from_str(&env, "GDEF");

        client.set_record(&name, &owner, &old_address, &100);
        client.set_primary_name(&old_address, &owner, &name);

        // Verify primary name is set for old address
        assert_eq!(client.reverse(&old_address), Some(name.clone()));

        // Change address
        client.set_record(&name, &owner, &new_address, &101);

        // Old primary mapping should be cleaned up
        assert_eq!(client.reverse(&old_address), None);
        assert_eq!(client.reverse(&new_address), Some(name));
    }

    #[test]
    fn updating_address_preserves_text_records() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let old_address = String::from_str(&env, "GABC");
        let new_address = String::from_str(&env, "GDEF");

        client.set_record(&name, &owner, &old_address, &100);
        client.set_text_record(
            &name,
            &owner,
            &String::from_str(&env, "com.twitter"),
            &String::from_str(&env, "@timmy"),
            &101,
        );

        client.set_record(&name, &owner, &new_address, &102);

        let record = client.resolve(&name).unwrap();
        assert_eq!(
            record
                .addresses
                .get(String::from_str(&env, "stellar")),
            Some(new_address)
        );
        assert_eq!(record.text_records.len(), 1);
        assert_eq!(
            record
                .text_records
                .get(String::from_str(&env, "com.twitter")),
            Some(String::from_str(&env, "@timmy"))
        );
        assert_eq!(record.updated_at, 102);
    }

    // Issue #315: Test text record value size limits
    #[test]
    fn enforces_text_record_value_size_limit() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let address = String::from_str(&env, "GABC");

        client.set_record(&name, &owner, &address, &100);

        // Valid value at limit
        let valid_value = String::from_str(&env, &"x".repeat(MAX_TEXT_RECORD_VALUE_LENGTH));
        client.set_text_record(
            &name,
            &owner,
            &String::from_str(&env, "key1"),
            &valid_value,
            &101,
        );

        let record = client.resolve(&name).unwrap();
        assert_eq!(record.text_records.len(), 1);

        // Value exceeding limit should fail
        let oversized_value = String::from_str(&env, &"x".repeat(MAX_TEXT_RECORD_VALUE_LENGTH + 1));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.set_text_record(
                &name,
                &owner,
                &String::from_str(&env, "key2"),
                &oversized_value,
                &102,
            );
        }));

        assert!(result.is_err(), "text record value exceeding limit should fail");
    }

    // Issue #317: Test multi-chain address records
    #[test]
    fn supports_multi_chain_address_records() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name = String::from_str(&env, "timmy.xlm");
        let stellar_address = String::from_str(&env, "GABC");
        let ethereum_address = String::from_str(&env, "0x1234567890123456789012345678901234567890");

        // Set Stellar address
        client.set_record(&name, &owner, &stellar_address, &100);

        // Set Ethereum address using set_address
        client.set_address(
            &name,
            &owner,
            &String::from_str(&env, "ethereum"),
            &ethereum_address,
            &101,
        );

        let record = client.resolve(&name).unwrap();
        assert_eq!(
            record
                .addresses
                .get(String::from_str(&env, "stellar")),
            Some(stellar_address)
        );
        assert_eq!(
            record
                .addresses
                .get(String::from_str(&env, "ethereum")),
            Some(ethereum_address)
        );

        // Test get_address helper
        assert_eq!(
            client.get_address(&name, &String::from_str(&env, "ethereum")),
            Some(ethereum_address)
        );
    }

    // Issue #321: Test batch resolver queries
    #[test]
    fn batch_resolve_returns_records_for_multiple_names() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name1 = String::from_str(&env, "alice.xlm");
        let name2 = String::from_str(&env, "bob.xlm");
        let name3 = String::from_str(&env, "charlie.xlm");
        let address1 = String::from_str(&env, "GAAA");
        let address2 = String::from_str(&env, "GBBB");

        client.set_record(&name1, &owner, &address1, &100);
        client.set_record(&name2, &owner, &address2, &101);

        // Batch resolve with one missing name
        let names = Vec::from_array(&env, [name1.clone(), name2.clone(), name3.clone()]);
        let results = client.batch_resolve(&names);

        assert_eq!(results.len(), 3);
        assert!(results.get(0).is_some()); // alice.xlm exists
        assert!(results.get(1).is_some()); // bob.xlm exists
        assert_eq!(results.get(2), None); // charlie.xlm doesn't exist
    }

    // Issue #321: Test batch reverse queries
    #[test]
    fn batch_reverse_returns_names_for_multiple_addresses() {
        let env = Env::default();
        let contract_id = env.register(ResolverContract, ());
        let client = ResolverContractClient::new(&env, &contract_id);

        let owner = Address::generate(&env);
        let name1 = String::from_str(&env, "alice.xlm");
        let name2 = String::from_str(&env, "bob.xlm");
        let address1 = String::from_str(&env, "GAAA");
        let address2 = String::from_str(&env, "GBBB");
        let address3 = String::from_str(&env, "GCCC");

        client.set_record(&name1, &owner, &address1, &100);
        client.set_record(&name2, &owner, &address2, &101);

        // Batch reverse lookup with one missing address
        let addresses = Vec::from_array(&env, [address1.clone(), address2.clone(), address3.clone()]);
        let results = client.batch_reverse(&addresses);

        assert_eq!(results.len(), 3);
        assert_eq!(results.get(0), Some(Some(name1))); // GAAA -> alice.xlm
        assert_eq!(results.get(1), Some(Some(name2))); // GBBB -> bob.xlm
        assert_eq!(results.get(2), Some(None)); // GCCC -> None
    }
}
