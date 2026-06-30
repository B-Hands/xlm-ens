from __future__ import annotations

import asyncio
import threading
from dataclasses import asdict
from typing import Any, Iterable, List, Optional, Sequence

import pandas as pd

from .analytics import list_names_frame, name_price_analysis, ownership_distribution
from .backend import AsyncContractBackend
from .generated import (
    AuctionContractClient,
    BridgeContractClient,
    NftContractClient,
    RegistrarContractClient,
    RegistryContractClient,
    ResolverContractClient,
    SubdomainContractClient,
)
from .models import NameRecord, RegistrationQuote


class _LoopRunner:
    def __init__(self) -> None:
        self._loop = asyncio.new_event_loop()
        self._thread = threading.Thread(target=self._run_loop, daemon=True)
        self._thread.start()

    def _run_loop(self) -> None:
        asyncio.set_event_loop(self._loop)
        self._loop.run_forever()

    def run(self, coro: Any) -> Any:
        future = asyncio.run_coroutine_threadsafe(coro, self._loop)
        return future.result()

    def close(self) -> None:
        self._loop.call_soon_threadsafe(self._loop.stop)
        self._thread.join(timeout=1)


class AsyncXlmNsClient:
    def __init__(
        self,
        backend: AsyncContractBackend,
        *,
        registry_contract_id: Optional[str] = None,
        registrar_contract_id: Optional[str] = None,
        resolver_contract_id: Optional[str] = None,
        subdomain_contract_id: Optional[str] = None,
        bridge_contract_id: Optional[str] = None,
        nft_contract_id: Optional[str] = None,
        auction_contract_id: Optional[str] = None,
    ) -> None:
        self.registry = RegistryContractClient(backend, "registry", registry_contract_id)
        self.registrar = RegistrarContractClient(backend, "registrar", registrar_contract_id)
        self.resolver = ResolverContractClient(backend, "resolver", resolver_contract_id)
        self.subdomain = SubdomainContractClient(backend, "subdomain", subdomain_contract_id)
        self.bridge = BridgeContractClient(backend, "bridge", bridge_contract_id)
        self.nft = NftContractClient(backend, "nft", nft_contract_id)
        self.auction = AuctionContractClient(backend, "auction", auction_contract_id)

    async def list_names(self, owners: Sequence[str]) -> pd.DataFrame:
        portfolios = await asyncio.gather(*(self.registry.get_owner_portfolio(owner) for owner in owners))
        records: List[NameRecord] = [record for portfolio in portfolios for record in portfolio]
        return list_names_frame(records)

    async def ownership_distribution(self, owners: Sequence[str]) -> pd.DataFrame:
        portfolios = await asyncio.gather(*(self.registry.get_owner_portfolio(owner) for owner in owners))
        records = [record for portfolio in portfolios for record in portfolio]
        return ownership_distribution(records)

    async def name_price_analysis(self, quotes: Sequence[RegistrationQuote]) -> pd.DataFrame:
        return name_price_analysis(quotes)


class XlmNsClient:
    def __init__(self, async_client: AsyncXlmNsClient) -> None:
        self._async = async_client
        self._runner = _LoopRunner()

    @property
    def registry(self) -> Any:
        return self._async.registry

    @property
    def registrar(self) -> Any:
        return self._async.registrar

    @property
    def resolver(self) -> Any:
        return self._async.resolver

    @property
    def subdomain(self) -> Any:
        return self._async.subdomain

    @property
    def bridge(self) -> Any:
        return self._async.bridge

    @property
    def nft(self) -> Any:
        return self._async.nft

    @property
    def auction(self) -> Any:
        return self._async.auction

    def list_names(self, owners: Sequence[str]) -> pd.DataFrame:
        return self._runner.run(self._async.list_names(owners))

    def ownership_distribution(self, owners: Sequence[str]) -> pd.DataFrame:
        return self._runner.run(self._async.ownership_distribution(owners))

    def name_price_analysis(self, quotes: Sequence[RegistrationQuote]) -> pd.DataFrame:
        return self._runner.run(self._async.name_price_analysis(quotes))

    def close(self) -> None:
        self._runner.close()

    def __enter__(self) -> "XlmNsClient":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

