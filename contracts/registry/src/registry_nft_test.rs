/// Integration tests: Registry and NFT contract synchronization.
///
/// These tests verify that NFT state remains synchronized after registry
/// operations like transfer, burn, and renewal.
#[cfg(test)]
mod registry_nft_integration {
    use soroban_sdk::{
        contract, contracterror, contractimpl, contracttype, testutils::Address as _, Address, Env,
        String,
    };
    use xlm_ns_registrar::{RegistrarContract, RegistrarContractClient};
    use xlm_ns_registry::{RegistryContract, RegistryContractClient};

    // A mock NFT contract to test synchronization from the Registry.
    #[contract]
    pub struct MockNftContract;

    #[contracttype]
    #[derive(Clone)]
    enum DataKey {
        Owner(String),
        Expiry(String),
        Registry,
    }

    #[contracterror]
    #[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
    #[repr(u32)]
    pub enum NftError {
        NotAuthorized = 1,
        NotFound = 2,
    }

    #[contractimpl]
    impl MockNftContract {
        pub fn initialize(env: Env, registry: Address) {
            env.storage().instance().set(&DataKey::Registry, &registry);
        }

        // Mint an NFT for a name. Only the name's owner in the registry can do this.
        pub fn mint(env: Env, name: String, owner: Address) -> Result<(), NftError> {
            owner.require_auth();
            let registry_id = env.storage().instance().get(&DataKey::Registry).unwrap();
            let registry_client = RegistryContractClient::new(&env, &registry_id);

            // Verify caller owns the name in the registry.
            let registry_entry = registry_client.resolve(&name, &env.ledger().timestamp());
            if registry_entry.owner != owner {
                return Err(NftError::NotAuthorized);
            }

            env.storage().persistent().set(&DataKey::Owner(name), &owner);
            Ok(())
        }

        // Synchronize owner from a trusted Registry contract call.
        pub fn sync_owner(env: Env, name: String, new_owner: Address) {
            let registry_id: Address = env.storage().instance().get(&DataKey::Registry).unwrap();
            registry_id.require_auth(); // Only registry can call this

            env.storage().persistent().set(&DataKey::Owner(name), &new_owner);
        }

        // Synchronize expiry from a trusted Registry contract call.
        pub fn sync_expiry(env: Env, name: String, new_expiry: u64) {
            let registry_id: Address = env.storage().instance().get(&DataKey::Registry).unwrap();
            registry_id.require_auth(); // Only registry can call this

            env.storage().persistent().set(&DataKey::Expiry(name), &new_expiry);
        }

        // Burn an NFT from a trusted Registry contract call.
        pub fn burn(env: Env, name: String) {
            let registry_id: Address = env.storage().instance().get(&DataKey::Registry).unwrap();
            registry_id.require_auth(); // Only registry can call this

            env.storage().persistent().remove(&DataKey::Owner(name.clone()));
            env.storage().persistent().remove(&DataKey::Expiry(name));
        }

        // Public view function to get the owner of an NFT.
        pub fn owner_of(env: Env, name: String) -> Result<Address, NftError> {
            env.storage()
                .persistent()
                .get(&DataKey::Owner(name))
                .ok_or(NftError::NotFound)
        }

        // Public view function to get the expiry of an NFT.
        pub fn expiry_of(env: Env, name: String) -> Result<u64, NftError> {
            env.storage()
                .persistent()
                .get(&DataKey::Expiry(name))
                .ok_or(NftError::NotFound)
        }
    }

    struct TestSetup {
        env: Env,
        registrar: RegistrarContractClient<'static>,
        registry: RegistryContractClient<'static>,
        nft: MockNftContractClient<'static>,
        admin: Address,
        owner1: Address,
        owner2: Address,
    }

    fn setup_env() -> TestSetup {
        let env = Env::default();
        env.mock_all_auths();

        let admin = Address::generate(&env);
        let owner1 = Address::generate(&env);
        let owner2 = Address.generate(&env);

        let registry_id = env.register_contract(None, RegistryContract);
        let registrar_id = env.register_contract(None, RegistrarContract);
        let nft_id = env.register_contract(None, MockNftContract);

        let registrar = RegistrarContractClient::new(&env, &registrar_id);
        let registry = RegistryContractClient::new(&env, &registry_id);
        let nft = MockNftContractClient::new(&env, &nft_id);

        // Initialization
        registry.initialize(&admin);
        registrar.initialize(&registry_id);
        nft.initialize(&registry_id);

        // Wire registry to NFT contract
        registry.set_nft_contract(&nft_id);

        TestSetup {
            env,
            registrar,
            registry,
            nft,
            admin,
            owner1,
            owner2,
        }
    }

