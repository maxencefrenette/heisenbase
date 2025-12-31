use duckdb::{Connection, params};
use polars::prelude::*;
use std::{fs, path::Path};

use super::index_pgn;
use anyhow::{Result, anyhow};
use heisenbase::material_key::MaterialKey;
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_file::write_wdl_file;
use heisenbase::wdl_score_range::WdlScoreRange;
use heisenbase::wdl_table::WdlTable;

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
    let heisenbase_dir = Path::new("./data/heisenbase");
    fs::create_dir_all(heisenbase_dir)?;
    let filename = heisenbase_dir.join(format!("{}.hbt", wdl_table.material));
    write_wdl_file(&filename, &wdl_table)?;
    log_stats_to_index(&wdl_table, &counts)?;
    println!("Wrote table to {}", filename.display());
    println!();
    Ok(())
}

fn log_stats_to_index(wdl_table: &WdlTable, counts: &[usize; 7]) -> Result<()> {
    let heisenbase_dir = Path::new("./data/heisenbase");
    fs::create_dir_all(heisenbase_dir)?;

    let mut children: Vec<String> = wdl_table
        .material
        .child_material_keys()
        .into_iter()
        .map(|key| key.to_string())
        .collect();
    children.sort();

    let name = wdl_table.material.to_string();
    let num_pieces = wdl_table.material.total_piece_count() as i64;
    let num_pawns = wdl_table.material.pawns.pawn_count() as i64;
    let num_non_pawns = wdl_table.material.non_pawn_piece_count() as i64;
    let total = wdl_table.positions.len() as i64;
    let children_literal = if children.is_empty() {
        "[]".to_string()
    } else {
        let items = children
            .iter()
            .map(|child| format!("'{}'", child.replace('\'', "''")))
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{}]", items)
    };

    let conn = Connection::open(heisenbase_dir.join("index.duckdb"))?;
    conn.execute("DELETE FROM material_keys WHERE name = ?", params![name])?;
    let insert_sql = format!(
        "INSERT INTO material_keys (
            name,
            children,
            num_pieces,
            num_pawns,
            num_non_pawns,
            total,
            illegal,
            win,
            draw,
            loss,
            win_or_draw,
            draw_or_loss,
            unknown
        ) VALUES ('{}', {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
        name.replace('\'', "''"),
        children_literal,
        num_pieces,
        num_pawns,
        num_non_pawns,
        total,
        counts[WdlScoreRange::IllegalPosition as usize],
        counts[WdlScoreRange::Win as usize],
        counts[WdlScoreRange::Draw as usize],
        counts[WdlScoreRange::Loss as usize],
        counts[WdlScoreRange::WinOrDraw as usize],
        counts[WdlScoreRange::DrawOrLoss as usize],
        counts[WdlScoreRange::Unknown as usize],
    );
    conn.execute(&insert_sql, [])?;

    Ok(())
}

pub(crate) fn run_generate_many(min_games: u64, max_pieces: u32) -> Result<()> {
    let df = LazyFrame::scan_parquet(index_pgn::PARQUET_PATH, Default::default())?
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
        .collect()?;

    let keys = df.column("material_key")?;

    let mut candidates = Vec::new();
    for key in keys.str()?.into_iter() {
        let key = key.ok_or_else(|| anyhow!("material_key is null"))?;
        let material_key = MaterialKey::from_string(key)
            .map_err(|err| anyhow!("invalid material key: {key}: {err}"))?;
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
        run_generate(material_key)?;
    }

    Ok(())
}
