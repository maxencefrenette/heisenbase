use anyhow::{Result, anyhow};
use clap::Args;
use heisenbase::material_key::MaterialKey;
use heisenbase::storage::Database;
use rusqlite::{OptionalExtension, params};

#[derive(Args)]
#[command(
    about = "Show detailed stats for one material key",
    long_about = "Show detailed stats for one material key, including whether a WDL table exists, the material_keys row, the pgn_index row, child dependencies, and stale status."
)]
pub(crate) struct MaterialArgs {
    /// Material key describing pieces, e.g. `KQvK`.
    material_key: String,
}

pub(crate) fn run(args: MaterialArgs) -> Result<()> {
    let material = MaterialKey::from_string(&args.material_key)
        .map_err(|err| anyhow!("invalid material key: {}: {}", args.material_key, err))?;
    let db = Database::open_default()?;
    let conn = db.conn();
    let material_key = material.to_string();

    let wdl_payload_bytes: Option<i64> = conn
        .query_row(
            "SELECT length(payload) FROM wdl_tables WHERE material_key = ?1",
            [&material_key],
            |row| row.get(0),
        )
        .optional()?;

    let material_row = db.get_material_stats(&material)?;
    let pgn_row = db.get_pgn_index_row(&material)?;

    println!("material_key: {}", material_key);
    println!("has_wdl_table: {}", wdl_payload_bytes.is_some());
    println!("wdl_payload_bytes: {}", wdl_payload_bytes.unwrap_or(0));
    println!("has_material_row: {}", material_row.is_some());
    println!("has_pgn_index_row: {}", pgn_row.is_some());

    if let Some(row) = material_row {
        let (is_stale, newest_solved_child_updated_at) =
            stale_status(conn, &row.children, row.updated_at)?;
        let solved_positions = row.win + row.draw + row.loss;
        let partially_solved_positions = solved_positions + row.win_or_draw + row.draw_or_loss;

        println!("material.name: {}", row.name);
        println!("material.children_count: {}", row.children.len());
        println!(
            "material.children: {}",
            if row.children.is_empty() {
                "(none)".to_string()
            } else {
                row.children.join(",")
            }
        );
        println!("material.num_pieces: {}", row.num_pieces);
        println!("material.num_pawns: {}", row.num_pawns);
        println!("material.num_non_pawns: {}", row.num_non_pawns);
        println!("material.total: {}", row.total);
        println!("material.illegal: {}", row.illegal);
        println!("material.solved: {}", solved_positions);
        println!("material.partially_solved: {}", partially_solved_positions);
        println!("material.unknown: {}", row.unknown);
        println!("material.win: {}", row.win);
        println!("material.draw: {}", row.draw);
        println!("material.loss: {}", row.loss);
        println!("material.win_or_draw: {}", row.win_or_draw);
        println!("material.draw_or_loss: {}", row.draw_or_loss);
        println!(
            "material.solved_fraction: {}",
            format_ratio(solved_positions, row.total)
        );
        println!(
            "material.partially_solved_fraction: {}",
            format_ratio(partially_solved_positions, row.total)
        );
        println!(
            "material.unknown_fraction: {}",
            format_ratio(row.unknown, row.total)
        );
        println!("material.updated_at: {}", row.updated_at);
        println!("material.is_stale: {}", is_stale);
        println!(
            "material.newest_solved_child_updated_at: {}",
            newest_solved_child_updated_at
        );
    }

    if let Some(row) = pgn_row {
        println!("pgn.material_key: {}", row.material_key);
        println!("pgn.num_games: {}", row.num_games);
        println!("pgn.num_positions: {}", row.num_positions);
        println!("pgn.total_games: {}", row.total_games);
        println!("pgn.total_positions: {}", row.total_positions);
        println!("pgn.material_key_size: {}", row.material_key_size);
        println!("pgn.num_pieces: {}", row.num_pieces);
        println!("pgn.num_pawns: {}", row.num_pawns);
        println!("pgn.num_non_pawns: {}", row.num_non_pawns);
        println!("pgn.utility: {}", format_float(row.utility));
        println!(
            "pgn.utility_rank: {}",
            format_float(row.utility / row.material_key_size as f64)
        );
    }

    Ok(())
}

fn stale_status(
    conn: &rusqlite::Connection,
    children: &[String],
    updated_at: i64,
) -> Result<(bool, i64)> {
    let mut newest_solved_child_updated_at = 0;
    let mut stmt = conn.prepare("SELECT updated_at FROM material_keys WHERE name = ?1")?;
    for child in children {
        let child_updated_at: Option<i64> = stmt
            .query_row(params![child], |row| row.get(0))
            .optional()?;
        if let Some(child_updated_at) = child_updated_at {
            newest_solved_child_updated_at = newest_solved_child_updated_at.max(child_updated_at);
        }
    }
    Ok((
        newest_solved_child_updated_at > updated_at,
        newest_solved_child_updated_at,
    ))
}

fn format_ratio(numerator: i64, denominator: i64) -> String {
    if denominator == 0 {
        return "0.000000".to_string();
    }
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn format_float(value: f64) -> String {
    format!("{:.12e}", value)
}
