
// c:\Users\USER\Downloads\xlm-ens	ests\integrationegistry_nft_test.rs

#[cfg(test)]
mod registry_nft_integration {
    use soroban_sdk::{testutils::Address as _, Address, Env, String};
    use xlm_ns_registrar::{RegistrarContract, RegistrarContractClient};
    use xlm_ns_registry::{RegistryContract, RegistryContractClient};
    use xlm_ns_nft::{NftContract, NftContractClient};

    struct TimeHelper {
        pub now: u64,
    }

    impl TimeHelper {
        pub fn new(start: u64) -> Self {
            Self { now: start }
        }
        pub fn advance(&mut self, seconds: u64) {
            self.now += seconds;
        }
        pub fn future(&self, seconds: u64) -> u64 {
            self.now + seconds
        }
    }

    fn setup_env() -> (
        Env,
        RegistrarContractClient<'static>,
        RegistryContractClient<'static>,
        NftContractClient<'static>,
        Address,
    ) {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);

        let registry_id = env.register_contract(None, RegistryContract);
        let registrar_id = env.register_contract(None, RegistrarContract);
        let nft_id = env.register_contract(None, NftContract);

        let registrar = RegistrarContractClient::new(&env, &registrar_id);
        let registry = RegistryContractClient::new(&env, &registry_id);
        let nft = NftContractClient::new(&env, &nft_id);
        
        // Initialize contracts
        registry.initialize(&admin);
        registrar.initialize(&registry_id);
        nft.initialize(&admin);

        // Wire the registry to the NFT contract
        registry.set_nft_contract(&nft_id);

        (env, registrar, registry, nft, admin)
    }

    #[test]
    fn register_mint_and_transfer_flow() {
        let (env, registrar, registry, nft, _admin) = setup_env();
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let label = String::from_str(&env, "test");
        let name = String::from_str(&env, "test.xlm");
        let mut time = TimeHelper::new(1_000_000);

        // Register a name
        let quote = registrar.quote_registration(&label, &1, &time.now);
        registrar.register(&label, &owner, &1, &quote.fee_stroops, &time.now);

        // Verify NFT was minted to the correct owner
        let nft_owner = nft.owner_of(&name).expect("NFT not minted");
        assert_eq!(nft_owner, owner);

        // Transfer the name
        registry.transfer(&name, &owner, &new_owner, &time.now);

        // Verify NFT ownership was transferred
        let new_nft_owner = nft.owner_of(&name).expect("NFT ownership not transferred");
        assert_eq!(new_nft_owner, new_owner);
    }

    #[test]
    fn burn_flow() {
        let (env, registrar, registry, nft, _admin) = setup_env();
        let owner = Address::generate(&env);
        let label = String::from_str(&env, "test");
        let name = String::from_str(&env, "test.xlm");
        let mut time = TimeHelper::new(1_000_000);

        // Register a name
        let quote = registrar.quote_registration(&label, &1, &time.now);
        registrar.register(&label, &owner, &1, &quote.fee_stroops, &time.now);

        // Verify NFT was minted
        assert!(nft.owner_of(&name).is_some());

        // Burn the name
        registry.burn(&name, &owner, &time.now);

        // Verify NFT was burned
        assert!(nft.owner_of(&name).is_none());
    }

    #[test]
    fn renewal_flow() {
        let (env, registrar, registry, nft, _admin) = setup_env();
        let owner = Address::generate(&env);
        let label = String::from_str(&env, "test");
        let name = String::from_str(&env, "test.xlm");
        let mut time = TimeHelper::new(1_000_000);

        // Register a name
        let quote = registrar.quote_registration(&label, &1, &time.now);
        registrar.register(&label, &owner, &1, &quote.fee_stroops, &time.now);

        // Verify initial expiry
        let token = nft.token(&name).expect("NFT not found");
        assert_eq!(token.expires_at, quote.expiry_unix);

        // Renew the name
        time.advance(1000);
        let renewal_quote = registrar.quote_registration(&label, &1, &time.now);
        registrar.renew(&name, &owner, &1, &renewal_quote.fee_stroops, &time.now);

        // Verify expiry was updated
        let updated_token = nft.token(&name).expect("NFT not found");
        assert!(updated_token.expires_at > token.expires_at);
    }

    #[test]
    fn expired_name_cannot_be_transferred() {
        let (env, registrar, registry, nft, _admin) = setup_env();
        let owner = Address::generate(&env);
        let new_owner = Address::generate(&env);
        let label = String::from_str(&env, "test");
        let name = String::from_str(&env, "test.xlm");
        let mut time = TimeHelper::new(1_000_000);

        // Register a name
        let quote = registrar.quote_registration(&label, &1, &time.now);
        registrar.register(&label, &owner, &1, &quote.fee_stroops, &time.now);

        // Advance time past expiry
        time.advance(quote.expiry_unix - time.now + 1);

        // Try to transfer the name
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.transfer(&name, &owner, &new_owner, &time.now);
        }));

        assert!(result.is_err());

        // Verify NFT ownership was not transferred
        let nft_owner = nft.owner_of(&name).expect("NFT not minted");
        assert_eq!(nft_owner, owner);
    }
}
