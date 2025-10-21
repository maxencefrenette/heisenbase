#!/usr/bin/env -S uv run --script
from __future__ import annotations

from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
HEISEN = ROOT / "data"
SYZYGY = ROOT / "syzygy"

COMPARISONS = [
    ("KQvK", "KQvK.hbt", "KQvK.rtbw"),
    ("KRvK", "KRvK.hbt", "KRvK.rtbw"),
    ("KPvK", "KPvK.hbt", "KPvK.rtbw"),
    ("KBlvK", "KBdvK.hbt", "KBvK.rtbw"),
    ("KNvK", "KNvK.hbt", "KNvK.rtbw"),
]


def pretty(size: int) -> str:
    return f"{size / (1 << 20):.3f} MiB"


for label, h_name, s_name in COMPARISONS:
    h_size = (HEISEN / h_name).stat().st_size
    s_size = (SYZYGY / s_name).stat().st_size
    ratio = h_size / s_size if s_size else float("inf")
    print(
        f"{label}: heisenbase {pretty(h_size)} vs syzygy {pretty(s_size)} "
        f"(ratio {ratio:.2f})"
    )
