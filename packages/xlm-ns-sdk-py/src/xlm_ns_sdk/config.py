from __future__ import annotations

from dataclasses import dataclass, field
from datetime import timedelta
from enum import Enum


def default_user_agent() -> str:
    return "xlm-ns-sdk/0.1.0"


@dataclass
class RetryConfig:
    max_retries: int = 3
    initial_backoff: timedelta = timedelta(seconds=1)
    max_backoff: timedelta = timedelta(seconds=30)
    jitter: bool = True


@dataclass
class ClientConfig:
    timeout: timedelta = timedelta(seconds=30)
    retry: RetryConfig = field(default_factory=RetryConfig)
    user_agent: str = field(default_factory=default_user_agent)
    poll_final_status: bool = True
    transaction_poll_timeout: timedelta = timedelta(seconds=60)


class NetworkPreset(Enum):
    TESTNET = ("https://soroban-testnet.stellar.org", "Test SDF Network ; September 2015")
    MAINNET = ("https://soroban.stellar.org", "Public Global Stellar Network ; September 2015")

    @property
    def rpc_url(self) -> str:
        return self.value[0]

    @property
    def passphrase(self) -> str:
        return self.value[1]

