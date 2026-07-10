use anyhow::{Result, anyhow};
use polars::prelude::*;
use rusqlite::Connection;

use crate::material_key::MaterialKey;
use crate::position_indexer::PositionIndexer;
use crate::storage::Database;

#[derive(Debug)]
pub struct GenerationTarget {
    pub material_key: MaterialKey,
    pub is_stale: bool,
    pub num_games: i64,
    pub material_key_size: i64,
    pub utility: f64,
    pub utility_rank: f64,
    pub transitive_utility: f64,
    pub transitive_utility_rank: f64,
}

#[derive(Debug)]
pub struct UtilityStats {
    pub material_key: MaterialKey,
    pub has_table: bool,
    pub has_pgn_index: bool,
    pub num_games: i64,
    pub material_key_size: i64,
    pub utility: f64,
    pub utility_rank: f64,
    pub transitive_utility: f64,
    pub transitive_utility_rank: f64,
    pub contributions: Vec<TransitiveUtilityContribution>,
}

#[derive(Debug)]
pub struct TransitiveUtilityContribution {
    pub parent: String,
    pub parent_utility: f64,
    pub parent_total: i64,
    pub parent_unresolved: i64,
    pub unresolved_fraction: f64,
    pub child_count: i64,
    pub contribution: f64,
}

pub fn compute_generation_targets(
    db: &Database,
    min_games: u64,
    max_pieces: u32,
) -> Result<Vec<GenerationTarget>> {
    let material_lf = load_material_keys_df(db.conn())?.lazy();
    let pgn_lf = load_pgn_index_df(db.conn())?.lazy();
    let transitive_lf = compute_transitive_utility_lf(material_lf.clone(), pgn_lf.clone());
    let stale_lf = compute_stale_material_keys_lf(material_lf.clone());

    // Direct utility and transitive utility both represent shares of global PGN
    // position mass. We divide by material_key_size only at ranking time so both
    // scores are normalized in the same way when we compare candidate tables.
    //
    // We also carry solved/stale status through the same lazy pipeline:
    // - `has_table` means the material key is already solved
    // - `is_stale` means at least one solved child dependency is newer than it
    //
    // Stale solved tables always rank ahead of unsolved tables.
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
        .join(
            material_lf.select([col("name"), col("updated_at")]),
            [col("material_key")],
            [col("name")],
            JoinArgs::new(JoinType::Left),
        )
        .join(
            stale_lf,
            [col("material_key")],
            [col("material_key")],
            JoinArgs::new(JoinType::Left),
        )
        .with_columns([
            col("transitive_utility").fill_null(lit(0.0)),
            col("updated_at").is_not_null().alias("has_table"),
            col("is_stale").fill_null(lit(false)),
        ])
        .with_columns([
            (col("utility") / col("material_key_size").cast(DataType::Float64))
                .alias("utility_rank"),
            (col("transitive_utility") / col("material_key_size").cast(DataType::Float64))
                .alias("transitive_utility_rank"),
        ])
        .sort(
            [
                "is_stale",
                "has_table",
                "transitive_utility_rank",
                "utility_rank",
                "material_key",
            ],
            SortMultipleOptions::new().with_order_descending_multi([true, true, true, true, false]),
        )
        .collect()?;

    let keys = sorted.column("material_key")?.str()?;
    let stale = sorted.column("is_stale")?.bool()?;
    let solved = sorted.column("has_table")?.bool()?;
    let num_games = sorted.column("num_games")?.i64()?;
    let material_key_size = sorted.column("material_key_size")?.i64()?;
    let utility = sorted.column("utility")?.f64()?;
    let utility_rank = sorted.column("utility_rank")?.f64()?;
    let transitive_utility = sorted.column("transitive_utility")?.f64()?;
    let transitive_utility_rank = sorted.column("transitive_utility_rank")?.f64()?;
    let mut candidates = Vec::with_capacity(keys.len());
    for index in 0..sorted.height() {
        let key = keys
            .get(index)
            .ok_or_else(|| anyhow!("material_key is null"))?;
        let is_stale = stale
            .get(index)
            .ok_or_else(|| anyhow!("is_stale is null"))?;
        let has_table = solved
            .get(index)
            .ok_or_else(|| anyhow!("has_table is null"))?;
        if has_table && !is_stale {
            continue;
        }
        let material_key = MaterialKey::from_string(key)
            .map_err(|err| anyhow!("invalid material key in pgn_index: {key}: {err}"))?;
        candidates.push(GenerationTarget {
            material_key,
            is_stale,
            num_games: num_games
                .get(index)
                .ok_or_else(|| anyhow!("num_games is null"))?,
            material_key_size: material_key_size
                .get(index)
                .ok_or_else(|| anyhow!("material_key_size is null"))?,
            utility: utility
                .get(index)
                .ok_or_else(|| anyhow!("utility is null"))?,
            utility_rank: utility_rank
                .get(index)
                .ok_or_else(|| anyhow!("utility_rank is null"))?,
            transitive_utility: transitive_utility
                .get(index)
                .ok_or_else(|| anyhow!("transitive_utility is null"))?,
            transitive_utility_rank: transitive_utility_rank
                .get(index)
                .ok_or_else(|| anyhow!("transitive_utility_rank is null"))?,
        });
    }
    Ok(candidates)
}

