# xlm-ens SDK Quickstart Guide

This guide will walk you through the process of integrating the xlm-ens SDK into your Rust application. You will learn how to install the SDK, configure a client, and perform common operations like resolving, registering, and managing names.

## 1. Installation

Add the `xlm-ns-sdk` crate to your `Cargo.toml` file:

```toml
[dependencies]
xlm-ns-sdk = "0.1.0" # Replace with the latest version
```

## 2. Configuration

First, you need to configure a client to connect to the xlm-ens contracts on the desired network.

```rust
use xlm_ns_sdk::Client;

fn main() {
    // Configure the client for the testnet
    let client = Client::new("https://rpc-testnet.stellar.org:443".to_string());
    println!("Successfully configured client for testnet");
}
```

## 3. Resolve a Name

You can resolve a `.xlm` name to get the associated address.

```rust
use xlm_ns_sdk::Client;

fn main() {
    let client = Client::new("https://rpc-testnet.stellar.org:443".to_string());
    let name = "test.xlm";
    match client.resolve_name(name) {
        Ok(address) => println!("{} resolves to {}", name, address),
        Err(e) => println!("Failed to resolve {}: {}", name, e),
    }
}
```

## 4. Check Availability

You can check if a name is available for registration.

```rust
use xlm_ns_sdk::Client;

fn main() {
    let client = Client::new("https://rpc-testnet.stellar.org:443".to_string());
    let name = "new-name.xlm";
    match client.is_available(name) {
        Ok(true) => println!("{} is available for registration", name),
        Ok(false) => println!("{} is not available for registration", name),
        Err(e) => println!("Failed to check availability for {}: {}", name, e),
    }
}
```

## 5. Register a Name

To register a name, you need to estimate the fee and then submit the registration transaction.

```rust
use xlm_ns_sdk::Client;

fn main() {
    let client = Client::new("https://rpc-testnet.stellar.org:443".to_string());
    let name = "new-name.xlm";
    let owner = "YOUR_ACCOUNT_ADDRESS"; // Replace with your account address

    // Estimate the registration fee
    match client.estimate_registration_fee(name) {
        Ok(fee) => {
            println!("Estimated registration fee for {}: {}", name, fee);
            // Register the name
            match client.register_name(name, owner, fee) {
                Ok(_) => println!("Successfully registered {}", name),
                Err(e) => println!("Failed to register {}: {}", name, e),
            }
        }
        Err(e) => println!("Failed to estimate registration fee for {}: {}", name, e),
    }
}
```

## 6. Set Records

Once you own a name, you can set various resolver records.

```rust
use xlm_ns_sdk::Client;
use xlm_ns_sdk::records::{Address, Text, ContentHash};


fn main() {
    let client = Client::new("https://rpc-testnet.stellar.org:443".to_string());
    let name = "your-name.xlm"; // Replace with your name
    let owner_keypair = "YOUR_KEYPAIR"; // Replace with your keypair

    // Set an address record
    let address_record = Address::new("YOUR_OTHER_ACCOUNT_ADDRESS".to_string());
    match client.set_record(name, &address_record, owner_keypair) {
        Ok(_) => println!("Successfully set address record for {}", name),
        Err(e) => println!("Failed to set address record for {}: {}", name, e),
    }

    // Set a text record
    let text_record = Text::new("twitter".to_string(), "@xlm_ens".to_string());
    match client.set_record(name, &text_record, owner_keypair) {
        Ok(_) => println!("Successfully set text record for {}", name),
        Err(e) => println!("Failed to set text record for {}: {}", name, e),
    }

}
```

## 7. Troubleshooting

Here are some common errors you might encounter:

*   **Wrong Network**: Ensure your client is configured to the correct network (testnet or mainnet).
*   **Insufficient Funds**: Make sure the account you are using to register a name has enough XLM to cover the registration fee.
*   **Name Taken**: If a name is already registered, you cannot register it again.
*   **Invalid Name**: Names must be at least 3 characters long and can only contain alphanumeric characters and hyphens.
*   **RPC Errors**: If you are having trouble connecting to the RPC endpoint, check the status of the Stellar network and the RPC provider.

## 8. Companion Example

A complete, runnable example project is available in the `examples/quickstart` directory of the SDK repository. You can run it with `cargo run`.
