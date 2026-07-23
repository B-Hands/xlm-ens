//! Batch resolution example.
//!
//! Run with:
//!
//! ```sh
//! cargo run -p xlm-ns-sdk --example batch_resolve -- \
//!     https://soroban-rpc.example CDAD...REGISTRY CDAD...RESOLVER alice.xlm bob.xlm
//! ```
//!
//! Defaults are used when the args are omitted. The example shows the shape of
//! a bulk integration: one call for the whole list, per-name error handling
//! instead of an all-or-nothing result, and an explicit chunk size for callers
//! who have measured how many of their names fit in one invocation.

use std::env;
use std::time::Duration;

use xlm_ns_sdk::{BatchResolveError, ClientConfig, XlmNsClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let rpc_url = args
        .first()
        .cloned()
        .unwrap_or_else(|| "http://localhost:8000/soroban/rpc".into());
    let registry_id = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "CDAD...REGISTRY".into());
    let resolver_id = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "CDAD...RESOLVER".into());

    let names: Vec<String> = if args.len() > 3 {
        args[3..].to_vec()
    } else {
        vec!["alice.xlm".into(), "bob.xlm".into(), "carol.xlm".into()]
    };

    let client = XlmNsClient::builder(rpc_url)
        .registry(registry_id)
        .resolver(resolver_id)
        .config(
            ClientConfig::default()
                .with_timeout(Duration::from_secs(15))
                .with_max_retries(2)
                // Batches longer than this are split across several
                // invocations, transparently to this call site.
                .with_batch_chunk_size(25)
                .with_user_agent("batch-resolve-example/0.1"),
        )
        .build();

    println!("resolving {} name(s) in one batch...", names.len());
    let results = client.batch_resolve(names).await?;

    let mut resolved = 0usize;
    let mut failed = 0usize;

    for result in &results {
        match result.as_result() {
            Ok(address) => {
                resolved += 1;
                println!(
                    "  ok   {} -> {address} (ttl: {:?}, expires_at: {:?})",
                    result.name, result.ttl_seconds, result.expires_at,
                );
            }
            Err(err) => {
                failed += 1;
                // A per-name failure leaves every other name in the batch
                // untouched — worth branching on when deciding whether to
                // retry, skip, or surface the name to a user.
                let action = match err {
                    BatchResolveError::InvalidName { .. } => "fix the input",
                    BatchResolveError::NotFound => "name is unregistered",
                    BatchResolveError::Expired { .. } => "renew or re-register",
                    BatchResolveError::NoAddress => "owner has set no address",
                    BatchResolveError::Rpc { .. } => "transient — safe to retry",
                };
                println!("  fail {} -> {err} ({action})", result.name);
            }
        }
    }

    println!("{resolved} resolved, {failed} failed");
    Ok(())
}
