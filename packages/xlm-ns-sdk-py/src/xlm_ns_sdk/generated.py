from __future__ import annotations

from dataclasses import asdict
from typing import Any, Dict, List, Optional

from .backend import AsyncContractBackend, ContractInvocation
from .models import (
    AddControllerRequest,
    AuctionCreateRequest,
    AuctionInfo,
    AuctionState,
    BidRequest,
    BridgeRoute,
    BuildMessageRequest,
    CreateSubdomainRequest,
    NameRecord,
    NftRecord,
    PortfolioPage,
    RegisterChainRequest,
    RegisterParentRequest,
    RegistrarMetrics,
    RegistrationQuote,
    RegistrationReceipt,
    RegistrationRequest,
    RegistryEntry,
    RenewResult,
    RenewalReceipt,
    RenewalRequest,
    ResolutionRecord,
    ResolutionResult,
    ReverseResolution,
    SimulationResult,
    Subdomain,
    TextRecord,
    TextRecordUpdate,
    TextRecordsUpdate,
    TransactionSubmission,
    TransferRequest,
    TransferSubdomainRequest,
)


class _ContractClientBase:
    def __init__(self, backend: AsyncContractBackend, contract_name: str, contract_id: Optional[str]) -> None:
        self._backend = backend
        self._contract_name = contract_name
        self._contract_id = contract_id

    async def _read(self, method: str, **arguments: Any) -> Any:
        return await self._backend.call(
            ContractInvocation(self._contract_name, self._contract_id, method, arguments, mode="read")
        )

    async def _write(self, method: str, **arguments: Any) -> Any:
        return await self._backend.call(
            ContractInvocation(self._contract_name, self._contract_id, method, arguments, mode="write")
        )


class RegistryContractClient(_ContractClientBase):
    async def resolve(self, name: str) -> ResolutionResult:
        return await self._read("resolve", name=name)

    async def get_registry_metadata(self, name: str) -> NameRecord:
        return await self._read("get_registry_metadata", name=name)

    async def get_owner_portfolio(self, owner: str) -> List[NameRecord]:
        return await self._read("get_owner_portfolio", owner=owner)

    async def get_registration(self, name: str) -> Optional[ResolutionResult]:
        return await self._read("get_registration", name=name)

    async def list_registrations_by_owner(self, owner: str) -> List[NameRecord]:
        return await self._read("list_registrations_by_owner", owner=owner)

    async def list_registrations_by_owner_page(self, owner: str, limit: int = 50, cursor: int = 0) -> PortfolioPage:
        return await self._read("list_registrations_by_owner_page", owner=owner, limit=limit, cursor=cursor)

    async def transfer(self, request: TransferRequest) -> TransactionSubmission:
        return await self._write("transfer", request=asdict(request))

    async def set_resolver(self, name: str, resolver: Optional[str]) -> TransactionSubmission:
        return await self._write("set_resolver", name=name, resolver=resolver)


class ResolverContractClient(_ContractClientBase):
    async def reverse_resolve(self, address: str) -> ReverseResolution:
        return await self._read("reverse_resolve", address=address)

    async def reverse_lookup(self, address: str) -> Optional[str]:
        return await self._read("reverse_lookup", address=address)

    async def get_primary_name(self, address: str) -> Optional[str]:
        return await self._read("get_primary_name", address=address)

    async def get_text_records(self, name: str) -> Dict[str, str]:
        return await self._read("get_text_records", name=name)

    async def get_text_record(self, name: str, key: str) -> TextRecord:
        return await self._read("get_text_record", name=name, key=key)

    async def set_text_record(self, request: TextRecordUpdate) -> TransactionSubmission:
        return await self._write("set_text_record", request=asdict(request))

    async def set_text_records(self, request: TextRecordsUpdate) -> TransactionSubmission:
        return await self._write("set_text_records", request=asdict(request))


