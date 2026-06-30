# xlm-ns-sdk

Python SDK for the xlm-ens Soroban contracts.

This package is designed for analytics and notebook workflows:

- typed dataclass models for contract inputs and outputs
- async contract clients for high-throughput collection
- blocking wrappers for scripts and notebooks that prefer synchronous code
- pandas-friendly helpers for bulk queries and aggregations
- a generator script that can refresh the contract bindings from Soroban spec JSON

## Install

```bash
pip install xlm-ns-sdk
```

For notebook workflows:

```bash
pip install "xlm-ns-sdk[notebooks]"
```

## Structure

- `xlm_ns_sdk.models` contains the typed data classes and enums.
- `xlm_ns_sdk.generated` contains the contract client bindings.
- `xlm_ns_sdk.client` exposes the high-level async and blocking facades.
- `xlm_ns_sdk.analytics` contains pandas helpers for bulk analysis.
- `scripts/generate_python_sdk.py` regenerates bindings from spec JSON.

## Example

```python
from xlm_ns_sdk import AsyncXlmNsClient, StaticContractBackend

backend = StaticContractBackend()
client = AsyncXlmNsClient(backend)
quote = await client.registrar.quote_registration("alice", duration_years=1)
print(quote.total_fee)
```

The notebook examples under `notebooks/` show registration trends, pricing
analysis, and ownership distribution workflows using pandas DataFrames.

