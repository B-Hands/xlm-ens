#[cfg(test)]
mod tests {
    use soroban_sdk::{testutils::{Address as _, Events}, Address, Env, String};

    use crate::{BridgeContract, BridgeContractClient, BridgeError};

    fn setup_client(env: &Env) -> BridgeContractClient<'_> {
        let contract_id = env.register(BridgeContract, ());
        BridgeContractClient::new(env, &contract_id)
    }

    fn setup_initialized_client(env: &Env) -> (BridgeContractClient<'_>, Address) {
        env.mock_all_auths();
        let client = setup_client(env);
        let admin = Address::generate(env);
        client.initialize(&admin);
        (client, admin)
    }

    fn add_default_chain(
        client: &BridgeContractClient<'_>,
        env: &Env,
        chain: &str,
        resolver: &str,
    ) {
        client.add_supported_chain(
            &String::from_str(env, chain),
            &String::from_str(env, resolver),
        );
    }

    fn register_default_chains(client: &BridgeContractClient<'_>, env: &Env) {
        add_default_chain(client, env, "base", "0xbaseResolver");
        add_default_chain(client, env, "ethereum", "0xethResolver");
        add_default_chain(client, env, "arbitrum", "0xarbResolver");
    }

    #[test]
    fn stores_bridge_routes_in_contract_storage() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);
        let contract_id = client.address.clone();

        let base = String::from_str(&env, "base");
        let name = String::from_str(&env, "timmy.xlm");

        add_default_chain(&client, &env, "base", "0xbaseResolver");
        client.register_chain(&base);
        let route = client.route(&base).unwrap();
        let payload = client.build_message(&name, &base);

        assert_eq!(
            route.destination_resolver,
            String::from_str(&env, "0xbaseResolver")
        );
        assert!(payload.to_string().contains("timmy.xlm"));
        env.as_contract(&contract_id, || {
            assert!(env.storage().persistent().has(&crate::DataKey::Route(base)));
        });
    }

    #[test]
    fn build_message_exact_payload_base_chain() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "base");
        let name = String::from_str(&env, "alice.xlm");

        add_default_chain(&client, &env, "base", "0xbaseResolver");
        client.register_chain(&chain);
        let payload = client.build_message(&name, &chain);

        let expected = String::from_str(
            &env,
            r#"{"type":"xlm-ns-resolution","name":"alice.xlm","destination_chain":"base","resolver":"0xbaseResolver"}"#,
        );
        assert_eq!(payload, expected);
    }

    #[test]
    fn build_message_exact_payload_ethereum_chain() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "ethereum");
        let name = String::from_str(&env, "bob.xlm");

        add_default_chain(&client, &env, "ethereum", "0xethResolver");
        client.register_chain(&chain);
        let payload = client.build_message(&name, &chain);

        let expected = String::from_str(
            &env,
            r#"{"type":"xlm-ns-resolution","name":"bob.xlm","destination_chain":"ethereum","resolver":"0xethResolver"}"#,
        );
        assert_eq!(payload, expected);
    }

    #[test]
    fn build_message_exact_payload_arbitrum_chain() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "arbitrum");
        let name = String::from_str(&env, "charlie.xlm");

        add_default_chain(&client, &env, "arbitrum", "0xarbResolver");
        client.register_chain(&chain);
        let payload = client.build_message(&name, &chain);

        let expected = String::from_str(
            &env,
            r#"{"type":"xlm-ns-resolution","name":"charlie.xlm","destination_chain":"arbitrum","resolver":"0xarbResolver"}"#,
        );
        assert_eq!(payload, expected);
    }

    #[test]
    fn build_message_payload_with_long_name() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "base");
        let name = String::from_str(&env, "very-long-domain-name.xlm");

        add_default_chain(&client, &env, "base", "0xbaseResolver");
        client.register_chain(&chain);
        let payload = client.build_message(&name, &chain);

        let expected = String::from_str(
            &env,
            r#"{"type":"xlm-ns-resolution","name":"very-long-domain-name.xlm","destination_chain":"base","resolver":"0xbaseResolver"}"#,
        );
        assert_eq!(payload, expected);
    }

    #[test]
    fn build_message_payload_special_characters() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "ethereum");
        let name = String::from_str(&env, "test-name.xlm");

        add_default_chain(&client, &env, "ethereum", "0xethResolver");
        client.register_chain(&chain);
        let payload = client.build_message(&name, &chain);

        let expected = String::from_str(
            &env,
            r#"{"type":"xlm-ns-resolution","name":"test-name.xlm","destination_chain":"ethereum","resolver":"0xethResolver"}"#,
        );
        assert_eq!(payload, expected);
    }

    #[test]
    fn build_message_payload_version_locked() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "base");
        let name = String::from_str(&env, "version.xlm");

        add_default_chain(&client, &env, "base", "0xbaseResolver");
        client.register_chain(&chain);
        let payload = client.build_message(&name, &chain);

        assert!(payload
            .to_string()
            .starts_with(r#"{"type":"xlm-ns-resolution""#));

        let expected = String::from_str(
            &env,
            r#"{"type":"xlm-ns-resolution","name":"version.xlm","destination_chain":"base","resolver":"0xbaseResolver"}"#,
        );
        assert_eq!(payload, expected);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #2)")]
    fn build_message_fails_for_unregistered_chain() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "polygon");
        let name = String::from_str(&env, "test.xlm");

        client.build_message(&name, &chain);
    }

    #[test]
    #[should_panic(expected = "Error(Contract, #1)")]
    fn build_message_fails_for_invalid_name() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "base");
        let invalid_name = String::from_str(&env, "invalid");

        add_default_chain(&client, &env, "base", "0xbaseResolver");
        client.register_chain(&chain);
        client.build_message(&invalid_name, &chain);
    }

    #[test]
    fn register_chain_rejects_unsupported_destination() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "base");
        let result = client.try_register_chain(&chain);
        assert!(matches!(result, Err(Ok(BridgeError::UnsupportedChain))));
    }

    #[test]
    fn version_is_exposed() {
        let env = Env::default();
        let client = setup_client(&env);
        assert_eq!(client.version(), 1);
    }

    // ==================== Supported Chains Registry Tests ====================

    #[test]
    fn add_supported_chain_registers_chain_with_resolver() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "base");
        let resolver = String::from_str(&env, "0xbaseResolver");
        client.add_supported_chain(&chain_id, &resolver);

        let supported = client.supported_chain(&chain_id).unwrap();
        assert_eq!(supported.chain_id, chain_id);
        assert_eq!(supported.resolver_address, resolver);
    }

    #[test]
    fn get_supported_chains_returns_registered_chains() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        register_default_chains(&client, &env);
        let chains = client.get_supported_chains();
        assert_eq!(chains.len(), 3);
    }

    #[test]
    fn add_supported_chain_rejects_duplicate() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "ethereum");
        let resolver = String::from_str(&env, "0xethResolver");
        client.add_supported_chain(&chain_id, &resolver);

        let result = client.try_add_supported_chain(&chain_id, &resolver);
        assert!(matches!(result, Err(Ok(BridgeError::AlreadyExists))));
    }

    #[test]
    fn remove_supported_chain_deregisters_chain() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "base");
        let resolver = String::from_str(&env, "0xbaseResolver");
        client.add_supported_chain(&chain_id, &resolver);
        client.remove_supported_chain(&chain_id);

        assert!(client.supported_chain(&chain_id).is_none());
        assert_eq!(client.get_supported_chains().len(), 0);
    }

    #[test]
    fn remove_supported_chain_rejects_unknown_chain() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "polygon");
        let result = client.try_remove_supported_chain(&chain_id);
        assert!(matches!(result, Err(Ok(BridgeError::NotFound))));
    }

    #[test]
    fn remove_supported_chain_clears_cached_route() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "ethereum");
        add_default_chain(&client, &env, "ethereum", "0xethResolver");
        client.register_chain(&chain_id);
        assert!(client.route(&chain_id).is_some());

        client.remove_supported_chain(&chain_id);
        assert!(client.route(&chain_id).is_none());
    }

    #[test]
    fn register_chain_succeeds_for_supported_chain() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain = String::from_str(&env, "arbitrum");
        add_default_chain(&client, &env, "arbitrum", "0xarbResolver");
        client.register_chain(&chain);

        let route = client.route(&chain).unwrap();
        assert_eq!(
            route.destination_resolver,
            String::from_str(&env, "0xarbResolver")
        );
    }

    #[test]
    fn add_supported_chain_emits_event() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "base");
        let resolver = String::from_str(&env, "0xbaseResolver");
        client.add_supported_chain(&chain_id, &resolver);

        assert!(!env.events().all().events().is_empty());
    }

    #[test]
    fn remove_supported_chain_emits_event() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "base");
        let resolver = String::from_str(&env, "0xbaseResolver");
        client.add_supported_chain(&chain_id, &resolver);
        client.remove_supported_chain(&chain_id);

        assert!(!env.events().all().events().is_empty());
    }

    #[test]
    fn add_supported_chain_rejects_empty_resolver() {
        let env = Env::default();
        let (client, _) = setup_initialized_client(&env);

        let chain_id = String::from_str(&env, "base");
        let resolver = String::from_str(&env, "");
        let result = client.try_add_supported_chain(&chain_id, &resolver);
        assert!(matches!(result, Err(Ok(BridgeError::Validation))));
    }
}
