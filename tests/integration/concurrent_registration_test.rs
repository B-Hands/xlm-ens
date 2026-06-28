use soroban_sdk::{testutils::Address as _, Address, Env, String};
use xlm_ns_auction::{AuctionContract, AuctionContractClient};
use xlm_ns_registrar::{RegistrarContract, RegistrarContractClient};
use xlm_ns_registry::{RegistryContract, RegistryContractClient};
use soroban_sdk::token::{StellarAssetClient, Client};

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

fn setup_registrar_registry() -> (Env, RegistrarContractClient<'static>, RegistryContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let registry_id = env.register(RegistryContract, ());
    let registrar_id = env.register(RegistrarContract, ());

    let registrar = RegistrarContractClient::new(&env, &registrar_id);
    let registry = RegistryContractClient::new(&env, &registry_id);

    // Wire the registrar to the registry.
    registrar.initialize(&registry_id);

    (env, registrar, registry)
}

fn setup_auction_and_token() -> (Env, AuctionContractClient<'static>, Address, StellarAssetClient<'static>, Client<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let auction_id = env.register(AuctionContract, ());
    let auction_client = AuctionContractClient::new(&env, &auction_id);

    // Initialize auction with admin
    auction_client.initialize(&admin);

    // Setup token
    let token_admin = Address::generate(&env);
    let contract = env.register_stellar_asset_contract(token_admin.clone());
    let token_asset = StellarAssetClient::new(&env, &contract);
    let token_client = Client::new(&env, &contract);

    (env, auction_client, token_admin, token_asset, token_client)
}

#[test]
fn test_concurrent_registration_same_ledger() {
    let (env, registrar, registry) = setup_registrar_registry();
    let owner1 = Address::generate(&env);
    let owner2 = Address::generate(&env);
    let label = String::from_str(&env, "alice");
    let name = String::from_str(&env, "alice.xlm");
    let time = TimeHelper::new(1_000_000);

    // Get a quote for registration
    let quote = registrar.quote_registration(&label, &1, &time.now);
    let fee_stroops = quote.fee_stroops;

    // Fund the accounts with enough lumens (native asset) to pay for registration
    // In Soroban test env, we can assume accounts have sufficient balance for native asset.
    // But we need to set the balance? Actually, the test environment doesn't charge for transactions,
    // but the contract checks the payment. We'll just provide the payment from the account.
    // We don't need to actually transfer native asset in test env because the contract doesn't check balance?
    // It does: the user must provide the payment, and the contract will subtract from treasury.
    // The user's balance is not checked by the contract; it's assumed the user has provided the amount.
    // So we just need to pass the payment_stroops.

    // We'll simulate two transactions in the same ledger by invoking the registrar.register function twice
    // in the same environment (same ledger state) without advancing time.

    // First registration attempt (should succeed)
    let result1 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        registrar.register(&label, &owner1, &1, &fee_stroops, &time.now);
    }));
    assert!(result1.is_ok(), "First registration should succeed");

    // Second registration attempt (should fail because name is now taken)
    let result2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        registrar.register(&label, &owner2, &1, &fee_stroops, &time.now);
    }));
    assert!(result2.is_err(), "Second registration should fail due to already registered");

    // Verify that the first owner is the owner in both contracts
    let reg_record = registrar.registration(&name).expect("Registrar record should exist");
    assert_eq!(reg_record.owner, owner1);
    let reg_entry = registry.resolve(&name, &time.now).expect("Registry entry should exist");
    assert_eq!(reg_entry.owner, owner1);

    // Ensure the second owner does not have any record
    assert!(registrar.registration(&name).map(|r| r.owner).ok() != Some(owner2));
}

#[test]
fn test_registration_racing_against_auction_settlement() {
    // Setup environment with registrar, registry, auction, and token
    let (env, registrar, registry) = setup_registrar_registry();
    let (_, auction_client, token_admin, token_asset, token_client) = setup_auction_and_token();

    let auction_winner = Address::generate(&env);
    let auction_loser = Address::generate(&env); // This user will lose the auction but is needed for Vickrey pricing
    let challenger = Address::generate(&env); // Another user trying to register the same name

    let label = String::from_str(&env, "auction_name");
    let name = String::from_str(&env, "auction_name.xlm");
    let mut time = TimeHelper::new(1_000_000);

    // Create an auction
    let reserve_price = 100;
    let starts_at = time.now;
    let ends_at = time.future(100); // Auction ends in 100 seconds

    auction_client.create_auction(
        &name,
        &token_asset.address,
        &Address::generate(&env), // treasury (we don't care about treasury for this test)
        &reserve_price,
        &starts_at,
        &ends_at,
    );

    // Fund bidders with tokens
    token_admin.mint(&auction_winner, &1000);
    token_admin.mint(&challenger, &1000);

    // Place bids
    auction_client.place_bid(&name, &auction_winner, &500, &time.future(10)); // Higher bid
    auction_client.place_bid(&name, &auction_loser, &300, &time.future(20));  // Lower bid

    // Advance time past auction end
    time.advance(101); // Now after ends_at

    // Now, before the auction is settled, the challenger races to register the name directly.
    // This simulates a front-running scenario.

    // Get registration quote
    let quote = registrar.quote_registration(&label, &1, &time.now);
    let fee_stroops = quote.fee_stroops;

    // Challenger registers the name first. This should succeed.
    registrar.register(&label, &challenger, &1, &fee_stroops, &time.now);

    // Verify challenger is the new owner.
    let reg_entry = registry.resolve(&name, &time.now).expect("Registry entry should exist for challenger");
    assert_eq!(reg_entry.owner, challenger);

    // Now, the auction winner attempts to settle the auction.
    // The settlement logic should detect the name is already taken and fail gracefully,
    // refunding the winner's bid.
    let settlement = auction_client.settle(&name, &time.now).expect("Settlement should run");

    // Because the name was taken, the auction is considered unsold from the winner's perspective.
    assert!(!settlement.sold, "Settlement should indicate the name was not sold");
    assert_eq!(settlement.winner, None, "There should be no winner as the name was already taken");

    // Verify the auction winner was fully refunded.
    // The auction contract should have transferred the 500 bid back to the auction_winner.
    assert_eq!(token_client.balance(&auction_winner), 1000, "Auction winner should be fully refunded");

    // The auction loser also gets their bid back.
    assert_eq!(token_client.balance(&auction_loser), 1000, "Auction loser should be fully refunded");
}