pub fn compute_utility_stats(db: &Database, material: &MaterialKey) -> Result<UtilityStats> {
    let pgn_row = db.get_pgn_index_row(material)?;
    let has_pgn_index = pgn_row.is_some();
    let material_key_size = pgn_row
        .as_ref()
        .map(|row| row.material_key_size)
        .unwrap_or_else(|| PositionIndexer::new(material.clone()).total_positions() as i64);
    let num_games = pgn_row.as_ref().map_or(0, |row| row.num_games);
    let utility = pgn_row.as_ref().map_or(0.0, |row| row.utility);

    let material_lf = load_material_keys_df(db.conn())?.lazy();
    let pgn_lf = load_pgn_index_df(db.conn())?.lazy();
    let contributions_df = compute_transitive_utility_contributions_lf(material_lf, pgn_lf)
        .filter(col("material_key").eq(lit(material.to_string())))
        .filter(col("contribution").gt(lit(0.0)))
        .sort(
            ["contribution", "parent"],
            SortMultipleOptions::new().with_order_descending_multi([true, false]),
        )
        .collect()?;

    let parents = contributions_df.column("parent")?.str()?;
    let parent_utility = contributions_df.column("parent_utility")?.f64()?;
    let parent_total = contributions_df.column("parent_total")?.f64()?;
    let parent_unresolved = contributions_df.column("parent_unresolved")?.f64()?;
    let unresolved_fraction = contributions_df.column("unresolved_fraction")?.f64()?;
    let child_count = contributions_df.column("child_count")?.i64()?;
    let contribution = contributions_df.column("contribution")?.f64()?;
    let mut contributions = Vec::with_capacity(contributions_df.height());
    for index in 0..contributions_df.height() {
        contributions.push(TransitiveUtilityContribution {
            parent: parents
                .get(index)
                .ok_or_else(|| anyhow!("parent is null"))?
                .to_string(),
            parent_utility: parent_utility
                .get(index)
                .ok_or_else(|| anyhow!("parent_utility is null"))?,
            parent_total: parent_total
                .get(index)
                .ok_or_else(|| anyhow!("parent_total is null"))? as i64,
            parent_unresolved: parent_unresolved
                .get(index)
                .ok_or_else(|| anyhow!("parent_unresolved is null"))?
                as i64,
            unresolved_fraction: unresolved_fraction
                .get(index)
                .ok_or_else(|| anyhow!("unresolved_fraction is null"))?,
            child_count: child_count
                .get(index)
                .ok_or_else(|| anyhow!("child_count is null"))?,
            contribution: contribution
                .get(index)
                .ok_or_else(|| anyhow!("contribution is null"))?,
        });
    }

    let transitive_utility = contributions
        .iter()
        .map(|contribution| contribution.contribution)
        .sum();

    Ok(UtilityStats {
        material_key: material.clone(),
        has_table: db.has_wdl_table(material)?,
        has_pgn_index,
        num_games,
        material_key_size,
        utility,
        utility_rank: utility / material_key_size as f64,
        transitive_utility,
        transitive_utility_rank: transitive_utility / material_key_size as f64,
        contributions,
    })
}

