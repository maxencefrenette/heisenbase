# /// script
# requires-python = ">=3.13"
# dependencies = [
#     "altair",
#     "duckdb",
#     "marimo>=0.17.0",
#     "pandas",
#     "pyzmq",
# ]
# ///

import marimo

__generated_with = "0.18.4"
app = marimo.App()


@app.cell
def _():
    from pathlib import Path
    import duckdb
    import pandas as pd
    import altair as alt
    import marimo as mo

    db_path = (
        Path(__file__) / ".." / ".." / "data" / "heisenbase" / "index.duckdb"
    ).resolve()
    con = duckdb.connect(str(db_path), read_only=True)

    df = con.execute("select * from material_keys").fetchdf()
    df
    return alt, df, mo, pd


@app.cell
def _(mo):
    mo.md(r"""
    # Distribution of WDL values
    """)
    return


@app.cell
def _(df):
    counts = df.sum(numeric_only=True)
    counts
    return (counts,)


@app.cell
def _(alt, counts, pd):
    _df = pd.DataFrame([
        { "x": "win", "y": counts["win"]},
        { "x": "draw", "y": counts["draw"]},
        { "x": "loss", "y": counts["loss"]},
        { "x": "win_or_draw", "y": counts["win_or_draw"]},
        { "x": "draw_or_loss", "y": counts["draw_or_loss"]},
        { "x": "unknown", "y": counts["unknown"]},
    ])

    _chart = (
        alt.Chart(_df)
            .mark_bar()
            .encode(
                x=alt.X(field='x', type='nominal', sort=None),
                y=alt.Y(field='y', type='quantitative'),
                tooltip=[
                    alt.Tooltip(field='x'),
                    alt.Tooltip(field='y', format=',.0f')
                ]
            )
            .properties(
                config={
                    'axis': {
                        'grid': False
                    }
                }
            )
    )
    _chart
    return


if __name__ == "__main__":
    app.run()
