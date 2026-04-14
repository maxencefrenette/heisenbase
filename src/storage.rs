use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};

use crate::material_key::MaterialKey;
use crate::wdl_file::{decode_wdl_bytes, encode_wdl_bytes};
use crate::wdl_table::WdlTable;

pub const DATA_DIR: &str = "./data";
pub const DB_PATH: &str = "./data/heisenbase.db";

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone)]
pub struct MaterialStatsRow {
    pub name: String,
    pub children: Vec<String>,
    pub num_pieces: i64,
    pub num_pawns: i64,
    pub num_non_pawns: i64,
    pub total: i64,
    pub illegal: i64,
    pub win: i64,
    pub draw: i64,
    pub loss: i64,
    pub win_or_draw: i64,
    pub draw_or_loss: i64,
    pub unknown: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct PgnIndexRawRow {
    pub material_key: String,
    pub num_games: i64,
    pub num_positions: i64,
    pub total_games: i64,
    pub total_positions: i64,
}

#[derive(Debug, Clone)]
pub struct PgnIndexRow {
    pub material_key: String,
    pub num_games: i64,
    pub num_positions: i64,
    pub total_games: i64,
    pub total_positions: i64,
    pub material_key_size: i64,
    pub num_pieces: i64,
    pub num_pawns: i64,
    pub num_non_pawns: i64,
    pub utility: f64,
}

impl Database {
    pub fn open_default() -> Result<Self> {
        Self::open_at(Path::new(DB_PATH))
    }

