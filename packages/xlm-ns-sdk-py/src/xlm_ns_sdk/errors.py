from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum


class ContractErrorCode(IntEnum):
    NAME_NOT_FOUND = 1
    NOT_OWNER = 2
    EXPIRED = 3
    INVALID_LABEL = 4
    OTHER = 99


@dataclass(slots=False)
class ContractError(Exception):
    code: ContractErrorCode
    message: str = ""

    def __str__(self) -> str:
        suffix = f": {self.message}" if self.message else ""
        return f"contract error {self.code.name.lower()}{suffix}"


class SdkError(Exception):
    pass


class ValidationError(SdkError):
    pass


class BackendError(SdkError):
    pass

