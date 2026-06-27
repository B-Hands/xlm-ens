#[cfg(test)] ///////
mod tests {
    extern crate std;

    use std::format;

    use soroban_sdk::{
        contract, contractimpl, contracttype,
        testutils::Address as _, Address, Env, String,
    };
    use soroban_sdk::token;

    use crate::{AuctionContract, AuctionContractClient};

    #[contract]
    pub struct ReentrantToken;

    #[derive(Clone)]
    #[contracttype]
    enum ReentrantKey {
        Balance(Address),
        Auction,
        Bidder,
        Name,
        Triggered,
    }

    #[contractimpl]
    impl ReentrantToken {
        pub fn initialize(env: Env, auction: Address, bidder: Address, name: String) {
            env.storage().instance().set(&ReentrantKey::Auction, &auction);
            env.storage().instance().set(&ReentrantKey::Bidder, &bidder);
            env.storage().instance().set(&ReentrantKey::Name, &name);
        }

        pub fn mint(env: Env, to: Address, amount: i128) {
            let key = ReentrantKey::Balance(to);
            let balance = env
                .storage()
                .persistent()
                .get::<_, i128>(&key)
                .unwrap_or(0);
            env.storage()
                .persistent()
                .set(&key, &balance.saturating_add(amount));
        }

        pub fn balance(env: Env, owner: Address) -> i128 {
            env.storage()
                .persistent()
                .get(&ReentrantKey::Balance(owner))
                .unwrap_or(0)
        }

        pub fn transfer(
            env: Env,
            from: Address,
            to: Address,
            amount: i128,
        ) -> Result<(), ()> {
            if !env
                .storage()
                .instance()
                .get::<_, bool>(&ReentrantKey::Triggered)
                .unwrap_or(false)
            {
                env.storage().instance().set(&ReentrantKey::Triggered, &true);
                let auction: Address = env.storage().instance().get(&ReentrantKey::Auction).unwrap();
                let bidder: Address = env.storage().instance().get(&ReentrantKey::Bidder).unwrap();
                let name: String = env.storage().instance().get(&ReentrantKey::Name).unwrap();
                let client = AuctionContractClient::new(&env, &auction);
                let _ = client.try_place_bid(&name, &bidder, &1u64, &env.ledger().timestamp());
            }

            let from_key = ReentrantKey::Balance(from);
            let to_key = ReentrantKey::Balance(to);
            let from_balance = env
                .storage()
                .persistent()
                .get::<_, i128>(&from_key)
                .unwrap_or(0);
            let to_balance = env
                .storage()
                .persistent()
                .get::<_, i128>(&to_key)
                .unwrap_or(0);

            env.storage()
                .persistent()
                .set(&from_key, &from_balance.saturating_sub(amount));
            env.storage()
                .persistent()
                .set(&to_key, &to_balance.saturating_add(amount));
            Ok(())
        }
    }

    fn setup_token(
        env: &Env,
    ) -> (
        Address,
        token::StellarAssetClient<'static>,
        token::Client<'static>,
    ) {
        let admin = Address::generate(env);
        let contract = env.register_stellar_asset_contract(admin.clone());
        (
            contract.clone(),
            token::StellarAssetClient::new(env, &contract),
            token::Client::new(env, &contract),
        )
    }

    fn setup_reentrant_token(
        env: &Env,
        auction: &Address,
        bidder: &Address,
        name: &String,
    ) -> (Address, ReentrantTokenClient<'static>) {
        let contract_id = env.register_contract(None, ReentrantToken);
        let client = ReentrantTokenClient::new(env, &contract_id);
        client.initialize(auction, bidder, name);
        (contract_id, client)
    }

    #[test]
    fn stores_auctions_in_contract_storage() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let (asset, token_admin, token) = setup_token(&env);
        let treasury = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        token_admin.mint(&alice, &1000);
        token_admin.mint(&bob, &1000);

        let name = String::from_str(&env, "vip.xlm");

        client.create_auction(&name, &asset, &treasury, &200, &10, &20);
        client.place_bid(&name, &alice, &500, &12);
        client.place_bid(&name, &bob, &300, &13);

        let settlement = client.settle(&name, &21).unwrap();
        assert_eq!(settlement.winner, Some(alice.clone()));
        assert_eq!(settlement.clearing_price, 300);
        assert!(settlement.sold);

