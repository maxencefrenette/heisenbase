use anyhow::{Context, Result};
use clap::Args;
use heisenbase::storage::Database;
use polars::prelude::*;

#[derive(Args)]
#[command(
    about = "Show piece-count distribution under a size budget",
    long_about = "Simulate a tablebase built in direct utility-rank order and summarize its material keys, positions, and indexed PGN position coverage by piece count."
)]
pub(crate) struct PieceHistogramArgs {
    /// Maximum number of tablebase positions in the simulated tablebase.
    #[arg(long)]
    max_positions: u64,
    /// Minimum number of indexed PGN games required.
    #[arg(long, default_value_t = 1)]
    min_games: u64,
    /// Maximum total number of pieces allowed.
    #[arg(long)]
    max_pieces: Option<u32>,
}

pub(crate) fn run(args: PieceHistogramArgs) -> Result<()> {
    let max_positions = i64::try_from(args.max_positions)
        .context("--max-positions exceeds the supported SQLite integer range")?;
    let db = Database::open_default()?;
    let rows = build_piece_histogram(&db, max_positions, args.min_games, args.max_pieces)?;

    let selected_tables: i64 = rows.iter().map(|row| row.tables).sum();
    let selected_positions: i64 = rows.iter().map(|row| row.positions).sum();
    let pgn_position_fraction: f64 = rows.iter().map(|row| row.pgn_position_fraction).sum();

    println!("rank_by: utility_rank");
    println!("max_positions: {}", args.max_positions);
    println!("min_games: {}", args.min_games);
    println!(
        "max_pieces: {}",
        args.max_pieces
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!("selected_tables: {}", selected_tables);
    println!("selected_positions: {}", selected_positions);
    println!(
        "pgn_position_fraction: {}",
        format_float(pgn_position_fraction)
    );
    println!(
        "num_pieces\ttables\tpositions\ttable_fraction\tposition_fraction\tpgn_position_fraction"
    );

    for row in rows {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            row.num_pieces,
            row.tables,
            row.positions,
            format_ratio(row.tables, selected_tables),
            format_ratio(row.positions, selected_positions),
            format_float(row.pgn_position_fraction),
        );
    }

    Ok(())
}

fn build_piece_histogram(
    db: &Database,
    max_positions: i64,
    min_games: u64,
    max_pieces: Option<u32>,
) -> Result<Vec<HistogramRow>> {
    let mut ranked = load_pgn_index_df(db)?.lazy().filter(
        col("num_games")
            .cast(DataType::Int64)
            .gt_eq(lit(min_games as i64)),
    );
    if let Some(max_pieces) = max_pieces {
        ranked = ranked.filter(
            col("num_pieces")
                .cast(DataType::Int64)
                .lt_eq(lit(max_pieces as i64)),
        );
    }

    let grouped = ranked
        .with_columns([
            (col("utility") / col("material_key_size").cast(DataType::Float64))
                .alias("utility_rank"),
        ])
        .sort(
            ["utility_rank", "utility", "material_key"],
            SortMultipleOptions::new().with_order_descending_multi([true, true, false]),
        )
        .with_columns([col("material_key_size")
            .cum_sum(false)
            .alias("cumulative_positions")])
        .filter(col("cumulative_positions").lt_eq(lit(max_positions)))
        .group_by([col("num_pieces")])
        .agg([
            col("material_key").count().alias("tables"),
            col("material_key_size").sum().alias("positions"),
            col("utility").sum().alias("pgn_position_fraction"),
        ])
        .sort(["num_pieces"], SortMultipleOptions::default())
        .collect()?;

    let num_pieces = grouped.column("num_pieces")?.i64()?;
    let tables = grouped.column("tables")?.u32()?;
    let positions = grouped.column("positions")?.i64()?;
    let pgn_position_fraction = grouped.column("pgn_position_fraction")?.f64()?;
    let mut rows = Vec::with_capacity(grouped.height());
    for index in 0..grouped.height() {
        rows.push(HistogramRow {
            num_pieces: num_pieces.get(index).unwrap(),
            tables: i64::from(tables.get(index).unwrap()),
            positions: positions.get(index).unwrap(),
            pgn_position_fraction: pgn_position_fraction.get(index).unwrap(),
        });
    }
    Ok(rows)
}

fn load_pgn_index_df(db: &Database) -> Result<DataFrame> {
    let mut stmt = db.conn().prepare(
        "SELECT material_key, num_games, num_pieces, material_key_size, utility
         FROM pgn_index",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, f64>(4)?,
        ))
    })?;

    let mut material_keys = Vec::new();
    let mut num_games = Vec::new();
    let mut num_pieces = Vec::new();
    let mut material_key_size = Vec::new();
    let mut utility = Vec::new();
    for row in rows {
        let (material_key, games, pieces, size, score) = row?;
        material_keys.push(material_key);
        num_games.push(games);
        num_pieces.push(pieces);
        material_key_size.push(size);
        utility.push(score);
    }

    Ok(DataFrame::new(vec![
        Series::new("material_key", material_keys),
        Series::new("num_games", num_games),
        Series::new("num_pieces", num_pieces),
        Series::new("material_key_size", material_key_size),
        Series::new("utility", utility),
    ])?)
}

struct HistogramRow {
    num_pieces: i64,
    tables: i64,
    positions: i64,
    pgn_position_fraction: f64,
}

fn format_ratio(numerator: i64, denominator: i64) -> String {
    if denominator == 0 {
        return "0.000000".to_string();
    }
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn format_float(value: f64) -> String {
    format!("{value:.12e}")
}
