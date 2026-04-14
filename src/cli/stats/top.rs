use anyhow::Result;
use clap::{Args, ValueEnum};
use heisenbase::storage;
use rusqlite::params;

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum TopMetric {
    /// Rank by utility divided by material_key_size.
    UtilityRank,
    /// Rank by raw PGN utility mass.
    Utility,
    /// Rank by number of indexed games.
    NumGames,
    /// Rank by number of indexed positions.
    NumPositions,
}

#[derive(Args)]
#[command(
    about = "Show the top PGN-indexed material keys",
    long_about = "Show the top PGN-indexed material keys ranked by utility, utility rank, game count, or indexed position count."
)]
pub(crate) struct TopArgs {
    /// Which column to rank by.
    #[arg(long, value_enum, default_value_t = TopMetric::UtilityRank)]
    by: TopMetric,
    /// Maximum number of rows to print.
    #[arg(long, default_value_t = 20)]
    limit: usize,
    /// Minimum number of PGN games required.
    #[arg(long, default_value_t = 1)]
    min_games: u64,
    /// Maximum total number of pieces allowed.
    #[arg(long)]
    max_pieces: Option<u32>,
}

pub(crate) fn run(args: TopArgs) -> Result<()> {
    let conn = storage::open_database()?;
    let order_by = match args.by {
        TopMetric::UtilityRank => "utility_rank DESC, utility DESC, material_key ASC",
        TopMetric::Utility => "utility DESC, utility_rank DESC, material_key ASC",
        TopMetric::NumGames => "num_games DESC, utility_rank DESC, material_key ASC",
        TopMetric::NumPositions => "num_positions DESC, utility_rank DESC, material_key ASC",
    };
    let sql = format!(
        "SELECT
            p.material_key,
            EXISTS (
                SELECT 1
                FROM wdl_tables w
                WHERE w.material_key = p.material_key
            ) AS has_table,
            p.num_games,
            p.num_positions,
            p.total_positions,
            p.material_key_size,
            p.num_pieces,
            p.utility,
            p.utility / CAST(p.material_key_size AS REAL) AS utility_rank
         FROM pgn_index p
         WHERE p.num_games >= ?1
           AND (?2 IS NULL OR p.num_pieces <= ?2)
         ORDER BY {order_by}
         LIMIT ?3"
    );

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![
            args.min_games as i64,
            args.max_pieces.map(i64::from),
            args.limit as i64
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, bool>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, f64>(7)?,
                row.get::<_, f64>(8)?,
            ))
        },
    )?;

    println!(
        "rank_by: {}",
        match args.by {
            TopMetric::UtilityRank => "utility_rank",
            TopMetric::Utility => "utility",
            TopMetric::NumGames => "num_games",
            TopMetric::NumPositions => "num_positions",
        }
    );
    println!("limit: {}", args.limit);
    println!("min_games: {}", args.min_games);
    println!(
        "max_pieces: {}",
        args.max_pieces
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    );
    println!(
        "material_key\thas_table\tnum_games\tnum_positions\ttotal_positions\tmaterial_key_size\tnum_pieces\tutility\tutility_rank"
    );

    for row in rows {
        let (
            material_key,
            has_table,
            num_games,
            num_positions,
            total_positions,
            material_key_size,
            num_pieces,
            utility,
            utility_rank,
        ) = row?;
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            material_key,
            has_table,
            num_games,
            num_positions,
            total_positions,
            material_key_size,
            num_pieces,
            format_float(utility),
            format_float(utility_rank)
        );
    }

    Ok(())
}

fn format_float(value: f64) -> String {
    format!("{:.12e}", value)
}
