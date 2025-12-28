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
    return alt, df


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


if __name__ == "__main__":
    app.run()
