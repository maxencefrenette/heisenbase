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
    import altair as alt

    df = pd.read_parquet("../data/pgn_index.parquet")
    df = df[df["num_games"] > 1]
    df["utility"] = 1_000_000_000 * df["num_positions"] / df["total_positions"] / df["material_key_size"]
    df = df.sort_values("utility", ascending=False)
    df["cumulative_positions"] = (df["num_positions"] / df["total_positions"]).cumsum()
    df["cumulative_material_key_size"] = df["material_key_size"].cumsum()
    df.reset_index(drop=True, inplace=True)

    mo.md(f"Number of indexed material keys: {len(df):,}")
    return alt, df, pd


@app.cell
def _(df):
    df.head(10_000)
    return


@app.cell
def _(df):
    df[df["material_key_size"] > 8192].head(10_000)
    return


@app.cell
def _(alt, df):
    def chart1(max_cumulative_material_key_size):
        df2 = df[df["cumulative_material_key_size"] < max_cumulative_material_key_size]

        if len(df2) > 1000:
            step = len(df2) // 1000
            df2 = df2.iloc[::step]

        return alt.Chart(df2).mark_line().encode(
            x='cumulative_material_key_size',
            y='cumulative_positions',
        )
    return (chart1,)


@app.cell
def _(chart1):
    chart1(1e9)
    return


@app.cell
def _(chart1):
    chart1(1e12)
    return


@app.cell
def _(chart1):
    chart1(1e15)
    return


@app.cell
def _(alt, df, pd):
    import numpy as np

    def downsample_equal_per_logbin(df, x, n_out, nbins=50, base=10, random_state=0):
        d = df[df[x] > 0].copy()
        logx = np.log(d[x].to_numpy()) / np.log(base)

        bins = np.linspace(logx.min(), logx.max(), nbins + 1)
        d["_bin"] = pd.cut(logx, bins=bins, include_lowest=True)

        k = max(1, int(np.ceil(n_out / nbins)))

        # shuffle once, then take first k per bin
        d = d.sample(frac=1, random_state=random_state)
        out = d.groupby("_bin", observed=True).head(k).drop(columns="_bin")

        # trim if overshot
        if len(out) > n_out:
            out = out.sample(n_out, random_state=random_state)
        return out

    alt.Chart(downsample_equal_per_logbin(df, "cumulative_material_key_size", 1000, nbins=100)).mark_line().encode(
        x=alt.X('cumulative_material_key_size', scale=alt.Scale(type='log')),
        y=alt.Y('cumulative_positions', scale=alt.Scale(type='log')),
    )
    return


if __name__ == "__main__":
    app.run()
