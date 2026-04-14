use duckdb::{Connection, params};
use polars::prelude::*;
use std::ops::Not;
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
    loop {
        let candidates = compute_transitive_utility_candidates(min_games, max_pieces)?;
        let mut next = None;
        for candidate in candidates {
            let filename = format!("./data/heisenbase/{}.hbt", candidate);
            if !Path::new(&filename).exists() {
                next = Some(candidate);
                break;
            }
        }
        let Some(material_key) = next else {
            println!(
                "No material keys matched filters (min-games: {}, max-pieces: {}).",
                min_games, max_pieces
            );
            break;
        };
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

fn compute_transitive_utility_candidates(
    min_games: u64,
    max_pieces: u32,
) -> Result<Vec<MaterialKey>> {
    let material_df = load_material_keys_df()?;
    let transitive_df = compute_transitive_utility_df(&material_df)?;
    let pgn_df = LazyFrame::scan_parquet(index_pgn::PARQUET_PATH, Default::default())?
        .filter(
            col("num_games")
                .cast(DataType::Int64)
                .gt(lit(min_games as i64)),
        )
        .filter(
            col("num_pieces")
                .cast(DataType::Int64)
                .lt_eq(lit(max_pieces as i64)),
        )
        .collect()?;
    let joined = pgn_df.join(
        &transitive_df,
        ["material_key"],
        ["material_key"],
        JoinArgs::new(JoinType::Left),
    )?;
    let joined = joined
        .lazy()
        .with_columns([col("transitive_utility").fill_null(lit(0.0))])
        .collect()?;
    let sorted = joined.sort(
        ["transitive_utility"],
        SortMultipleOptions::new().with_order_descending(true),
    )?;
    let keys = sorted.column("material_key")?.str()?;
    let mut candidates = Vec::with_capacity(keys.len());
    for key in keys.into_iter() {
        let key = key.ok_or_else(|| anyhow!("material_key is null"))?;
        let material_key = MaterialKey::from_string(key)
            .map_err(|err| anyhow!("invalid material key: {key}: {err}"))?;
        candidates.push(material_key);
    }
    Ok(candidates)
}

fn load_material_keys_df() -> Result<DataFrame> {
    let conn = Connection::open("./data/heisenbase/index.duckdb")?;
    let mut stmt = conn.prepare(
        "SELECT name, to_json(children) AS children_json, unknown, win_or_draw, draw_or_loss \
         FROM material_keys",
    )?;
    let rows = stmt.query_map([], |row| {
        let name: String = row.get(0)?;
        let children_json: String = row.get(1)?;
        let unknown: i64 = row.get(2)?;
        let win_or_draw: i64 = row.get(3)?;
        let draw_or_loss: i64 = row.get(4)?;
        Ok((name, children_json, unknown, win_or_draw, draw_or_loss))
    })?;

    let mut names = Vec::new();
    let mut children = Vec::new();
    let mut unresolved = Vec::new();
    for row in rows {
        let (name, children_json, unknown, win_or_draw, draw_or_loss) = row?;
        let parsed: Vec<String> = serde_json::from_str(&children_json)
            .map_err(|err| anyhow!("invalid children JSON for {name}: {err}"))?;
        names.push(name);
        unresolved.push((unknown + win_or_draw + draw_or_loss) as f64);
        children.push(parsed);
    }
    let child_counts: Vec<i64> = children.iter().map(|c| c.len() as i64).collect();
    let total_children: usize = child_counts.iter().map(|count| *count as usize).sum();
    let mut children_builder =
        ListStringChunkedBuilder::new("children", children.len(), total_children);
    for list in &children {
        children_builder.append_values_iter(list.iter().map(|s| s.as_str()));
    }
    let children_series = children_builder.finish().into_series();

    Ok(DataFrame::new(vec![
        Series::new("name", names),
        children_series,
        Series::new("unresolved", unresolved),
        Series::new("child_count", child_counts),
    ])?)
}

fn compute_transitive_utility_df(material_df: &DataFrame) -> Result<DataFrame> {
    let edges = material_df
        .clone()
        .lazy()
        .filter(col("child_count").gt(lit(0)))
        .explode(["children"])
        .select([
            col("children").alias("child"),
            (col("unresolved") / col("child_count").cast(DataType::Float64)).alias("share"),
        ])
        .collect()?;

    let mut current = edges;
    let mut terminals: Vec<DataFrame> = Vec::new();
    let material_lf = material_df.clone().lazy();

    loop {
        if current.height() == 0 {
            break;
        }
        let joined = current
            .lazy()
            .join(
                material_lf.clone(),
                [col("child")],
                [col("name")],
                JoinArgs::new(JoinType::Left),
            )
            .collect()?;
        let children_col = joined.column("children")?;
        let missing_mask = children_col.is_null();
        let missing = joined.filter(&missing_mask)?;
        if missing.height() > 0 {
            terminals.push(missing.select(["child", "share"])?);
        }
        let present_mask = missing_mask.not();
        let present = joined.filter(&present_mask)?;
        if present.height() == 0 {
            break;
        }
        let next = present
            .lazy()
            .filter(col("child_count").gt(lit(0)))
            .explode(["children"])
            .select([
                col("children").alias("child"),
                (col("share") / col("child_count").cast(DataType::Float64)).alias("share"),
            ])
            .collect()?;
        current = next;
    }

    if terminals.is_empty() {
        return Ok(DataFrame::new(vec![
            Series::new("material_key", Vec::<String>::new()),
            Series::new("transitive_utility", Vec::<f64>::new()),
        ])?);
    }

    let mut combined = terminals[0].clone();
    for df in terminals.iter().skip(1) {
        combined.vstack_mut(df)?;
    }
    let mut grouped = combined
        .lazy()
        .group_by([col("child")])
        .agg([col("share").sum().alias("transitive_utility")])
        .collect()?;
    grouped.rename("child", "material_key")?;
    Ok(grouped)
}