fn load_material_keys_df(conn: &Connection) -> Result<DataFrame> {
    let mut stmt = conn.prepare(
        "SELECT name, children_json, total, unknown, win_or_draw, draw_or_loss, updated_at
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
            row.get::<_, i64>(6)?,
        ))
    })?;

    let mut names = Vec::new();
    let mut children = Vec::new();
    let mut totals = Vec::new();
    let mut unresolved = Vec::new();
    let mut updated_at = Vec::new();
    for row in rows {
        let (name, children_json, total, unknown, win_or_draw, draw_or_loss, updated) = row?;
        let parsed: Vec<String> = serde_json::from_str(&children_json)
            .map_err(|err| anyhow!("invalid children JSON for {name}: {err}"))?;
        names.push(name);
        totals.push(total as f64);
        unresolved.push((unknown + win_or_draw + draw_or_loss) as f64);
        updated_at.push(updated);
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
        Series::new("updated_at", updated_at),
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
    compute_transitive_utility_contributions_lf(material_lf, pgn_lf)
        .group_by([col("material_key")])
        .agg([col("contribution").sum().alias("transitive_utility")])
}

fn compute_transitive_utility_contributions_lf(
    material_lf: LazyFrame,
    pgn_lf: LazyFrame,
) -> LazyFrame {
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
        .with_columns([
            (col("unresolved") / col("total")).alias("unresolved_fraction"),
            (col("utility") * (col("unresolved") / col("total"))
                / col("child_count").cast(DataType::Float64))
            .alias("contribution"),
        ])
        .select([
            col("children").alias("material_key"),
            col("name").alias("parent"),
            col("utility").alias("parent_utility"),
            col("total").alias("parent_total"),
            col("unresolved").alias("parent_unresolved"),
            col("unresolved_fraction"),
            col("child_count"),
            col("contribution"),
        ])
}

fn compute_stale_material_keys_lf(material_lf: LazyFrame) -> LazyFrame {
    // A solved table is stale once any solved child dependency is newer than the
    // table itself. This is computed in the same lazy pipeline as ranking so we
    // avoid per-candidate database lookups.
    material_lf
        .clone()
        .filter(col("child_count").gt(lit(0)))
        .explode([col("children")])
        .join(
            material_lf.clone().select([col("name"), col("updated_at")]),
            [col("children")],
            [col("name")],
            JoinArgs::new(JoinType::Inner),
        )
        .group_by([col("name")])
        .agg([
            col("updated_at").first().alias("updated_at"),
            col("updated_at_right").max().alias("max_child_updated_at"),
        ])
        .with_columns([col("max_child_updated_at")
            .gt(col("updated_at"))
            .alias("is_stale")])
        .select([col("name").alias("material_key"), col("is_stale")])
}

