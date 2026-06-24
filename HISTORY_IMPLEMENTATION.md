# xlm-ns history Command Implementation

## Overview

The `xlm-ns history` command provides CLI-native access to historical transactions involving xlm-ns domains and addresses directly from the terminal.

## Status

This is an **MVP (Minimum Viable Product)** implementation with the following components complete:

✅ Command registration and CLI argument parsing
✅ Event type classification system
✅ Caching infrastructure (filesystem-based, 5-minute TTL)
✅ Output formatting (human-readable, JSON, CSV)
✅ Provider interface abstraction
✅ Error handling and validation

## Architecture

### Command Handler
**File:** `cli/src/commands/history.rs`

Entry point: `run_history(config, address, name, limit, no_cache, output_format)`

### Core Components

#### EventType Enum
Classifies historical events into these categories:
- `register` - Initial domain registration
- `renew` - Name renewal or expiration extension
- `transfer` - Ownership transfer between addresses
- `bid` - Auction bid placement
- `auction_win` - Bid won an auction
- `auction_claim` - Claiming an auctioned name
- `auction_cancel` - Auction cancellation
- `resolver_update` - Resolver record modification
- `metadata_update` - Metadata changes
- `unknown` - Unclassified events

#### HistoryEvent Structure
```rust
pub struct HistoryEvent {
    pub timestamp: String,        // RFC3339 format
    pub ledger: u32,              // Ledger sequence number
    pub tx_hash: String,          // Transaction hash
    pub event_type: EventType,    // Classified event type
    pub name: Option<String>,     // Domain name if applicable
    pub owner: Option<String>,    // Owner address
    pub previous_owner: Option<String>,
    pub counterparty: Option<String>,
    pub amount: Option<String>,   // Amount in XLM
    pub fee: Option<String>,      // Fee in XLM
    pub contract_id: String,      // Contract that emitted event
    pub explorer_url: String,     // Link to Stellar Expert
    pub raw: Option<serde_json::Value>,
}
```

#### Caching Layer
**Features:**
- Platform-specific cache directories:
  - Linux: `~/.cache/xlm-ns/`
  - macOS: `~/Library/Caches/xlm-ns/`
  - Windows: `%LOCALAPPDATA%/xlm-ns/cache/`
- 5-minute TTL
- Filesystem-based storage
- Cache key: `history:{address}:{name}:{limit}`
- Bypass with `--no-cache` flag

#### Provider Interface
```rust
#[async_trait]
trait HistoryProvider {
    async fn get_address_history(&self, address: &str, limit: usize) 
        -> Result<Vec<HistoryEvent>>;
    
    async fn get_name_history(&self, name: &str, limit: usize) 
        -> Result<Vec<HistoryEvent>>;
}
```

Current implementation: `SorobanHistoryProvider` (skeleton)

### Output Formats

#### Human-Readable (default)
```
Name History
================================================================================

2026-06-24T12:42:00Z
REGISTER
Name: alice.xlm
Fee: 15 XLM
Tx: 4f2ca5...
Ledger: 61234567
--------------------------------------------------------------------------------

2026-05-10T18:33:00Z
RENEW
Name: alice.xlm
Fee: 5 XLM
Tx: 6d1a8f...
Ledger: 61234566
```

#### JSON (--format json)
```json
[
  {
    "timestamp": "2026-06-24T12:42:00Z",
    "type": "register",
    "name": "alice.xlm",
    "fee": "15",
    "tx_hash": "4f2ca5...",
    "ledger": 61234567,
    "explorer_url": "https://stellar.expert/explorer/public/tx/4f2ca5..."
  }
]
```

#### CSV (--format csv)
```
timestamp,type,name,owner,counterparty,amount,fee,tx_hash,ledger,explorer_url
2026-06-24T12:42:00Z,register,alice.xlm,,,,15,4f2ca5...,61234567,https://stellar.expert/...
```

## Implementation Status

### ✅ Completed
- Event type classification system
- Caching infrastructure
- Output formatting (all three formats)
- Provider abstraction
- Command registration
- Argument validation
- Error handling
- Explorer URL generation

### 🚧 In Progress / TODO

#### Phase 1: Event Fetching Implementation
**Required:** Implement `SorobanHistoryProvider` to query actual events

```rust
async fn get_address_history(&self, address: &str, limit: usize) 
    -> Result<Vec<HistoryEvent>>
{
    // 1. Create Soroban RPC client
    let rpc = self.client.rpc_client()?;
    
    // 2. Query events using soroban_rpcEventGetEventsByTopics
    //    with filter for:
    //    - Contract IDs: registry, registrar, resolver, auction
    //    - Topics containing address
    
    // 3. Parse event data from XDR
    
    // 4. Classify by contract and event structure
    
    // 5. Transform into HistoryEvent objects
    
    // 6. Sort by timestamp (descending)
    
    // 7. Apply limit
    
    // 8. Return results
}
```

