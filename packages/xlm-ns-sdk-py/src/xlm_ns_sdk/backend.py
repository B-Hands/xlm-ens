from __future__ import annotations

import asyncio
from copy import deepcopy
from dataclasses import dataclass
from inspect import isawaitable
from typing import Any, Callable, Dict, Mapping, MutableMapping, Optional, Protocol, Tuple

from .errors import BackendError


@dataclass(frozen=True)
class ContractInvocation:
    contract_name: str
    contract_id: Optional[str]
    method: str
    arguments: Mapping[str, Any]
    mode: str = "read"


class AsyncContractBackend(Protocol):
    async def call(self, invocation: ContractInvocation) -> Any: ...


class StaticContractBackend:
    def __init__(self, responses: Optional[Mapping[Tuple[str, str], Any]] = None) -> None:
        self._responses: Dict[Tuple[str, str], Any] = dict(responses or {})

    def set_response(self, contract_name: str, method: str, value: Any) -> None:
        self._responses[(contract_name, method)] = value

    async def call(self, invocation: ContractInvocation) -> Any:
        key = (invocation.contract_name, invocation.method)
        if key not in self._responses:
            raise BackendError(f"no static response configured for {invocation.contract_name}.{invocation.method}")

        value = self._responses[key]
        if callable(value):
            result = value(invocation)
            if isawaitable(result):
                return await result
            return result
        return deepcopy(value)


async def gather_callables(items: Mapping[str, Callable[[], Any]]) -> Dict[str, Any]:
    results: Dict[str, Any] = {}
    for key, factory in items.items():
        value = factory()
        if isawaitable(value):
            value = await value
        results[key] = value
    return results