#[cfg(test)]
mod tests {
    use super::{compute_generation_targets, compute_utility_stats};
    use crate::material_key::MaterialKey;
    use crate::storage::{Database, MaterialStatsRow, PgnIndexRow};
    use rusqlite::{Connection, params};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn explains_parent_transitive_utility_contribution() {
        let db_path = temp_db_path("utility-contribution");
        let mut db = Database::open_at(&db_path).unwrap();
        db.upsert_material_stats(&MaterialStatsRow {
            name: "KQvKR".to_string(),
            children: vec!["KNvK".to_string()],
            num_pieces: 5,
            num_pawns: 0,
            num_non_pawns: 5,
            total: 100,
            illegal: 0,
            win: 70,
            draw: 0,
            loss: 0,
            win_or_draw: 10,
            draw_or_loss: 10,
            unknown: 10,
            updated_at: 1,
        })
        .unwrap();
        db.replace_pgn_index(&[
            PgnIndexRow {
                material_key: "KQvKR".to_string(),
                num_games: 10,
                num_positions: 10,
                total_games: 100,
                total_positions: 100,
                material_key_size: 100,
                num_pieces: 5,
                num_pawns: 0,
                num_non_pawns: 5,
                utility: 0.2,
            },
            PgnIndexRow {
                material_key: "KNvK".to_string(),
                num_games: 100,
                num_positions: 90,
                total_games: 100,
                total_positions: 100,
                material_key_size: 10,
                num_pieces: 3,
                num_pawns: 0,
                num_non_pawns: 3,
                utility: 0.9,
            },
        ])
        .unwrap();

        let material = MaterialKey::from_string("KNvK").unwrap();
        let stats = compute_utility_stats(&db, &material).unwrap();
        assert_eq!(stats.contributions.len(), 1);
        assert_eq!(stats.contributions[0].parent, "KQvKR");
        assert!((stats.contributions[0].unresolved_fraction - 0.3).abs() < 1e-12);
        assert!((stats.transitive_utility - 0.06).abs() < 1e-12);
        assert!((stats.transitive_utility_rank - 0.006).abs() < 1e-12);

        drop(db);
        fs::remove_file(&db_path).unwrap();
    }

    #[test]
    fn prefers_stale_tables_over_unsolved_candidates() {
        let db_path = temp_db_path("stale-priority");
        let db = Database::open_at(&db_path).unwrap();
        let conn = db.conn();
        seed_wdl_table(conn, "KQvKR");
        seed_wdl_table(conn, "KQvK");
        seed_wdl_table(conn, "KRvK");

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
                unknown,
                updated_at
            ) VALUES (?1, ?2, 5, 0, 5, 100, 0, 0, 0, 0, 10, 10, 10, ?3)",
            params!["KQvKR", "[\"KQvK\",\"KRvK\"]", 10_i64],
        )
        .unwrap();
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
                unknown,
                updated_at
            ) VALUES (?1, '[]', 3, 0, 3, 100, 0, 0, 0, 0, 0, 0, 0, ?2)",
            params!["KQvK", 20_i64],
        )
        .unwrap();
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
                unknown,
                updated_at
            ) VALUES (?1, '[]', 3, 0, 3, 100, 0, 0, 0, 0, 0, 0, 0, ?2)",
            params!["KRvK", 10_i64],
        )
        .unwrap();

        // KNvK is unsolved and has much higher direct utility, but KQvKR is stale
        // because one of its solved child tables is newer.
        conn.execute(
            "INSERT INTO pgn_index (
                material_key,
                num_games,
                num_positions,
                total_games,
                total_positions,
                material_key_size,
                num_pieces,
                num_pawns,
                num_non_pawns,
                utility
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                "KQvKR", 10_i64, 10_i64, 100_i64, 100_i64, 1_i64, 5_i64, 0_i64, 5_i64, 0.1_f64
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pgn_index (
                material_key,
                num_games,
                num_positions,
                total_games,
                total_positions,
                material_key_size,
                num_pieces,
                num_pawns,
                num_non_pawns,
                utility
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                "KNvK", 100_i64, 90_i64, 100_i64, 100_i64, 1_i64, 3_i64, 0_i64, 3_i64, 0.9_f64
            ],
        )
        .unwrap();

        let target = compute_generation_targets(&db, 1, 5)
            .unwrap()
            .into_iter()
            .next()
            .expect("expected a generation target");
        assert_eq!(target.material_key.to_string(), "KQvKR");
        assert!(target.is_stale);

        fs::remove_file(&db_path).unwrap();
    }

    fn seed_wdl_table(conn: &Connection, material_key: &str) {
        conn.execute(
            "INSERT INTO wdl_tables (material_key, payload) VALUES (?1, X'00')",
            [material_key],
        )
        .unwrap();
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("heisenbase-{label}-{unique}.db"))
    }
}
