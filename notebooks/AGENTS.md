# AGENTS Instructions

All notebooks in this folder are [marimo](https://docs.marimo.io/) notebooks. Look at existing notebooks to get a sense of how they should be made.

## Checks

After modifying a notebook, run the following

* `uvx marimo check notebook_name.py`

## Current status

The notebooks in this folder still target the pre-SQLite storage layout. They will not work
correctly until they are migrated from DuckDB and Parquet inputs to `data/heisenbase.db`.