    fn register_name(setup: &TestSetup, label: &str, owner: &Address) -> String {
        let label_str = String::from_str(&setup.env, label);
        let name = String::from_str(&setup.env, &format!("{}.xlm", label));
        let now = setup.env.ledger().timestamp();

        let quote = setup.registrar.quote_registration(&label_str, &1, &now);
        setup
            .registrar
            .register(&label_str, owner, &1, &quote.fee_stroops, &now);
        name
    }

    #[test]
    fn minting_requires_registry_ownership() {
        let setup = setup_env();
        let name = register_name(&setup, "alice", &setup.owner1);

        // owner2 tries to mint, should fail
        let res = setup.nft.try_mint(&name, &setup.owner2);
        assert!(res.is_err());

        // owner1 mints, should succeed
        setup.nft.mint(&name, &setup.owner1);
        assert_eq!(setup.nft.owner_of(&name), setup.owner1);
    }

    #[test]
    fn transfer_updates_nft_owner() {
        let setup = setup_env();
        let name = register_name(&setup, "bob", &setup.owner1);
        setup.nft.mint(&name, &setup.owner1);

        assert_eq!(setup.nft.owner_of(&name), setup.owner1);

        // Transfer name in registry
        let now = setup.env.ledger().timestamp();
        setup
            .registry
            .transfer(&name, &setup.owner1, &setup.owner2, &now);

        // Verify NFT owner was synchronized
        assert_eq!(setup.nft.owner_of(&name), setup.owner2);
    }

    #[test]
    fn burn_invalidates_nft() {
        let setup = setup_env();
        let name = register_name(&setup, "carol", &setup.owner1);
        setup.nft.mint(&name, &setup.owner1);

        assert_eq!(setup.nft.owner_of(&name), setup.owner1);

        // Burn name in registry
        let now = setup.env.ledger().timestamp();
        setup.registry.burn(&name, &setup.owner1, &now);

        // Verify NFT was burned
        let res = setup.nft.try_owner_of(&name);
        assert!(res.is_err());
    }

    #[test]
    fn renewal_updates_nft_metadata() {
        let setup = setup_env();
        let name = register_name(&setup, "dave", &setup.owner1);
        setup.nft.mint(&name, &setup.owner1);

        let initial_reg_entry = setup.registry.resolve(&name, &setup.env.ledger().timestamp());
        setup
            .nft
            .sync_expiry(&name, &initial_reg_entry.expires_at);

        assert_eq!(
            setup.nft.expiry_of(&name),
            initial_reg_entry.expires_at
        );

        // Advance time and renew
        setup.env.ledger().with_mut(|l| {
            l.timestamp += 1000;
        });
        let now = setup.env.ledger().timestamp();
        let quote = setup.registrar.quote_registration(
            &String::from_str(&setup.env, "dave"),
            &1,
            &now,
        );
        setup
            .registrar
            .renew(&name, &setup.owner1, &1, &quote.fee_stroops, &now);

        // Verify NFT expiry was synchronized
        let renewed_reg_entry = setup.registry.resolve(&name, &now);
        assert_ne!(
            initial_reg_entry.expires_at,
            renewed_reg_entry.expires_at
        );
        assert_eq!(setup.nft.expiry_of(&name), renewed_reg_entry.expires_at);
    }

    #[test]
    fn expired_name_nft_can_be_transferred_but_registry_is_truth() {
        let setup = setup_env();
        let name = register_name(&setup, "eve", &setup.owner1);
        setup.nft.mint(&name, &setup.owner1);

        let initial_reg_entry = setup.registry.resolve(&name, &setup.env.ledger().timestamp());

        // Advance time past expiry
        setup.env.ledger().with_mut(|l| {
            l.timestamp = initial_reg_entry.expires_at + 1;
        });

        // Name is expired in registry
        assert!(setup
            .registry
            .try_resolve(&name, &setup.env.ledger().timestamp())
            .is_err());

        // But NFT ownership is unchanged until registry state changes
        assert_eq!(setup.nft.owner_of(&name), setup.owner1);
    }
}