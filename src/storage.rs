use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use rusqlite::{Connection, OptionalExtension, params};

use crate::material_key::MaterialKey;
use crate::wdl_file::{decode_wdl_bytes, encode_wdl_bytes};
use crate::wdl_table::WdlTable;

pub const DATA_DIR: &str = "./data";
pub const DB_PATH: &str = "./data/heisenbase.db";

pub fn open_database() -> Result<Connection> {
    open_database_at_path(Path::new(DB_PATH))
}

pub fn open_database_at_path(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create data directory {}", parent.display()))?;
    }
    let conn = Connection::open(path)
        .with_context(|| format!("failed to open sqlite database {}", path.display()))?;
    init_db(&conn)?;
    Ok(conn)
}

pub fn init_db(conn: &Connection) -> Result<()> {
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

pub fn store_wdl_table(conn: &Connection, table: &WdlTable) -> Result<()> {
    let payload = encode_wdl_bytes(table)?;
    conn.execute(
        "INSERT INTO wdl_tables (material_key, payload)
         VALUES (?1, ?2)
         ON CONFLICT(material_key) DO UPDATE SET payload = excluded.payload",
        params![table.material.to_string(), payload],
    )?;
    Ok(())
}

pub fn load_wdl_table(conn: &Connection, material: &MaterialKey) -> Result<Option<WdlTable>> {
    let payload: Option<Vec<u8>> = conn
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

pub fn has_wdl_table(conn: &Connection, material: &MaterialKey) -> Result<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM wdl_tables WHERE material_key = ?1 LIMIT 1",
            params![material.to_string()],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

pub fn list_wdl_table_keys(conn: &Connection) -> Result<Vec<MaterialKey>> {
    let mut stmt = conn.prepare("SELECT material_key FROM wdl_tables ORDER BY material_key ASC")?;
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
