use anyhow::{Result, anyhow};
use heisenbase::material_key::MaterialKey;
use heisenbase::storage;
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_score_range::WdlScoreRange;
use heisenbase::wdl_table::WdlTable;
use polars::prelude::*;
use rusqlite::{Connection, params};

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

    let conn = storage::open_database()?;
    storage::store_wdl_table(&conn, &wdl_table)?;
    log_stats_to_index(&conn, &wdl_table, &counts)?;
    println!(
        "Stored table in {} for {}",
        storage::DB_PATH,
        wdl_table.material
    );
    println!();
    Ok(())
}

fn log_stats_to_index(conn: &Connection, wdl_table: &WdlTable, counts: &[usize; 7]) -> Result<()> {
    let mut children: Vec<String> = wdl_table
        .material
        .child_material_keys()
        .into_iter()
        .map(|key| key.to_string())
        .collect();
    children.sort();
    let children_json = serde_json::to_string(&children)?;

    conn.execute(
        "INSERT INTO material_keys (
            name,
            children_json,
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
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(name) DO UPDATE SET
            children_json = excluded.children_json,
            num_pieces = excluded.num_pieces,
            num_pawns = excluded.num_pawns,
            num_non_pawns = excluded.num_non_pawns,
            total = excluded.total,
            illegal = excluded.illegal,
            win = excluded.win,
            draw = excluded.draw,
            loss = excluded.loss,
            win_or_draw = excluded.win_or_draw,
            draw_or_loss = excluded.draw_or_loss,
            unknown = excluded.unknown",
        params![
            wdl_table.material.to_string(),
            children_json,
            wdl_table.material.total_piece_count() as i64,
            wdl_table.material.pawns.pawn_count() as i64,
            wdl_table.material.non_pawn_piece_count() as i64,
            wdl_table.positions.len() as i64,
            counts[WdlScoreRange::IllegalPosition as usize] as i64,
            counts[WdlScoreRange::Win as usize] as i64,
            counts[WdlScoreRange::Draw as usize] as i64,
            counts[WdlScoreRange::Loss as usize] as i64,
            counts[WdlScoreRange::WinOrDraw as usize] as i64,
            counts[WdlScoreRange::DrawOrLoss as usize] as i64,
            counts[WdlScoreRange::Unknown as usize] as i64,
        ],
    )?;

    Ok(())
}

pub(crate) fn run_generate_many(min_games: u64, max_pieces: u32) -> Result<()> {
    let conn = storage::open_database()?;

    loop {
        let candidates = compute_transitive_utility_candidates(&conn, min_games, max_pieces)?;
        let mut next = None;
        for candidate in candidates {
            if !storage::has_wdl_table(&conn, &candidate)? {
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
        println!("Generating {}", material_key);
        run_generate(material_key)?;
    }

    Ok(())
}

fn compute_transitive_utility_candidates(
    conn: &Connection,
    min_games: u64,
    max_pieces: u32,
) -> Result<Vec<MaterialKey>> {
    let material_lf = load_material_keys_df(conn)?.lazy();
    let pgn_lf = load_pgn_index_df(conn)?.lazy();
    let transitive_lf = compute_transitive_utility_lf(material_lf.clone(), pgn_lf.clone());

    // Direct utility and transitive utility both represent shares of global PGN
    // position mass. We divide by material_key_size only at ranking time so both
    // scores are normalized in the same way when we compare candidate tables.
    let sorted = pgn_lf
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
        .join(
            transitive_lf,
            [col("material_key")],
            [col("material_key")],
            JoinArgs::new(JoinType::Left),
        )
        .with_columns([
            col("transitive_utility").fill_null(lit(0.0)),
            (col("utility") / col("material_key_size").cast(DataType::Float64))
                .alias("utility_rank"),
            (col("transitive_utility") / col("material_key_size").cast(DataType::Float64))
                .alias("transitive_utility_rank"),
        ])
        .sort(
            ["transitive_utility_rank", "utility_rank", "material_key"],
            SortMultipleOptions::new().with_order_descending_multi([true, true, false]),
        )
        .collect()?;
    let keys = sorted.column("material_key")?.str()?;
    let mut candidates = Vec::with_capacity(keys.len());
    for key in keys.into_iter() {
        let key = key.ok_or_else(|| anyhow!("material_key is null"))?;
        let material_key = MaterialKey::from_string(key)
            .map_err(|err| anyhow!("invalid material key in pgn_index: {key}: {err}"))?;
        candidates.push(material_key);
    }
    Ok(candidates)
}

fn load_material_keys_df(conn: &Connection) -> Result<DataFrame> {
    let mut stmt = conn.prepare(
        "SELECT name, children_json, total, unknown, win_or_draw, draw_or_loss
         FROM material_keys",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, i64>(5)?,
        ))
    })?;

    let mut names = Vec::new();
    let mut children = Vec::new();
    let mut totals = Vec::new();
    let mut unresolved = Vec::new();
    for row in rows {
        let (name, children_json, total, unknown, win_or_draw, draw_or_loss) = row?;
        let parsed: Vec<String> = serde_json::from_str(&children_json)
            .map_err(|err| anyhow!("invalid children JSON for {name}: {err}"))?;
        names.push(name);
        totals.push(total as f64);
        unresolved.push((unknown + win_or_draw + draw_or_loss) as f64);
        children.push(parsed);
    }

    let child_counts: Vec<i64> = children.iter().map(|list| list.len() as i64).collect();
    let total_children = children.iter().map(Vec::len).sum();
    let mut children_builder =
        ListStringChunkedBuilder::new("children", children.len(), total_children);
    for list in &children {
        children_builder.append_values_iter(list.iter().map(String::as_str));
    }

    Ok(DataFrame::new(vec![
        Series::new("name", names),
        children_builder.finish().into_series(),
        Series::new("total", totals),
        Series::new("unresolved", unresolved),
        Series::new("child_count", child_counts),
    ])?)
}

fn load_pgn_index_df(conn: &Connection) -> Result<DataFrame> {
    let mut stmt = conn.prepare(
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

fn compute_transitive_utility_lf(material_lf: LazyFrame, pgn_lf: LazyFrame) -> LazyFrame {
    // Transitive utility propagates only one step into unsolved territory.
    // Each solved parent contributes:
    //
    //     parent.utility * (parent.unresolved / parent.total) / parent.child_count
    //
    // This keeps propagated mass in the same units as direct utility:
    // both are shares of global PGN position mass before any size normalization.
    material_lf
        .clone()
        .filter(col("child_count").gt(lit(0)))
        .join(
            pgn_lf.select([col("material_key"), col("utility")]),
            [col("name")],
            [col("material_key")],
            JoinArgs::new(JoinType::Left),
        )
        .with_columns([col("utility").fill_null(lit(0.0))])
        .explode([col("children")])
        .join(
            material_lf.clone(),
            [col("children")],
            [col("name")],
            JoinArgs::new(JoinType::Left),
        )
        .filter(col("children_right").is_null())
        .group_by([col("children")])
        .agg([(col("utility") * (col("unresolved") / col("total"))
            / col("child_count").cast(DataType::Float64))
        .sum()
        .alias("transitive_utility")])
        .rename(["children"], ["material_key"])
}
