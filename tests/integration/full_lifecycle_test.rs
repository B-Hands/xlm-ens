/// Integration test: the full name lifecycle from registration through
/// expiry, grace period, claimable state, and re-registration by a new owner.
///
/// This walks all five contracts (registry, registrar, resolver, subdomain,
/// nft) through a single continuous timeline so that cross-contract
/// regressions in lifecycle transitions (stale resolver data, orphaned NFTs,
/// subdomains outliving a reclaimed parent, ...) surface in one place instead
/// of being scattered across per-contract suites.
#[cfg(test)]
mod full_lifecycle_integration {
    use soroban_sdk::{
        testutils::{Address as _, Ledger},
        Address, Env, String,
    };

    use xlm_ns_nft::{NftContract, NftContractClient};
    use xlm_ns_registrar::{RegistrarContract, RegistrarContractClient, RegistrationStatus};
    use xlm_ns_registry::{NameState, RegistryContract, RegistryContractClient};
    use xlm_ns_resolver::{ResolverContract, ResolverContractClient};
    use xlm_ns_subdomain::{SubdomainContract, SubdomainContractClient};

    struct Lifecycle<'a> {
        env: Env,
        registrar: RegistrarContractClient<'a>,
        registry: RegistryContractClient<'a>,
        resolver: ResolverContractClient<'a>,
        subdomain: SubdomainContractClient<'a>,
        nft: NftContractClient<'a>,
    }

    fn setup() -> Lifecycle<'static> {
        let env = Env::default();
        // Registration exercises nested cross-contract auth (registrar ->
        // registry -> nft), which requires non-root auth mocking; the same
        // pattern is used in registry_nft_test.rs.
        env.mock_all_auths_allowing_non_root_auth();

        let admin = Address::generate(&env);

        let registry_id = env.register(RegistryContract, ());
        let registrar_id = env.register(RegistrarContract, ());
        let resolver_id = env.register(ResolverContract, ());
        let subdomain_id = env.register(SubdomainContract, ());
        let nft_id = env.register(NftContract, ());

        let registrar = RegistrarContractClient::new(&env, &registrar_id);
        let registry = RegistryContractClient::new(&env, &registry_id);
        let resolver = ResolverContractClient::new(&env, &resolver_id);
        let subdomain = SubdomainContractClient::new(&env, &subdomain_id);
        let nft = NftContractClient::new(&env, &nft_id);

        registry.initialize(&admin);
        registrar.initialize(&registry_id, &admin);
        nft.initialize(&admin);
        resolver.initialize(&registry_id, &admin);
        subdomain.initialize(&admin);

        registry.set_nft_contract(&nft_id);
        subdomain.set_registry_contract(&registry_id);
        resolver.set_subdomain_contract(&subdomain_id);

        Lifecycle {
            env,
            registrar,
            registry,
            resolver,
            subdomain,
            nft,
        }
    }

    /// Walks a single name through every lifecycle phase described in issue
    /// #613: registration, resolver + subdomain setup, expiry into the grace
    /// period, owner renewal during grace, expiry past grace into the
    /// claimable state, re-registration by a new owner, and finally fresh
    /// resolver/subdomain setup by the new owner.
    #[test]
    fn full_name_lifecycle_registration_through_reregistration() {
        let lc = setup();
        let (env, registrar, registry, resolver, subdomain, nft) = (
            &lc.env,
            &lc.registrar,
            &lc.registry,
            &lc.resolver,
            &lc.subdomain,
            &lc.nft,
        );

        let owner_a = Address::generate(env);
        let owner_b = Address::generate(env);
        let subdomain_owner = Address::generate(env);

        let label = String::from_str(env, "test");
        let name = String::from_str(env, "test.xlm");
        let sub_label = String::from_str(env, "pay");
        let sub_fqdn = String::from_str(env, "pay.test.xlm");

        let start = 1_000_000u64;
        env.ledger().set_timestamp(start);

        // ---------------------------------------------------------------
        // Phase 1: Owner A registers, sets resolver records, creates a
        // subdomain.
        // ---------------------------------------------------------------
        let quote = registrar.quote_registration(&label, &1, &start);
        registrar.register(&label, &owner_a, &1, &quote.fee_stroops, &start);

        let entry = registry.resolve(&name, &start);
        assert_eq!(entry.owner, owner_a);
        assert_eq!(entry.expires_at, quote.expiry_unix);

        let nft_owner = nft
            .owner_of(&name)
            .expect("NFT should be minted on registration");
        assert_eq!(nft_owner, owner_a);

        let owner_a_address = owner_a.to_string();
        resolver.set_record(&name, &owner_a, &owner_a_address, &start);
        resolver.set_primary_name(&owner_a_address, &owner_a, &name);
        assert_eq!(resolver.resolve(&name).unwrap().owner, owner_a);
        assert_eq!(resolver.reverse(&owner_a_address), Some(name.clone()));

        subdomain.register_parent(&name, &owner_a);
        let created_fqdn = subdomain.create(&sub_label, &name, &owner_a, &subdomain_owner, &start);
        assert_eq!(created_fqdn, sub_fqdn);
        assert!(subdomain.exists(&sub_fqdn));
        assert_eq!(subdomain.subdomains_for_parent(&name).len(), 1);

        // ---------------------------------------------------------------
        // Phase 2: advance past expiry into the grace period.
        // ---------------------------------------------------------------
        let expiry = quote.expiry_unix;
        let in_grace = expiry + 1;
        env.ledger().set_timestamp(in_grace);

        assert_eq!(
            registry.name_state(&name, &in_grace),
            NameState::GracePeriod
        );

        // The registry refuses to resolve a name that is no longer active.
        let resolve_during_grace = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registry.resolve(&name, &in_grace);
        }));
        assert!(
            resolve_during_grace.is_err(),
            "registry resolve must fail once the name leaves the active state"
        );

        // The parent leaving the active state immediately purges its
        // subdomains (this is the fix for issue #458).
        assert!(subdomain.record(&sub_fqdn).is_none());
        assert!(!subdomain.exists(&sub_fqdn));
        assert!(subdomain.parent(&name).is_none());
        assert!(subdomain.subdomains_for_parent(&name).is_empty());
        assert!(subdomain.subdomains_for_owner(&subdomain_owner).is_empty());

        // Owner A — and only Owner A — can still renew during grace.
        let intruder_renewal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registrar.renew(&name, &owner_b, &1, &quote.fee_stroops, &in_grace);
        }));
        assert!(
            intruder_renewal.is_err(),
            "a non-owner must not be able to renew during grace"
        );

        let grace_renewal_quote = registrar.quote_renewal(&name, &1, &in_grace);
        registrar.extend_during_grace(
            &name,
            &owner_a,
            &1,
            &grace_renewal_quote.fee_stroops,
            &in_grace,
        );

        let renewed_record = registrar.registration(&name).unwrap();
        assert!(renewed_record.expires_at > expiry);
        assert_eq!(registry.name_state(&name, &in_grace), NameState::Active);
        assert_eq!(
            registry.resolve(&name, &in_grace).expires_at,
            renewed_record.expires_at
        );

        // ---------------------------------------------------------------
        // Phase 3: advance past the (renewed) grace period into the
        // claimable state.
        // ---------------------------------------------------------------
        let past_grace = renewed_record.grace_period_ends_at + 1;
        env.ledger().set_timestamp(past_grace);

        assert_eq!(
            registry.name_state(&name, &past_grace),
            NameState::Claimable
        );
        assert_eq!(
            registrar.registration_status(&label, &past_grace),
            RegistrationStatus::Claimable
        );
        assert!(registrar.is_available(&label, &past_grace));

        // Subdomains remain purged and cannot be recreated against the
        // claimable parent.
        assert!(subdomain.parent(&name).is_none());
        assert!(subdomain.record(&sub_fqdn).is_none());

        // Even the original owner can no longer renew once claimable.
        let stale_owner_renewal = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            registrar.extend_during_grace(
                &name,
                &owner_a,
                &1,
                &grace_renewal_quote.fee_stroops,
                &past_grace,
            );
        }));
        assert!(
            stale_owner_renewal.is_err(),
            "renewal must fail once the name is claimable"
        );

        // ---------------------------------------------------------------
        // Phase 4: Owner B re-registers the claimable name.
        // ---------------------------------------------------------------
        let reclaim_quote = registrar.quote_registration(&label, &1, &past_grace);
        registrar.register(
            &label,
            &owner_b,
            &1,
            &reclaim_quote.fee_stroops,
            &past_grace,
        );

        let new_entry = registry.resolve(&name, &past_grace);
        assert_eq!(new_entry.owner, owner_b);
        assert_eq!(new_entry.expires_at, reclaim_quote.expiry_unix);
        assert!(registry.names_for_owner(&owner_a).is_empty());
        assert_eq!(registry.names_for_owner(&owner_b).len(), 1);

        // The stale NFT was burned and a fresh one minted to Owner B.
        let new_nft_owner = nft
            .owner_of(&name)
            .expect("a new NFT should be minted for owner B");
        assert_eq!(new_nft_owner, owner_b);
        let token = nft.token(&name).unwrap();
        assert_eq!(token.expires_at, reclaim_quote.expiry_unix);

        // Owner A has lost resolver authority over the reclaimed name.
        let stale_owner_write = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            resolver.set_record(&name, &owner_a, &owner_a_address, &past_grace);
        }));
        assert!(
            stale_owner_write.is_err(),
            "previous owner must lose resolver authority after reclaim"
        );

        // ---------------------------------------------------------------
        // Phase 5: Owner B sets fresh resolver records and recreates a
        // subdomain under the reclaimed parent.
        // ---------------------------------------------------------------
        let owner_b_address = owner_b.to_string();
        resolver.set_record(&name, &owner_b, &owner_b_address, &past_grace);
        resolver.set_primary_name(&owner_b_address, &owner_b, &name);

        let resolved = resolver.resolve(&name).unwrap();
        assert_eq!(resolved.owner, owner_b);
        assert_eq!(
            resolver.get_stellar_address(&name),
            Some(owner_b_address.clone())
        );
        assert_eq!(resolver.reverse(&owner_b_address), Some(name.clone()));

        // Owner A's old reverse mapping no longer resolves to the reclaimed
        // name: setting a new address for the name clears the previous
        // reverse/primary entries as part of `set_record`.
        assert_eq!(resolver.reverse(&owner_a_address), None);

        subdomain.register_parent(&name, &owner_b);
        let recreated_fqdn =
            subdomain.create(&sub_label, &name, &owner_b, &subdomain_owner, &past_grace);
        assert_eq!(recreated_fqdn, sub_fqdn);
        assert!(subdomain.exists(&sub_fqdn));
        assert_eq!(subdomain.record(&sub_fqdn).unwrap().owner, subdomain_owner);
        assert_eq!(subdomain.subdomains_for_parent(&name).len(), 1);
    }
}
