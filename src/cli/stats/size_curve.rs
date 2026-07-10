use anyhow::{Result, bail};
use clap::Args;
use heisenbase::storage::Database;
use polars::prelude::*;

#[derive(Args)]
#[command(
    about = "Show PGN coverage versus tablebase size",
    long_about = "Simulate a tablebase built in direct utility-rank order and show cumulative indexed PGN position coverage at logarithmically spaced size budgets."
)]
pub(crate) struct SizeCurveArgs {
    /// Number of logarithmically spaced checkpoints to print.
    #[arg(long, default_value_t = 30)]
    points: usize,
    /// Stop the simulated tablebase at this many positions.
    #[arg(long)]
    max_positions: Option<u64>,
    /// Minimum number of indexed PGN games required.
    #[arg(long, default_value_t = 1)]
    min_games: u64,
    /// Maximum total number of pieces allowed.
    #[arg(long)]
    max_pieces: Option<u32>,
}

pub(crate) fn run(args: SizeCurveArgs) -> Result<()> {
    if args.points < 2 {
        bail!("--points must be at least 2");
    }

    let db = Database::open_default()?;
    let frame = build_size_curve(&db, args.min_games, args.max_pieces)?;
    let points = sample_curve(&frame, args.points, args.max_positions)?;

    println!("rank_by: utility_rank");
    println!("points: {}", args.points);
    println!("min_games: {}", args.min_games);
    println!(
        "max_pieces: {}",
        args.max_pieces
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!(
        "max_positions: {}",
        args.max_positions
            .map(|value| value.to_string())
            .unwrap_or_else(|| "all".to_string())
    );
    println!(
        "budget_positions\tselected_tables\tselected_positions\tpgn_position_fraction\tlast_material_key"
    );

    for point in points {
        println!(
            "{}\t{}\t{}\t{}\t{}",
            point.budget_positions,
            point.selected_tables,
            point.selected_positions,
            format_float(point.pgn_position_fraction),
            point.last_material_key,
        );
    }

    Ok(())
}

fn build_size_curve(db: &Database, min_games: u64, max_pieces: Option<u32>) -> Result<DataFrame> {
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

    Ok(ranked
        .with_columns([
            (col("utility") / col("material_key_size").cast(DataType::Float64))
                .alias("utility_rank"),
        ])
        .sort(
            ["utility_rank", "utility", "material_key"],
            SortMultipleOptions::new().with_order_descending_multi([true, true, false]),
        )
        .with_columns([
            col("material_key_size")
                .cum_sum(false)
                .alias("cumulative_positions"),
            col("utility")
                .cum_sum(false)
                .alias("cumulative_pgn_position_fraction"),
        ])
        .select([
            col("material_key"),
            col("cumulative_positions"),
            col("cumulative_pgn_position_fraction"),
        ])
        .collect()?)
}

fn sample_curve(
    frame: &DataFrame,
    point_count: usize,
    max_positions: Option<u64>,
) -> Result<Vec<CurvePoint>> {
    if frame.is_empty() {
        return Ok(Vec::new());
    }

    let cumulative_positions: Vec<i64> = frame
        .column("cumulative_positions")?
        .i64()?
        .into_no_null_iter()
        .collect();
    let cumulative_pgn_fraction: Vec<f64> = frame
        .column("cumulative_pgn_position_fraction")?
        .f64()?
        .into_no_null_iter()
        .collect();
    let material_keys: Vec<&str> = frame
        .column("material_key")?
        .str()?
        .into_no_null_iter()
        .collect();

    let available_max = *cumulative_positions.last().unwrap();
    let requested_max = max_positions
        .map(i64::try_from)
        .transpose()?
        .unwrap_or(available_max);
    let max_budget = requested_max.min(available_max);
    let min_budget = cumulative_positions[0];
    if max_budget < min_budget {
        return Ok(Vec::new());
    }

    let mut points = Vec::new();
    let mut previous_index = None;
    for budget in logarithmic_budgets(min_budget, max_budget, point_count) {
        let selected_tables =
            cumulative_positions.partition_point(|positions| *positions <= budget);
        if selected_tables == 0 {
            continue;
        }
        let index = selected_tables - 1;
        if previous_index == Some(index) {
            continue;
        }
        previous_index = Some(index);
        points.push(CurvePoint {
            budget_positions: budget,
            selected_tables,
            selected_positions: cumulative_positions[index],
            pgn_position_fraction: cumulative_pgn_fraction[index],
            last_material_key: material_keys[index].to_string(),
        });
    }
    Ok(points)
}

fn logarithmic_budgets(min: i64, max: i64, count: usize) -> Vec<i64> {
    if min == max {
        return vec![max];
    }
    let log_min = (min as f64).ln();
    let log_max = (max as f64).ln();
    let mut budgets = Vec::with_capacity(count);
    for index in 0..count {
        let fraction = index as f64 / (count - 1) as f64;
        budgets.push((log_min + fraction * (log_max - log_min)).exp().round() as i64);
    }
    budgets[0] = min;
    budgets[count - 1] = max;
    budgets.dedup();
    budgets
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

struct CurvePoint {
    budget_positions: i64,
    selected_tables: usize,
    selected_positions: i64,
    pgn_position_fraction: f64,
    last_material_key: String,
}

fn format_float(value: f64) -> String {
    format!("{value:.12e}")
}

#[cfg(test)]
mod tests {
    use super::logarithmic_budgets;

    #[test]
    fn logarithmic_budgets_include_endpoints() {
        let budgets = logarithmic_budgets(100, 1_000_000, 5);
        assert_eq!(budgets.first(), Some(&100));
        assert_eq!(budgets.last(), Some(&1_000_000));
        assert!(budgets.windows(2).all(|pair| pair[0] < pair[1]));
    }
}
