# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "marimo>=0.17.0",
#     "pandas",
#     "pyarrow",
#     "pyzmq",
# ]
# ///

import marimo

__generated_with = "0.18.4"
app = marimo.App()


@app.cell
def _():
    import pandas as pd
    import marimo as mo

    df = pd.read_parquet("../data/pgn_index.parquet")
    df["usefulness"] = 1000 * df["num_games"] / df["total_positions"]
    df = df.sort_values("usefulness", ascending=False)
    df.reset_index(drop=True, inplace=True)

    mo.md(f"Total number of material keys: {len(df):,}")
    return (df,)


@app.cell
def _(df):
    df.head(10_000)
    return


@app.cell
def _(df):
    df[df["total_positions"] > 8192].head(10_000)
    return


if __name__ == "__main__":
    app.run()
