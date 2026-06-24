use xlm_ns_sdk::Client;

#[tokio::main]
async fn main() {
    // Configure the client for the testnet
    let client = Client::new("https://rpc-testnet.stellar.org:443".to_string());
    println!("Successfully configured client for testnet");

    // Resolve a Name
    let name_to_resolve = "test.xlm";
    println!("
Attempting to resolve name: {}", name_to_resolve);
    match client.resolve_name(name_to_resolve).await {
        Ok(address) => println!("{} resolves to {}", name_to_resolve, address),
        Err(e) => println!("Failed to resolve {}: {}", name_to_resolve, e),
    }

    // Check Availability
    let name_to_check = "new-name-for-quickstart.xlm";
    println!("
Checking availability of name: {}", name_to_check);
    match client.is_available(name_to_check).await {
        Ok(true) => println!("{} is available for registration", name_to_check),
        Ok(false) => println!("{} is not available for registration", name_to_check),
        Err(e) => println!("Failed to check availability for {}: {}", name_to_check, e),
    }

    // Register a Name
    // Please replace with a valid testnet account address and secret key
    let name_to_register = "new-name-for-quickstart.xlm";
    let owner_address = "G.......................................................";
    let owner_secret = "S.......................................................";

    if owner_address == "G......................................................." {
        println!("
Skipping name registration and setting records.");
        println!("Please replace the placeholder account address and secret key in `examples/quickstart/src/main.rs` to run this part of the example.");
    } else {
        println!("
Attempting to register name: {}", name_to_register);
        
        // Estimate the registration fee
        match client.estimate_registration_fee(name_to_register).await {
            Ok(fee) => {
                println!("Estimated registration fee for {}: {}", name_to_register, fee);
                // Register the name
                match client.register_name(name_to_register, owner_address, fee).await {
                    Ok(_) => {
                        println!("Successfully registered {}", name_to_register);

                        // Set Records
                        println!("
Attempting to set records for: {}", name_to_register);
                        
                        // Set an address record
                        let address_record = xlm_ns_sdk::records::Address::new("G.......................................................".to_string());
                        match client.set_record(name_to_register, &address_record, owner_secret).await {
                            Ok(_) => println!("Successfully set address record for {}", name_to_register),
                            Err(e) => println!("Failed to set address record for {}: {}", name_to_register, e),
                        }

                        // Set a text record
                        let text_record = xlm_ns_sdk::records::Text::new("twitter".to_string(), "@xlm_ens".to_string());
                        match client.set_record(name_to_register, &text_record, owner_secret).await {
                            Ok(_) => println!("Successfully set text record for {}", name_to_register),
                            Err(e) => println!("Failed to set text record for {}: {}", name_to_register, e),
                        }
                    },
                    Err(e) => println!("Failed to register {}: {}", name_to_register, e),
                }
            }
            Err(e) => println!("Failed to estimate registration fee for {}: {}", name_to_register, e),
        }
    }
}
