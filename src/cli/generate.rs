use anyhow::Result;
use heisenbase::generation_targets::compute_generation_targets;
use heisenbase::material_key::MaterialKey;
use heisenbase::storage::{self, Database, MaterialStatsRow};
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_score_range::WdlScoreRange;
use heisenbase::wdl_table::WdlTable;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn run_generate(material: MaterialKey) -> Result<()> {
    let mut table_builder = TableBuilder::new(material);
    let loaded: Vec<String> = table_builder
        .loaded_child_materials()
        .iter()
        .map(|k| k.to_string())
        .collect();
    let missing: Vec<String> = table_builder
        .missing_child_materials()
        .iter()
        .map(|k| k.to_string())
        .collect();
    println!(
        "Loaded child tables: {}",
        if loaded.is_empty() {
            "(none)".to_string()
        } else {
            loaded.join(", ")
        }
    );
    println!(
        "Missing child tables: {}",
        if missing.is_empty() {
            "(none)".to_string()
        } else {
            missing.join(", ")
        }
    );
    table_builder.solve();
    let wdl_table: WdlTable = table_builder.into();
    let total = wdl_table.positions.len() as f64;
    let mut counts = [0usize; 7];
    for wdl in &wdl_table.positions {
        counts[*wdl as usize] += 1;
    }
    println!("WDL statistics:");
    for variant in [
        WdlScoreRange::Unknown,
        WdlScoreRange::WinOrDraw,
        WdlScoreRange::DrawOrLoss,
        WdlScoreRange::Win,
        WdlScoreRange::Draw,
        WdlScoreRange::Loss,
        WdlScoreRange::IllegalPosition,
    ] {
        let count = counts[variant as usize];
        let percentage = if total > 0.0 {
            (count as f64 / total) * 100.0
        } else {
            0.0
        };
        println!("{variant:?}: {percentage:.2}%");
    }

    let db = Database::open_default()?;
    db.put_wdl_table(&wdl_table)?;
    log_stats_to_index(&db, &wdl_table, &counts)?;
    println!(
        "Stored table in {} for {}",
        storage::DB_PATH,
        wdl_table.material
    );
    println!();
    Ok(())
}

fn log_stats_to_index(db: &Database, wdl_table: &WdlTable, counts: &[usize; 7]) -> Result<()> {
    let updated_at = current_timestamp()?;
    let mut children: Vec<String> = wdl_table
        .material
        .child_material_keys()
        .into_iter()
        .map(|key| key.to_string())
        .collect();
    children.sort();
    let row = MaterialStatsRow {
        name: wdl_table.material.to_string(),
        children,
        num_pieces: wdl_table.material.total_piece_count() as i64,
        num_pawns: wdl_table.material.pawns.pawn_count() as i64,
        num_non_pawns: wdl_table.material.non_pawn_piece_count() as i64,
        total: wdl_table.positions.len() as i64,
        illegal: counts[WdlScoreRange::IllegalPosition as usize] as i64,
        win: counts[WdlScoreRange::Win as usize] as i64,
        draw: counts[WdlScoreRange::Draw as usize] as i64,
        loss: counts[WdlScoreRange::Loss as usize] as i64,
        win_or_draw: counts[WdlScoreRange::WinOrDraw as usize] as i64,
        draw_or_loss: counts[WdlScoreRange::DrawOrLoss as usize] as i64,
        unknown: counts[WdlScoreRange::Unknown as usize] as i64,
        updated_at,
    };
    db.upsert_material_stats(&row)?;

    Ok(())
}

pub(crate) fn run_generate_many(min_games: u64, max_pieces: u32) -> Result<()> {
    let db = Database::open_default()?;

    loop {
        let Some(target) = compute_generation_targets(&db, min_games, max_pieces)?
            .into_iter()
            .next()
        else {
            println!(
                "No material keys matched filters (min-games: {}, max-pieces: {}).",
                min_games, max_pieces
            );
            break;
        };
        if target.is_stale {
            println!("Regenerating stale {}", target.material_key);
        } else {
            println!("Generating {}", target.material_key);
        }
        run_generate(target.material_key)?;
    }

    Ok(())
}

fn current_timestamp() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}
