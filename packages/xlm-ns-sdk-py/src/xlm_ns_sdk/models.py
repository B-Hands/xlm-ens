from __future__ import annotations

from dataclasses import asdict, dataclass
from enum import Enum
from typing import Any, Dict, Iterable, List, Mapping, Optional, Sequence, TypeVar

import pandas as pd


def to_frame(items: Sequence[object]) -> pd.DataFrame:
    return pd.DataFrame([asdict(item) if hasattr(item, "__dataclass_fields__") else item for item in items])


class SubmissionStatus(str, Enum):
    SIMULATED = "simulated"
    SUBMITTED = "submitted"
    CONFIRMED = "confirmed"
    FAILED = "failed"


class AuctionStatus(str, Enum):
    ACTIVE = "active"
    ENDED = "ended"
    SETTLED = "settled"


@dataclass
class FeeBreakdown:
    base_fee: int
    premium_fee: int
    network_fee: int

    @property
    def total(self) -> int:
        return self.base_fee + self.premium_fee + self.network_fee


@dataclass
class RegistrationRequest:
    label: str
    owner: str
    duration_years: int
    signer: Optional[str] = None


@dataclass
class RegistrationQuote:
    label: str
    duration_years: int
    fee_breakdown: FeeBreakdown
    total_fee: int
    fee_currency: str
    expires_at: int
    grace_period_ends_at: int
    quoted_at: int
    contract_id: Optional[str] = None

    @property
    def fee(self) -> int:
        return self.total_fee


@dataclass
class RenewalRequest:
    name: str
    additional_years: int
    signer: Optional[str] = None


@dataclass
class TransactionSubmission:
    tx_hash: str
    status: SubmissionStatus
    ledger: Optional[int]
    submitted_at: int
    contract_id: Optional[str]
    network_passphrase: Optional[str]
    signer: Optional[str]


@dataclass
class RegistrationReceipt:
    name: str
    owner: str
    duration_years: int
    expires_at: int
    fee_paid: int
    submission: TransactionSubmission


@dataclass
class RenewResult:
    name: str
    new_expiry_ledger: int
    tx_hash: str
    ledger_sequence: int


@dataclass
class RenewalReceipt:
    name: str
    additional_years: int
    new_expiry: int
    fee_paid: int
    submission: TransactionSubmission


@dataclass
class ResolutionResult:
    name: str
    address: Optional[str]
    resolver: Optional[str]
    expires_at: Optional[int]
    is_wildcard: bool


@dataclass
class PortfolioPage:
    items: List[ResolutionResult]
    next_cursor: Optional[int]
    total: int


@dataclass
class ReverseResolution:
    address: str
    primary_name: Optional[str]
    resolver: Optional[str]


@dataclass
class TextRecord:
    name: str
    key: str
    value: Optional[str]


@dataclass
class TextRecordUpdate:
    name: str
    key: str
    value: Optional[str]
    signer: Optional[str] = None


@dataclass
class TextRecordsUpdate:
    name: str
    records: Mapping[str, Optional[str]]
    signer: Optional[str] = None


@dataclass
class TransferRequest:
    name: str
    new_owner: str
    signer: Optional[str] = None


@dataclass
class RegisterParentRequest:
    parent: str
    owner: str


@dataclass
class AddControllerRequest:
    parent: str
    controller: str


@dataclass
class CreateSubdomainRequest:
    label: str
    parent: str
    owner: str


@dataclass
class TransferSubdomainRequest:
    fqdn: str
    new_owner: str


@dataclass
class ParentDomain:
    owner: str
    controllers: List[str]


@dataclass
class SubdomainRecord:
    parent: str
    owner: str
    created_at: int


@dataclass
class Subdomain:
    label: str
    owner: str


@dataclass
class RegisterChainRequest:
    chain: str


@dataclass
class BuildMessageRequest:
    name: str
    chain: str


@dataclass
class BridgeRoute:
    destination_chain: str
    destination_resolver: str
    gateway: str


@dataclass
class NftRecord:
    token_id: str
    owner: str
    metadata_uri: Optional[str]


@dataclass
class RegistrarMetrics:
    treasury_balance: int
    total_registrations: int
    total_renewals: int


@dataclass
class NameRecord:
    owner: str
    registered_at: int
    expires_at: int
    grace_period_ends_at: int
    resolver: Optional[str]


@dataclass
class AuctionState:
    highest_bid: int
    end_time: int


@dataclass
class RegistryEntry:
    name: str
    owner: str
    resolver: Optional[str]
    target_address: Optional[str]
    metadata_uri: Optional[str]
    ttl_seconds: int
    registered_at: int
    expires_at: int
    grace_period_ends_at: int
    transfer_count: int


@dataclass
class ResolutionRecord:
    owner: str
    address: str
    text_records: Dict[str, str]
    updated_at: int
    is_wildcard: bool


@dataclass
class AuctionInfo:
    name: str
    owner: str
    reserve_price: int
    highest_bid: int
    highest_bidder: Optional[str]
    ends_at: int
    status: AuctionStatus


@dataclass
class AuctionCreateRequest:
    name: str
    asset: str
    treasury: str
    reserve_price: int
    duration_seconds: int
    signer: Optional[str] = None


@dataclass
class BidRequest:
    name: str
    amount: int
    signer: Optional[str] = None


@dataclass
class SimulationResult:
    fee_estimate: int
    auth_addresses: List[str]
    error: Optional[str]
    success: bool