        assert_eq!(token.balance(&alice), 1000 - 300);
        assert_eq!(token.balance(&bob), 1000);
        assert_eq!(token.balance(&treasury), 300);
    } //

    #[test]
    fn reentrant_token_transfer_cannot_reenter_place_bid() {
        let env = Env::default();
        env.mock_all_auths();

        let auction_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &auction_id);

        let alice = Address::generate(&env);
        let treasury = Address::generate(&env);
        let name = String::from_str(&env, "guarded.xlm");

        let (asset, token) = setup_reentrant_token(&env, &auction_id, &alice, &name);
        token.mint(&alice, &1000);

        client.create_auction(&name, &asset, &treasury, &100, &10, &20);
        client.place_bid(&name, &alice, &500, &12);

        let auction = client.auction(&name).unwrap();
        assert_eq!(auction.bids.len(), 1);
        assert_eq!(token.balance(&alice), 500);
        assert_eq!(token.balance(&treasury), 0);
    }

    #[test]
    fn test_auction_no_bids() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let (asset, _, _) = setup_token(&env);
        let treasury = Address::generate(&env);
        let name = String::from_str(&env, "ghost.xlm");
        client.create_auction(&name, &asset, &treasury, &100, &10, &20);

        let settlement = client.settle(&name, &21);
        assert!(settlement.is_none());
    }

    #[test]
    fn test_auction_reserve_not_met() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let (asset, token_admin, token) = setup_token(&env);
        let treasury = Address::generate(&env);
        let alice = Address::generate(&env);

        token_admin.mint(&alice, &1000);
        let name = String::from_str(&env, "cheap.xlm");
        client.create_auction(&name, &asset, &treasury, &1000, &10, &20);
        client.place_bid(&name, &alice, &500, &15);

        let settlement = client.settle(&name, &21).unwrap();
        assert_eq!(settlement.winner, None);
        assert_eq!(settlement.clearing_price, 0);
        assert!(!settlement.sold);

        assert_eq!(token.balance(&alice), 1000);
        assert_eq!(token.balance(&treasury), 0);
    }

    #[test]
    fn test_auction_tie_behavior() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let (asset, token_admin, token) = setup_token(&env);
        let treasury = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);

        token_admin.mint(&alice, &1000);
        token_admin.mint(&bob, &1000);
        let name = String::from_str(&env, "tie.xlm");
        client.create_auction(&name, &asset, &treasury, &100, &10, &20);

        client.place_bid(&name, &alice, &500, &12);
        client.place_bid(&name, &bob, &500, &13);

        let settlement = client.settle(&name, &21).unwrap();
        // First bidder wins in case of tie in current implementation
        assert_eq!(settlement.winner, Some(alice.clone()));
        assert_eq!(settlement.clearing_price, 500);
        assert!(settlement.sold);

        assert_eq!(token.balance(&alice), 1000 - 500);
        assert_eq!(token.balance(&bob), 1000);
        assert_eq!(token.balance(&treasury), 500);
    }

    #[test]
    fn list_auctions_paginates_in_creation_order() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        // Empty state: every helper returns an empty Vec, count is zero.
        assert_eq!(client.auction_count(), 0);
        assert_eq!(client.list_auctions(&0, &10).len(), 0);
        assert_eq!(client.list_active_auctions(&0, &0, &10).len(), 0);
        assert_eq!(client.list_settled_auctions(&0, &10).len(), 0);

        // Create three auctions with distinct windows so we can drive the
        // active/settled filters independently of one another.
        let alpha = String::from_str(&env, "alpha.xlm");
        let beta = String::from_str(&env, "beta.xlm");
        let gamma = String::from_str(&env, "gamma.xlm");
        let asset = Address::generate(&env);
        let treasury = Address::generate(&env);
        client.create_auction(&alpha, &asset, &treasury, &100, &10, &20);
        client.create_auction(&beta, &asset, &treasury, &100, &30, &40);
        client.create_auction(&gamma, &asset, &treasury, &100, &50, &60);

        assert_eq!(client.auction_count(), 3);

        let page1 = client.list_auctions(&0, &2);
        assert_eq!(page1.len(), 2);
        assert_eq!(page1.get_unchecked(0), alpha);
        assert_eq!(page1.get_unchecked(1), beta);

        let page2 = client.list_auctions(&2, &2);
        assert_eq!(page2.len(), 1);
        assert_eq!(page2.get_unchecked(0), gamma);

        // Offset past the end is empty, not an error.
        assert_eq!(client.list_auctions(&99, &10).len(), 0);
    }

    #[test]
    fn list_active_and_settled_filters_partition_by_state() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let (asset, token_admin, _) = setup_token(&env);
        let treasury = Address::generate(&env);
        let bidder = Address::generate(&env);
        token_admin.mint(&bidder, &1000);

        let alpha = String::from_str(&env, "alpha.xlm");
        let beta = String::from_str(&env, "beta.xlm");
        let gamma = String::from_str(&env, "gamma.xlm");

        client.create_auction(&alpha, &asset, &treasury, &100, &10, &20);
        client.create_auction(&beta, &asset, &treasury, &100, &30, &40);
        client.create_auction(&gamma, &asset, &treasury, &100, &50, &60);

        // At t=15: only alpha is currently accepting bids. None settled.
        let active = client.list_active_auctions(&15, &0, &10);
        assert_eq!(active.len(), 1);
        assert_eq!(active.get_unchecked(0), alpha);
        assert_eq!(client.list_settled_auctions(&0, &10).len(), 0);

        // Settle alpha at t=21. After settlement it must drop out of the
        // active set even at a time inside its original bidding window.
        client.place_bid(&alpha, &bidder, &200, &12);
        client.settle(&alpha, &21);

        let still_active = client.list_active_auctions(&15, &0, &10);
        assert_eq!(still_active.len(), 0);

        let settled = client.list_settled_auctions(&0, &10);
        assert_eq!(settled.len(), 1);
        assert_eq!(settled.get_unchecked(0), alpha);

        // At t=35: beta is active, alpha is settled, gamma hasn't started.
        let active_mid = client.list_active_auctions(&35, &0, &10);
        assert_eq!(active_mid.len(), 1);
        assert_eq!(active_mid.get_unchecked(0), beta);

        // Pagination on filtered list: offset within matches works.
        let page = client.list_active_auctions(&35, &1, &10);
        assert_eq!(page.len(), 0);
    }

    #[test]
    fn list_helpers_cap_limit_at_max_page_size() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);
        let asset = Address::generate(&env);
        let treasury = Address::generate(&env);

        // Create a handful of auctions; ask for a huge limit and verify the
        // contract caps it at MAX_PAGE_SIZE instead of returning the full
        // index (which would be unbounded). Label minimum length is 3, so use
        // "namXY.xlm" rather than "nX.xlm".
        for i in 0..5u32 {
            let s = format!("nam{i:02}.xlm");
            let name = String::from_str(&env, &s);
            client.create_auction(&name, &asset, &treasury, &100, &10, &20);
        }
        let huge = client.list_auctions(&0, &u32::MAX);
        assert!(huge.len() <= crate::MAX_PAGE_SIZE);
        assert_eq!(huge.len(), 5);
    }

    #[test]
    fn test_auction_clearing_price_logic() {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);

        let (asset, token_admin, token) = setup_token(&env);
        let treasury = Address::generate(&env);
        let alice = Address::generate(&env);
        let bob = Address::generate(&env);
        let charlie = Address::generate(&env);

        token_admin.mint(&alice, &1000);
        token_admin.mint(&bob, &1000);
        token_admin.mint(&charlie, &1000);
        let name = String::from_str(&env, "multi.xlm");
        client.create_auction(&name, &asset, &treasury, &100, &10, &20);

        client.place_bid(&name, &alice, &1000, &12);
        client.place_bid(&name, &bob, &500, &13);
        client.place_bid(&name, &charlie, &750, &14);

        let settlement = client.settle(&name, &21).unwrap();
        assert_eq!(settlement.winner, Some(alice.clone()));
        assert_eq!(settlement.clearing_price, 750); // Second highest bid
        assert!(settlement.sold);

        assert_eq!(token.balance(&alice), 1000 - 750);
        assert_eq!(token.balance(&bob), 1000);
        assert_eq!(token.balance(&charlie), 1000);
        assert_eq!(token.balance(&treasury), 750);
    }

    // ── #157: auction discovery query helpers ──────────────────────────────

    #[test]
    fn discovery_queries_handle_empty_state() {
        let env = Env::default();
        let client = AuctionContractClient::new(&env, &env.register(AuctionContract, ()));
        assert_eq!(client.auction_names().len(), 0);
        assert_eq!(client.active_auctions(&100).len(), 0);
        assert_eq!(client.settled_auctions().len(), 0);
    }

    #[test]
    fn discovery_queries_filter_active_and_settled() {
        let env = Env::default();
        env.mock_all_auths();
        let (asset, token_admin, _) = setup_token(&env);
        let treasury = Address::generate(&env);
        let alice = Address::generate(&env);
        token_admin.mint(&alice, &1000);
        let client = AuctionContractClient::new(&env, &env.register(AuctionContract, ()));

        let a = String::from_str(&env, "alpha.xlm");
        let b = String::from_str(&env, "bravo.xlm");
        let c = String::from_str(&env, "charlie.xlm");
        client.create_auction(&a, &asset, &treasury, &100, &10, &20);
        client.create_auction(&b, &asset, &treasury, &100, &10, &20);
        client.create_auction(&c, &asset, &treasury, &100, &100, &200);

        // Index records every created auction, in creation order.
        let names = client.auction_names();
        assert_eq!(names.len(), 3);
        assert_eq!(names.get(0), Some(a.clone()));

        // At t=15: a and b are open; c hasn't started.
        let active = client.active_auctions(&15);
        assert_eq!(active.len(), 2);

        // Settle `a`, then it must move out of active and into settled.
        client.place_bid(&a, &alice, &500, &12);
        client.settle(&a, &21).unwrap();

        let active_after = client.active_auctions(&15);
        assert_eq!(active_after.len(), 1); // only b remains active
        assert_eq!(active_after.get(0).unwrap().name, b);

        let settled = client.settled_auctions();
        assert_eq!(settled.len(), 1);
        assert_eq!(settled.get(0).unwrap().name, a);
    }

    #[test]
    fn version_is_exposed() {
        let env = Env::default();
        let contract_id = env.register(AuctionContract, ());
        let client = AuctionContractClient::new(&env, &contract_id);
        assert_eq!(client.version(), 1);
    }
}
