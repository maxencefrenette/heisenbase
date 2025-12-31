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


@app.cell
def _(alt, df, mo):
    _df = (
        df
            .groupby("num_pieces")
            .agg(
                win = ("win", "sum"),
                draw = ("draw", "sum"),
                loss = ("loss", "sum"),
                win_or_draw = ("win_or_draw", "sum"),
                draw_or_loss = ("draw_or_loss", "sum"),
            )
            .reset_index()
    )
    _df["solved_positions"] = _df["win"] + _df["draw"] + _df["loss"]
    _df["partially_solved_positions"] = _df["solved_positions"] + _df["win_or_draw"] + _df["draw_or_loss"]


    mo.output.append(
        alt.Chart(_df, title=f"Solved positions by piece count in current tablebase")
            .mark_bar()
            .encode(
                x=alt.X('num_pieces:N'),
                y=alt.Y('solved_positions'),
            )
    )

    mo.output.append(
        alt.Chart(_df, title=f"Partially solved positions by piece count in current tablebase")
            .mark_bar()
            .encode(
                x=alt.X('num_pieces:N'),
                y=alt.Y('partially_solved_positions'),
            )
    )

    return


if __name__ == "__main__":
    app.run()