class RegistrarContractClient(_ContractClientBase):
    async def quote_registration(self, label: str, duration_years: int) -> RegistrationQuote:
        return await self._read("quote_registration", label=label, duration_years=duration_years)

    async def register(self, request: RegistrationRequest) -> RegistrationReceipt:
        return await self._write("register", request=asdict(request))

    async def renew(self, request: RenewalRequest) -> RenewalReceipt:
        return await self._write("renew", request=asdict(request))

    async def simulate_register(self, request: RegistrationRequest) -> SimulationResult:
        return await self._read("simulate_register", request=asdict(request))

    async def simulate_renew(self, request: RenewalRequest) -> SimulationResult:
        return await self._read("simulate_renew", request=asdict(request))

    async def get_treasury_balance(self) -> int:
        return await self._read("get_treasury_balance")

    async def get_fee_metrics(self) -> RegistrarMetrics:
        return await self._read("get_fee_metrics")


class SubdomainContractClient(_ContractClientBase):
    async def register_parent(self, request: RegisterParentRequest) -> TransactionSubmission:
        return await self._write("register_parent", request=asdict(request))

    async def add_controller(self, request: AddControllerRequest) -> TransactionSubmission:
        return await self._write("add_controller", request=asdict(request))

    async def create_subdomain(self, request: CreateSubdomainRequest) -> Subdomain:
        return await self._write("create_subdomain", request=asdict(request))

    async def transfer_subdomain(self, request: TransferSubdomainRequest) -> TransactionSubmission:
        return await self._write("transfer_subdomain", request=asdict(request))

    async def get_subdomains(self, parent: str) -> List[Subdomain]:
        return await self._read("get_subdomains", parent=parent)


class BridgeContractClient(_ContractClientBase):
    async def register_chain(self, request: RegisterChainRequest) -> TransactionSubmission:
        return await self._write("register_chain", request=asdict(request))

    async def get_route(self, chain: str) -> Optional[BridgeRoute]:
        return await self._read("get_route", chain=chain)

    async def get_bridge_routes(self, name: str) -> List[BridgeRoute]:
        return await self._read("get_bridge_routes", name=name)

    async def build_message(self, request: BuildMessageRequest) -> str:
        return await self._read("build_message", request=asdict(request))


class NftContractClient(_ContractClientBase):
    async def mint_nft(self, token_id: str, owner: str) -> TransactionSubmission:
        return await self._write("mint_nft", token_id=token_id, owner=owner)

    async def approve_nft(self, token_id: str, operator: str) -> TransactionSubmission:
        return await self._write("approve_nft", token_id=token_id, operator=operator)

    async def transfer_nft(self, token_id: str, new_owner: str) -> TransactionSubmission:
        return await self._write("transfer_nft", token_id=token_id, new_owner=new_owner)

    async def get_nft(self, token_id: str) -> NftRecord:
        return await self._read("get_nft", token_id=token_id)

    async def get_nft_owner(self, token_id: str) -> str:
        return await self._read("get_nft_owner", token_id=token_id)

    async def get_nft_metadata(self, token_id: str) -> Optional[str]:
        return await self._read("get_nft_metadata", token_id=token_id)

    async def get_nft_record(self, token_id: str) -> NftRecord:
        return await self._read("get_nft_record", token_id=token_id)


class AuctionContractClient(_ContractClientBase):
    async def get_auction(self, name: str) -> Optional[AuctionInfo]:
        return await self._read("get_auction", name=name)

    async def get_auction_state(self, name: str) -> AuctionState:
        return await self._read("get_auction_state", name=name)

    async def create_auction(self, request: AuctionCreateRequest) -> TransactionSubmission:
        return await self._write("create_auction", request=asdict(request))

    async def bid_auction(self, request: BidRequest) -> TransactionSubmission:
        return await self._write("bid_auction", request=asdict(request))

    async def settle_auction(self, name: str) -> TransactionSubmission:
        return await self._write("settle_auction", name=name)

    async def simulate_and_submit(self, name: str, dry_run: bool = False) -> TransactionSubmission:
        return await self._write("simulate_and_submit", name=name, dry_run=dry_run)

    async def load_reserved_manifest(self, name: str) -> TransactionSubmission:
        return await self._write("load_reserved_manifest", name=name)

