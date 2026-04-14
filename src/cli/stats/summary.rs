use std::fs;

use anyhow::Result;
use clap::Args;
use heisenbase::storage;

#[derive(Args)]
#[command(
    about = "Show a compact database summary",
    long_about = "Show a compact database summary including table counts, database size, total stored positions, and aggregate WDL bucket counts."
)]
pub(crate) struct SummaryArgs {}

pub(crate) fn run(_: SummaryArgs) -> Result<()> {
    let conn = storage::open_database()?;
    let db_size_bytes = fs::metadata(storage::DB_PATH)
        .map(|metadata| metadata.len())
        .unwrap_or(0);

    let (
        wdl_tables,
        material_keys,
        pgn_index,
        pgn_index_raw,
        total_positions,
        illegal,
        win,
        draw,
        loss,
        win_or_draw,
        draw_or_loss,
        unknown,
    ): (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64) = conn.query_row(
        "SELECT
            (SELECT COUNT(*) FROM wdl_tables),
            (SELECT COUNT(*) FROM material_keys),
            (SELECT COUNT(*) FROM pgn_index),
            (SELECT COUNT(*) FROM pgn_index_raw),
            COALESCE(SUM(total), 0),
            COALESCE(SUM(illegal), 0),
            COALESCE(SUM(win), 0),
            COALESCE(SUM(draw), 0),
            COALESCE(SUM(loss), 0),
            COALESCE(SUM(win_or_draw), 0),
            COALESCE(SUM(draw_or_loss), 0),
            COALESCE(SUM(unknown), 0)
         FROM material_keys",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
                row.get(11)?,
            ))
        },
    )?;

    let solved_positions = win + draw + loss;
    let partially_solved_positions = solved_positions + win_or_draw + draw_or_loss;

    println!("db_path: {}", storage::DB_PATH);
    println!("db_size_bytes: {}", db_size_bytes);
    println!("wdl_tables: {}", wdl_tables);
    println!("material_keys: {}", material_keys);
    println!("pgn_index: {}", pgn_index);
    println!("pgn_index_raw: {}", pgn_index_raw);
    println!("tablebase.total_positions: {}", total_positions);
    println!("tablebase.illegal_positions: {}", illegal);
    println!("tablebase.solved_positions: {}", solved_positions);
    println!(
        "tablebase.partially_solved_positions: {}",
        partially_solved_positions
    );
    println!("tablebase.unknown_positions: {}", unknown);
    println!("tablebase.win_positions: {}", win);
    println!("tablebase.draw_positions: {}", draw);
    println!("tablebase.loss_positions: {}", loss);
    println!("tablebase.win_or_draw_positions: {}", win_or_draw);
    println!("tablebase.draw_or_loss_positions: {}", draw_or_loss);
    println!(
        "tablebase.solved_fraction: {}",
        format_ratio(solved_positions, total_positions)
    );
    println!(
        "tablebase.partially_solved_fraction: {}",
        format_ratio(partially_solved_positions, total_positions)
    );
    println!(
        "tablebase.unknown_fraction: {}",
        format_ratio(unknown, total_positions)
    );
    println!(
        "tablebase.illegal_fraction: {}",
        format_ratio(illegal, total_positions)
    );

    Ok(())
}

fn format_ratio(numerator: i64, denominator: i64) -> String {
    if denominator == 0 {
        return "0.000000".to_string();
    }
    format!("{:.6}", numerator as f64 / denominator as f64)
}