    pub fn open_at(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create data directory {}", parent.display()))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open sqlite database {}", path.display()))?;
        init_db(&conn)?;
        Ok(Self { conn })
    }

    pub fn reset_at(path: &Path) -> Result<Self> {
        if path.exists() {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove sqlite database {}", path.display()))?;
        }
        Self::open_at(path)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    pub fn put_wdl_table(&self, table: &WdlTable) -> Result<()> {
        let payload = encode_wdl_bytes(table)?;
        self.conn.execute(
            "INSERT INTO wdl_tables (material_key, payload)
             VALUES (?1, ?2)
             ON CONFLICT(material_key) DO UPDATE SET payload = excluded.payload",
            params![table.material.to_string(), payload],
        )?;
        Ok(())
    }

    pub fn get_wdl_table(&self, material: &MaterialKey) -> Result<Option<WdlTable>> {
        let payload: Option<Vec<u8>> = self
            .conn
            .query_row(
                "SELECT payload FROM wdl_tables WHERE material_key = ?1",
                params![material.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        payload
            .map(|bytes| decode_wdl_bytes(&bytes).map_err(|err| anyhow!(err)))
            .transpose()
    }

    pub fn has_wdl_table(&self, material: &MaterialKey) -> Result<bool> {
        let exists = self
            .conn
            .query_row(
                "SELECT 1 FROM wdl_tables WHERE material_key = ?1 LIMIT 1",
                params![material.to_string()],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
        Ok(exists)
    }

    pub fn list_wdl_table_keys(&self) -> Result<Vec<MaterialKey>> {
        let mut stmt = self
            .conn
            .prepare("SELECT material_key FROM wdl_tables ORDER BY material_key ASC")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut keys = Vec::new();
        for row in rows {
            let key = row?;
            let material = MaterialKey::from_string(&key)
                .map_err(|err| anyhow!("invalid material key in sqlite: {key}: {err}"))?;
            keys.push(material);
        }
        Ok(keys)
    }

    pub fn upsert_material_stats(&self, row: &MaterialStatsRow) -> Result<()> {
        let children_json = serde_json::to_string(&row.children)?;
        self.conn.execute(
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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
                unknown = excluded.unknown,
                updated_at = excluded.updated_at",
            params![
                row.name,
                children_json,
                row.num_pieces,
                row.num_pawns,
                row.num_non_pawns,
                row.total,
                row.illegal,
                row.win,
                row.draw,
                row.loss,
                row.win_or_draw,
                row.draw_or_loss,
                row.unknown,
                row.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn get_material_stats(&self, material: &MaterialKey) -> Result<Option<MaterialStatsRow>> {
        let name = material.to_string();
        self.conn
            .query_row(
                "SELECT
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
                 FROM material_keys
                 WHERE name = ?1",
                [name],
                |row| {
                    let children_json: String = row.get(1)?;
                    let children = serde_json::from_str(&children_json).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            children_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    Ok(MaterialStatsRow {
                        name: row.get(0)?,
                        children,
                        num_pieces: row.get(2)?,
                        num_pawns: row.get(3)?,
                        num_non_pawns: row.get(4)?,
                        total: row.get(5)?,
                        illegal: row.get(6)?,
                        win: row.get(7)?,
                        draw: row.get(8)?,
                        loss: row.get(9)?,
                        win_or_draw: row.get(10)?,
                        draw_or_loss: row.get(11)?,
                        unknown: row.get(12)?,
                        updated_at: row.get(13)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn replace_pgn_index_raw(&mut self, rows: &[PgnIndexRawRow]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM pgn_index_raw", [])?;
        let mut insert = tx.prepare(
            "INSERT INTO pgn_index_raw (
                material_key,
                num_games,
                num_positions,
                total_games,
                total_positions
            ) VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for row in rows {
            insert.execute(params![
                row.material_key,
                row.num_games,
                row.num_positions,
                row.total_games,
                row.total_positions,
            ])?;
        }
        drop(insert);
        tx.commit()?;
        Ok(())
    }

    pub fn replace_pgn_index(&mut self, rows: &[PgnIndexRow]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM pgn_index", [])?;
        let mut insert = tx.prepare(
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
        )?;
        for row in rows {
            insert.execute(params![
                row.material_key,
                row.num_games,
                row.num_positions,
                row.total_games,
                row.total_positions,
                row.material_key_size,
                row.num_pieces,
                row.num_pawns,
                row.num_non_pawns,
                row.utility,
            ])?;
        }
        drop(insert);
        tx.commit()?;
        Ok(())
    }

    pub fn get_pgn_index_row(&self, material: &MaterialKey) -> Result<Option<PgnIndexRow>> {
        let name = material.to_string();
        self.conn
            .query_row(
                "SELECT
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
                 FROM pgn_index
                 WHERE material_key = ?1",
                [name],
                |row| {
                    Ok(PgnIndexRow {
                        material_key: row.get(0)?,
                        num_games: row.get(1)?,
                        num_positions: row.get(2)?,
                        total_games: row.get(3)?,
                        total_positions: row.get(4)?,
                        material_key_size: row.get(5)?,
                        num_pieces: row.get(6)?,
                        num_pawns: row.get(7)?,
                        num_non_pawns: row.get(8)?,
                        utility: row.get(9)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }
}

fn init_db(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS wdl_tables (
            material_key TEXT PRIMARY KEY,
            payload BLOB NOT NULL
        );
        CREATE TABLE IF NOT EXISTS material_keys (
            name TEXT PRIMARY KEY,
            children_json TEXT NOT NULL,
            num_pieces INTEGER NOT NULL,
            num_pawns INTEGER NOT NULL,
            num_non_pawns INTEGER NOT NULL,
            total INTEGER NOT NULL,
            illegal INTEGER NOT NULL,
            win INTEGER NOT NULL,
            draw INTEGER NOT NULL,
            loss INTEGER NOT NULL,
            win_or_draw INTEGER NOT NULL,
            draw_or_loss INTEGER NOT NULL,
            unknown INTEGER NOT NULL,
            updated_at INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS pgn_index_raw (
            material_key TEXT PRIMARY KEY,
            num_games INTEGER NOT NULL,
            num_positions INTEGER NOT NULL,
            total_games INTEGER NOT NULL,
            total_positions INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS pgn_index (
            material_key TEXT PRIMARY KEY,
            num_games INTEGER NOT NULL,
            num_positions INTEGER NOT NULL,
            total_games INTEGER NOT NULL,
            total_positions INTEGER NOT NULL,
            material_key_size INTEGER NOT NULL,
            num_pieces INTEGER NOT NULL,
            num_pawns INTEGER NOT NULL,
            num_non_pawns INTEGER NOT NULL,
            utility REAL NOT NULL
        );",
    )?;
    Ok(())
}
