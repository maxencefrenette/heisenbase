#!/usr/bin/env -S uv run --script
"""
Download all 3-man and 4-man Syzygy WDL tablebases from the Lichess mirror.

Run with `uv run scripts/download_syzygy.py`
"""
from __future__ import annotations

import sys
import time
import urllib.parse
import urllib.request
from collections import Counter
from dataclasses import dataclass
from html.parser import HTMLParser
from pathlib import Path
from typing import Iterable, Sequence

# Configuration
BASE_URL = "https://tablebase.lichess.ovh/tables/standard/3-4-5-wdl/"
DEST_DIR = Path(__file__).resolve().parents[1] / "syzygy"
CHUNK_SIZE = 1 << 20  # 1 MiB chunks
OVERWRITE = True
DRY_RUN = False
USER_AGENT = "heisenbase-syzygy-downloader/1.0 (+https://lichess.org)"
PIECE_LETTERS = frozenset("KPNBRQ")


class _IndexParser(HTMLParser):
    """Collect all href values that point at *.rtbw files."""

    def __init__(self) -> None:
        super().__init__()
        self.links: list[str] = []

    def handle_starttag(self, tag: str, attrs: Sequence[tuple[str, str]]) -> None:
        if tag != "a":
            return
        for key, value in attrs:
            if key == "href" and value.endswith(".rtbw"):
                self.links.append(value)
                break


@dataclass(frozen=True)
class TableEntry:
    name: str
    piece_count: int

    @property
    def url(self) -> str:
        return urllib.parse.urljoin(BASE_URL, self.name)


def fetch_table_index() -> Iterable[str]:
    request = urllib.request.Request(
        BASE_URL,
        headers={"User-Agent": USER_AGENT},
    )
    with urllib.request.urlopen(request) as response:
        charset = response.headers.get_content_charset("utf-8")
        html = response.read().decode(charset, errors="replace")
    parser = _IndexParser()
    parser.feed(html)
    return parser.links


def classify_tables(names: Iterable[str]) -> list[TableEntry]:
    entries: list[TableEntry] = []
    for name in names:
        piece_count = sum(1 for char in Path(name).stem if char in PIECE_LETTERS)
        if piece_count in {3, 4}:
            entries.append(TableEntry(name=name, piece_count=piece_count))
    entries.sort(key=lambda entry: (entry.piece_count, entry.name))
    return entries


def format_bytes(size: int | float) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    value = float(size)
    for unit in units:
        if value < 1024 or unit == units[-1]:
            if unit == "B":
                return f"{int(value)} {unit}"
            return f"{value:.2f} {unit}"
        value /= 1024
    return f"{value:.2f} TB"


def download_table(
    entry: TableEntry,
    dest_dir: Path,
    chunk_size: int,
    overwrite: bool,
) -> tuple[str, str]:
    dest_path = dest_dir / entry.name
    if dest_path.exists() and not overwrite:
        return entry.name, "skipped (already exists)"

    tmp_path = dest_path.with_suffix(dest_path.suffix + ".part")
    if tmp_path.exists():
        tmp_path.unlink()

    request = urllib.request.Request(entry.url, headers={"User-Agent": USER_AGENT})
    start = time.monotonic()
    bytes_written = 0
    with urllib.request.urlopen(request) as response:
        with open(tmp_path, "wb") as fh:
            while True:
                chunk = response.read(chunk_size)
                if not chunk:
                    break
                fh.write(chunk)
                bytes_written += len(chunk)

    tmp_path.replace(dest_path)
    elapsed = time.monotonic() - start
    rate = bytes_written / elapsed if elapsed > 0 else 0
    return (
        entry.name,
        f"downloaded {format_bytes(bytes_written)} in {elapsed:.1f}s ({format_bytes(rate)}/s)",
    )


def main() -> int:
    table_names = fetch_table_index()

    tables = classify_tables(table_names)
    if not tables:
        print("No 3-man or 4-man tables found in the index.", file=sys.stderr)
        return 1

    counts = Counter(entry.piece_count for entry in tables)
    dest_dir = DEST_DIR.expanduser().resolve()
    dest_dir.mkdir(parents=True, exist_ok=True)

    print(f"Destination: {dest_dir}")
    print(
        f"Found {counts.get(3, 0)} 3-man tables and {counts.get(4, 0)} 4-man tables at {BASE_URL}"
    )

    if DRY_RUN:
        for entry in tables:
            print(f"{entry.piece_count}-man {entry.name}")
        return 0

    total = len(tables)
    for index, entry in enumerate(tables, start=1):
        print(
            f"[{index}/{total}] {entry.piece_count}-man {entry.name} ... ",
            end="",
            flush=True,
        )
        _, message = download_table(
            entry=entry,
            dest_dir=dest_dir,
            chunk_size=CHUNK_SIZE,
            overwrite=OVERWRITE,
        )
        print(message)

    print("\nAll requested tables are present.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
