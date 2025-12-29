use polars::prelude::*;
use std::{error::Error, io, path::Path};

use super::index_pgn;
use heisenbase::material_key::MaterialKey;
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_file::write_wdl_file;
use heisenbase::wdl_score_range::WdlScoreRange;
use heisenbase::wdl_table::WdlTable;

pub(crate) fn run_generate(material: MaterialKey) -> io::Result<()> {
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
    let filename = format!("./data/heisenbase/{}.hbt", wdl_table.material);
    write_wdl_file(&filename, &wdl_table)?;
    println!("Wrote table to {}", filename);
    println!();
    Ok(())
}

pub(crate) fn run_generate_many(min_games: u64, max_pieces: u32) -> Result<(), Box<dyn Error>> {
    let df = LazyFrame::scan_parquet(index_pgn::PARQUET_PATH, Default::default())
        .unwrap()
        .filter(col("num_games").gt(1))
        .with_columns([
            (lit(1_000_000_000f64) * col("num_positions").cast(DataType::Float64)
                / col("total_positions").cast(DataType::Float64)
                / col("material_key_size").cast(DataType::Float64))
            .alias("utility"),
        ])
        .sort(
            ["utility"],
            SortMultipleOptions::new().with_order_descending(true),
        )
        .collect()
        .unwrap();

    let keys = df.column("material_key").unwrap();

    let mut candidates = Vec::new();
    for key in keys.str().unwrap().into_iter() {
        let material_key = MaterialKey::from_string(key.expect("material_key null"))
            .expect("invalid material key");
        if material_key.total_piece_count() > max_pieces {
            continue;
        }
        candidates.push(material_key);
    }

    if candidates.is_empty() {
        println!(
            "No material keys matched filters (min-games: {}, max-pieces: {}).",
            min_games, max_pieces
        );
        return Ok(());
    }

    println!(
        "Generating {} material keys (min-games: {}, max-pieces: {}).",
        candidates.len(),
        min_games,
        max_pieces
    );

    for material_key in candidates {
        let material_str = material_key.to_string();
        let filename = format!("./data/heisenbase/{}.hbt", material_str);
        if Path::new(&filename).exists() {
            println!("Skipping {} (already exists)", material_str);
            continue;
        }
        println!("Generating {}", material_str);
        if let Err(err) = run_generate(material_key) {
            return Err(
                io::Error::other(format!("failed to generate {}: {}", material_str, err)).into(),
            );
        }
    }

    Ok(())
}
