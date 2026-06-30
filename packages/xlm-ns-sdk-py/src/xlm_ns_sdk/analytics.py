from __future__ import annotations

from collections import Counter
from dataclasses import asdict
from typing import Iterable, Sequence

import pandas as pd

from .models import NameRecord, RegistrationQuote, to_frame


def list_names_frame(records: Sequence[NameRecord]) -> pd.DataFrame:
    return to_frame(records)


def ownership_distribution(records: Sequence[NameRecord]) -> pd.DataFrame:
    counts = Counter(record.owner for record in records)
    frame = pd.DataFrame(sorted(counts.items()), columns=["owner", "name_count"])
    return frame.sort_values("name_count", ascending=False).reset_index(drop=True)


def name_price_analysis(quotes: Sequence[RegistrationQuote]) -> pd.DataFrame:
    rows = []
    for quote in quotes:
        rows.append(
            {
                "label": quote.label,
                "duration_years": quote.duration_years,
                "base_fee": quote.fee_breakdown.base_fee,
                "premium_fee": quote.fee_breakdown.premium_fee,
                "network_fee": quote.fee_breakdown.network_fee,
                "total_fee": quote.total_fee,
                "fee_currency": quote.fee_currency,
                "expires_at": quote.expires_at,
            }
        )
    frame = pd.DataFrame(rows)
    if not frame.empty:
        frame["effective_per_year"] = frame["total_fee"] / frame["duration_years"]
    return frame

