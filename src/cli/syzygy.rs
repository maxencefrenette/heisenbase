use std::path::Path;

use anyhow::{Result, bail};
use heisenbase::position_indexer::PositionIndexer;
use heisenbase::storage::Database;
use heisenbase::wdl_score_range::WdlScoreRange;
use rand::{Rng, SeedableRng, rngs::StdRng};
use shakmaty::{Chess, EnPassantMode, fen::Fen};
use shakmaty_syzygy::{SyzygyError, Tablebase, Wdl};

const SAMPLES_PER_TABLE: usize = 256;
const MAX_MISMATCHES_PER_TABLE: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimpleWdl {
    Win,
    Draw,
    Loss,
}

fn simplify_wdl(wdl: Wdl) -> SimpleWdl {
    match wdl {
        Wdl::Win | Wdl::CursedWin => SimpleWdl::Win,
        Wdl::Draw => SimpleWdl::Draw,
        Wdl::Loss | Wdl::BlessedLoss => SimpleWdl::Loss,
    }
}

fn heisenbase_allows(wdl: WdlScoreRange, syzygy: SimpleWdl) -> bool {
    match wdl {
        WdlScoreRange::Win => syzygy == SimpleWdl::Win,
        WdlScoreRange::Draw => syzygy == SimpleWdl::Draw,
        WdlScoreRange::Loss => syzygy == SimpleWdl::Loss,
        WdlScoreRange::WinOrDraw => matches!(syzygy, SimpleWdl::Win | SimpleWdl::Draw),
        WdlScoreRange::DrawOrLoss => matches!(syzygy, SimpleWdl::Draw | SimpleWdl::Loss),
        WdlScoreRange::Unknown => true,
        WdlScoreRange::IllegalPosition => false,
    }
}

fn collect_valid_indices(indexer: &PositionIndexer) -> Vec<usize> {
    (0..indexer.total_positions())
        .filter(|&idx| indexer.index_to_position(idx).is_ok())
        .collect()
}

pub(crate) fn run() -> Result<()> {
    let syzygy_dir = Path::new("./data/syzygy");
    let db = Database::open_default()?;

    let mut tablebase = Tablebase::<Chess>::new();
    let added = tablebase.add_directory(syzygy_dir)?;
    if added == 0 {
        bail!("no syzygy tables found in {}", syzygy_dir.display());
    }

    let mut three_man = Vec::new();
    let mut four_man = Vec::new();
    for key in db.list_wdl_table_keys()? {
        match key.total_piece_count() {
            3 => three_man.push(key),
            4 => four_man.push(key),
            _ => (),
        }
    }

    let mut rng = StdRng::from_entropy();
    let mut total_tables = 0usize;
    let mut total_positions = 0usize;
    let mut total_mismatches = 0usize;
    let mut total_uncertain = 0usize;
    let mut missing_tables = 0usize;
    let mut probe_errors = 0usize;

    for (label, keys) in [("3-man", three_man), ("4-man", four_man)] {
        println!(
            "Checking {} material keys ({} tables)...",
            label,
            keys.len()
        );
        for material in keys {
            total_tables += 1;
            let Some(table) = db.get_wdl_table(&material)? else {
                eprintln!("Missing heisenbase table for {}", material);
                continue;
            };
            let indexer = PositionIndexer::new(material.clone());
            let valid_indices = collect_valid_indices(&indexer);
            if valid_indices.is_empty() {
                eprintln!("No valid positions for {}", material);
                continue;
            }

            let mut mismatches = 0usize;
            let mut uncertain = 0usize;
            let mut missing_table = false;
            let mut probe_failed = false;

            for _ in 0..SAMPLES_PER_TABLE {
                let idx = valid_indices[rng.gen_range(0..valid_indices.len())];
                let pos = match indexer.index_to_position(idx) {
                    Ok(pos) => pos,
                    Err(_) => continue,
                };

                let hb_wdl = table.positions[idx];
                if hb_wdl.is_uncertain() {
                    uncertain += 1;
                }

                let syzygy_wdl = match tablebase.probe_wdl_after_zeroing(&pos) {
                    Ok(wdl) => wdl,
                    Err(SyzygyError::MissingTable { .. }) => {
                        missing_table = true;
                        break;
                    }
                    Err(_) => {
                        probe_failed = true;
                        break;
                    }
                };

                if !heisenbase_allows(hb_wdl, simplify_wdl(syzygy_wdl)) {
                    mismatches += 1;
                    if mismatches <= MAX_MISMATCHES_PER_TABLE {
                        let fen = Fen::from_position(&pos, EnPassantMode::Legal).to_string();
                        println!(
                            "Mismatch {}: hb={:?}, syzygy={:?}, fen={}",
                            material, hb_wdl, syzygy_wdl, fen
                        );
                    }
                }
            }

            if missing_table {
                missing_tables += 1;
                eprintln!("Missing Syzygy tables for {}", material);
                continue;
            }
            if probe_failed {
                probe_errors += 1;
                eprintln!("Syzygy probe failed for {}", material);
                continue;
            }

            total_positions += SAMPLES_PER_TABLE;
            total_mismatches += mismatches;
            total_uncertain += uncertain;

            if mismatches > 0 {
                println!(
                    "Found {} mismatches in {} ({} uncertain samples).",
                    mismatches, material, uncertain
                );
            }
        }
    }

    println!(
        "Checked {} tables ({} positions).",
        total_tables, total_positions
    );
    println!("Mismatches: {}", total_mismatches);
    println!("Uncertain samples: {}", total_uncertain);
    if missing_tables > 0 {
        println!("Missing Syzygy tables: {}", missing_tables);
    }
    if probe_errors > 0 {
        println!("Syzygy probe errors: {}", probe_errors);
    }

    if total_mismatches > 0 || missing_tables > 0 || probe_errors > 0 {
        bail!("syzygy comparison reported mismatches or errors");
    }

    Ok(())
}
