#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "huggingface-hub",
# ]
# ///
from pathlib import Path
from huggingface_hub import snapshot_download

snapshot_download(
    repo_id="official-stockfish/fishtest_pgns",
    repo_type="dataset",
    allow_patterns="25-09-*/*/*.pgn.gz",
    local_dir=Path(__file__).resolve().parents[1] / "data" / "fishtest_pgns",
)
