# xlm-ns-sdk

Async + blocking Rust SDK for the xlm-ns name service contracts on Soroban.

## Two surfaces

- **`XlmNsClient`** — async API, the canonical surface. Use this from any
  service already running on a tokio runtime.
- **`XlmNsBlockingClient`** — synchronous wrapper around the async client.
  Owns its own current-thread runtime so CLIs and scripts can use the SDK
  without taking on tokio plumbing.

The blocking client is implemented on top of the async one: every blocking
call drives the same async method through `runtime.block_on`. There is no
duplicated logic — the async path is the source of truth.

## Configuration

Transport-level controls live on `ClientConfig`:

| Field | Default | Purpose |
|---|---|---|
| `timeout` | `30s` | Per-request timeout. Bounds a single RPC call (not the total wall-clock across retries). |
| `retry.max_retries` | `3` | Number of retry attempts on transient transport errors. `0` disables retries. |
| `retry.initial_backoff` | `1s` | Initial delay before the first retry; doubles per attempt. |
| `retry.max_backoff` | `30s` | Cap on the exponential backoff delay. |
| `retry.jitter` | `true` | Randomize each retry delay uniformly in `[0, backoff]`. |
| `user_agent` | `xlm-ns-sdk/<crate-version>` | Sent as the HTTP `User-Agent` so operators can identify SDK traffic in upstream logs. |
| `batch_chunk_size` | `50` | Names sent per `batch_resolve` invocation. Larger batches are split automatically. |

Override anything with the chainable setters:

```rust
use std::time::Duration;
use xlm_ns_sdk::{ClientConfig, XlmNsClient};

let client = XlmNsClient::builder("https://soroban-rpc.example")
    .registry("CDAD...REGISTRY")
    .config(
        ClientConfig::default()
            .with_timeout(Duration::from_secs(10))
            .with_max_retries(5)
            .with_user_agent("my-service/1.2.3"),
    )
    .build();
```

## Async usage

```rust
use xlm_ns_sdk::{types::RegistrationRequest, XlmNsClient};

# async fn run() -> Result<(), xlm_ns_sdk::SdkError> {
let client = XlmNsClient::builder("https://soroban-rpc.example")
    .network_passphrase("Test SDF Network ; September 2015")
    .registry("CDAD...REGISTRY")
    .registrar("CDAD...REGISTRAR")
    .build();

let resolution = client.resolve("alice.xlm").await?;
println!("alice.xlm -> {:?}", resolution.address);

let receipt = client.register(RegistrationRequest {
    label: "bob".into(),
    owner: "GDRA...OWNER".into(),
    duration_years: 1,
    signer: Some("treasury".into()),
}).await?;
println!("registered {} for {} years", receipt.name, receipt.duration_years);
# Ok(()) }
```

## Batch resolution

`batch_resolve` maps onto the resolver contract's `batch_resolve` entry point,
so resolving `n` names costs one invocation per chunk instead of `n` separate
round-trips.

One bad name never fails the batch. Results come back in input order, one
`BatchResult` per name, and a name that could not be resolved carries a
`BatchResolveError` while its neighbours still return addresses:

```rust
use xlm_ns_sdk::XlmNsClient;

# async fn run() -> Result<(), xlm_ns_sdk::SdkError> {
let client = XlmNsClient::builder("https://soroban-rpc.example")
    .registry("CDAD...REGISTRY")
    .resolver("CDAD...RESOLVER")
    .build();

let results = client.batch_resolve(vec![
    "alice.xlm".into(),
    "bob.xlm".into(),
    "expired.xlm".into(),
]).await?;

for result in &results {
    match result.as_result() {
        Ok(address) => println!("{} -> {address} (ttl {:?})", result.name, result.ttl_seconds),
        Err(err) => eprintln!("{} failed: {err}", result.name),
    }
}
# Ok(()) }
```

`Err` from `batch_resolve` itself is reserved for problems with the request —
an unconfigured or malformed resolver contract ID, for instance.

### Chunking and retries

Soroban bounds the resources a single invocation may consume, so batches larger
than `ClientConfig::batch_chunk_size` (default 50) are split transparently.
Each chunk is retried independently under the client's `RetryConfig`; a chunk
that still fails after its retries marks only its own names with
`BatchResolveError::Rpc`, leaving the other chunks' results intact.

Raise the chunk size once you have measured that your names fit:

```rust
use xlm_ns_sdk::{ClientConfig, XlmNsClient};

let client = XlmNsClient::builder("https://soroban-rpc.example")
    .registry("CDAD...REGISTRY")
    .resolver("CDAD...RESOLVER")
    .config(ClientConfig::default().with_batch_chunk_size(100))
    .build();
```

Per-name failure variants: `InvalidName` (rejected client-side, never sent),
`NotFound`, `Expired`, `NoAddress`, and `Rpc`.

## Blocking usage

```rust
use xlm_ns_sdk::{XlmNsBlockingClient, XlmNsClient};

# fn run() -> Result<(), xlm_ns_sdk::SdkError> {
let client = XlmNsBlockingClient::from_async(
    XlmNsClient::builder("https://soroban-rpc.example")
        .registry("CDAD...REGISTRY")
        .build(),
)?;

let resolution = client.resolve("alice.xlm")?;
println!("alice.xlm -> {:?}", resolution.address);
# Ok(()) }
```

## Integration tests against a local Soroban node

See [`docs/sdk-integration-tests.md`](../../docs/sdk-integration-tests.md) for
the full local setup. The suite is gated on `XLM_NS_LIVE_SDK_TESTS=1` so it
does not run in default CI; once the env vars are set it covers a read path
(`resolve`) and a write path (`renew`) against deployed contracts.

## Spec drift

`scripts/check-sdk-bindings.sh` validates that every method the SDK calls
still exists on the corresponding contract. CI runs it as part of the
artifacts job; run it locally after rebuilding the WASM artifacts to catch
drift before opening a PR.
