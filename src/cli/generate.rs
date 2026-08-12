use anyhow::Result;
use heisenbase::generation_targets::{alternate_generation_targets, compute_generation_targets};
use heisenbase::material_key::MaterialKey;
use heisenbase::storage::{self, Database, MaterialStatsRow};
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_score_range::WdlScoreRange;
use heisenbase::wdl_table::WdlTable;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn run_generate(material: MaterialKey) -> Result<()> {
    let db = Database::open_default()?;
    run_generate_with_db(material, &db).map(|_| ())
}

fn run_generate_with_db(material: MaterialKey, db: &Database) -> Result<bool> {
    let previous_table = db.get_wdl_table(&material)?;
    let previous_updated_at = db
        .get_material_stats(&material)?
        .map(|stats| stats.updated_at);
    let mut table_builder = TableBuilder::new(material, db)?;
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

    let table_changed = previous_table.as_ref() != Some(&wdl_table);
    let updated_at = if table_changed {
        current_timestamp()?
    } else if let Some(previous_updated_at) = previous_updated_at {
        previous_updated_at
    } else {
        current_timestamp()?
    };

    if table_changed {
        db.put_wdl_table(&wdl_table)?;
    }
    log_stats_to_index(db, &wdl_table, &counts, updated_at)?;
    if table_changed {
        println!(
            "Stored changed table in {} for {}",
            storage::DB_PATH,
            wdl_table.material
        );
    } else {
        println!("Table unchanged for {}", wdl_table.material);
    }
    println!();
    Ok(table_changed)
}

fn log_stats_to_index(
    db: &Database,
    wdl_table: &WdlTable,
    counts: &[usize; 7],
    updated_at: i64,
) -> Result<()> {
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

pub(crate) fn run_generate_many(
    min_games: u64,
    max_pieces: u32,
    target_positions: Option<u64>,
) -> Result<()> {
    let db = Database::open_default()?;
    let mut validated_unchanged = HashMap::new();
    let mut prefer_stale = false;

    loop {
        let total_positions = db.total_table_positions()?;
        let remaining_positions = match target_positions {
            Some(target) if total_positions >= target => {
                println!(
                    "Reached target: {} positions stored (target: {}).",
                    total_positions, target
                );
                break;
            }
            Some(target) => Some(target - total_positions),
            None => None,
        };
        let unvalidated_targets: Vec<_> = compute_generation_targets(&db, min_games, max_pieces)?
            .into_iter()
            .filter(|target| {
                validated_unchanged
                    .get(&target.material_key)
                    .is_none_or(|validated_at| target.max_child_updated_at > *validated_at)
            })
            .collect();
        let new_table_budget = remaining_positions.map(|remaining| {
            let smallest_new_table = unvalidated_targets
                .iter()
                .filter(|target| !target.is_stale)
                .filter_map(|target| u64::try_from(target.material_key_size).ok())
                .min();
            effective_new_table_budget(remaining, smallest_new_table)
        });
        let eligible_targets = unvalidated_targets
            .into_iter()
            .filter(|target| {
                target.is_stale
                    || new_table_budget.is_none_or(|budget| {
                        u64::try_from(target.material_key_size)
                            .is_ok_and(|table_size| table_size <= budget)
                    })
            })
            .collect();
        let Some(target) = alternate_generation_targets(eligible_targets, prefer_stale)
            .into_iter()
            .next()
        else {
            println!(
                "No unvalidated material keys matched filters (min-games: {}, max-pieces: {}).",
                min_games, max_pieces
            );
            break;
        };
        prefer_stale = !target.is_stale;
        if target.is_stale {
            println!("Regenerating stale {}", target.material_key);
        } else {
            println!("Generating {}", target.material_key);
        }
        let material = target.material_key;
        if run_generate_with_db(material.clone(), &db)? {
            validated_unchanged.remove(&material);
        } else {
            validated_unchanged.insert(material, target.max_child_updated_at);
        }
    }

    Ok(())
}

fn effective_new_table_budget(remaining: u64, smallest_new_table: Option<u64>) -> u64 {
    smallest_new_table.map_or(remaining, |smallest| remaining.max(smallest))
}

fn current_timestamp() -> Result<i64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::effective_new_table_budget;

    #[test]
    fn position_budget_allows_smallest_table_to_cross_target() {
        assert_eq!(effective_new_table_budget(2_560, Some(8_192)), 8_192);
        assert_eq!(effective_new_table_budget(35_328, Some(8_192)), 35_328);
    }
}