**Key Methods Needed:**
- Query `stellar_rpc_client::Client::get_events()` (or equivalent)
- XDR parsing for event data
- Event topic filtering
- Timestamp extraction from ledger data

#### Phase 2: Event Parsing Enhancement

Implement detailed event parsing for each contract:

**Registry Contract Events:**
- Extract owner address from event topics
- Detect ownership changes

**Registrar Contract Events:**
- Parse registration/renewal parameters
- Extract fee information
- Detect grace period entries

**Auction Contract Events:**
- Parse bid amounts
- Track auction winners
- Detect claim operations

**Resolver Contract Events:**
- Extract text record updates
- Track resolver changes

#### Phase 3: Performance Optimization

- Implement cursor-based pagination
- Add date range filtering (`--from-date`, `--to-date`)
- Support event type filtering (`--type register`)
- Add contract filtering (`--contract registrar`)

#### Phase 4: Advanced Features

- Live event streaming (`--follow`)
- CSV/Markdown export
- Terminal hyperlinks for explorer URLs
- TUI history viewer

## Usage Examples

### Basic history for an address
```bash
xlm-ns history GB4P7...ABC
```

### Filter by domain name
```bash
xlm-ns history --name alice.xlm
```

### JSON output for automation
```bash
xlm-ns history GB4P7...ABC --format json | jq '.[] | select(.type == "transfer")'
```

### Skip cache for fresh data
```bash
xlm-ns history GB4P7...ABC --no-cache
```

### Combined example
```bash
xlm-ns history GB4P7...ABC \
  --name alice.xlm \
  --limit 10 \
  --format json
```

## File Structure

```
cli/src/
├── commands/
│   ├── history.rs           # Command handler + all logic
│   └── mod.rs               # (updated: added history module)
├── main.rs                  # (updated: added History command variant)
└── ...
```

## Dependencies Added

- `async-trait` (0.1) - For `#[async_trait]` on trait methods
- `chrono` (0.4) - For timestamp parsing and formatting

## Testing

Current implementation has skeleton structure for testing. To test:

```bash
# Build
cargo build -p xlm-ns-cli

# Run help
./target/debug/xlm-ns history --help

# Test with mock data (returns empty for now)
xlm-ns history GBRPYHIL2CI3WHQTLTPGXUNA77MRWIVFVZL4CVXYDW2TP7VVDZ5GPO7
```

## Next Steps for Extension

### To implement real event fetching:

1. **Review stellar-rpc-client API**
   - Check available methods for event queries
   - Understand XDR parsing requirements
   - Review event topic structure

2. **Implement get_address_history**
   - Query Soroban RPC for events involving the address
   - Parse XDR event data
   - Filter by xlm-ns contract IDs
   - Classify events by type
   - Return sorted results

3. **Implement get_name_history**
   - Similar to above but filter by domain name in event data
   - May require looking up registry entries for the name

4. **Add integration tests**
   - Test event parsing with sample XDR data
   - Test cache behavior
   - Test output formatting

5. **Validate on testnet**
   - Connect to testnet RPC
   - Verify event retrieval
   - Confirm classification accuracy

## Known Limitations

- Currently returns empty event lists (skeleton implementation)
- No real Soroban RPC event queries yet
- No timestamp parsing from actual ledger data
- No XDR parsing for event content
- No filtering by event types in implementation

## Contract IDs Reference

The command recognizes these contracts (configured via config file):
- `registry_contract_id` - Domain registry
- `registrar_contract_id` - Registration logic
- `resolver_contract_id` - Resolution records
- `auction_contract_id` - Auction management
- `bridge_contract_id` - Bridge routes
- `subdomain_contract_id` - Subdomain management
- `nft_contract_id` - NFT ownership metadata

## Configuration

Contract IDs are loaded from:
1. CLI arguments: `--registry-contract-id`, etc.
2. Environment variables: `REGISTRY_CONTRACT_ID`, etc.
3. Config file (TOML)
4. Defaults (mainnet/testnet presets)

See `config.rs` for details.

## Error Messages

Handled scenarios:
- Invalid Stellar address: "Invalid Stellar address: {error}"
- Missing address and name: "Either <address> or --name must be provided"
- Network failure: (forwarded from RPC client)
- No activity: "No xlm-ns activity found for this address."
- Cache errors: Silently fall back to fresh fetch

## Future Considerations

- Multi-address history aggregation
- Account watcher (monitor address continuously)
- Export to external formats (Parquet, database)
- Real-time notifications via webhooks
- Integration with analytics platforms
