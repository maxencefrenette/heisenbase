use duckdb::Connection;
use std::{fs, path::Path};

use anyhow::Result;

pub fn run_index_init() -> Result<()> {
    let heisenbase_dir = Path::new("./data/heisenbase");
    fs::create_dir_all(heisenbase_dir)?;

    let db_path = heisenbase_dir.join("index.duckdb");
    let conn = Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS material_keys (
            name VARCHAR,
            children VARCHAR[],
            num_pieces INTEGER,
            num_pawns INTEGER,
            num_non_pawns INTEGER,
            total INTEGER,
            illegal INTEGER,
            win INTEGER,
            draw INTEGER,
            loss INTEGER,
            win_or_draw INTEGER,
            draw_or_loss INTEGER,
            unknown INTEGER
        )",
        [],
    )?;
    conn.execute(
        "ALTER TABLE material_keys ADD COLUMN IF NOT EXISTS num_pieces INTEGER",
        [],
    )?;
    conn.execute(
        "ALTER TABLE material_keys ADD COLUMN IF NOT EXISTS num_pawns INTEGER",
        [],
    )?;
    conn.execute(
        "ALTER TABLE material_keys ADD COLUMN IF NOT EXISTS num_non_pawns INTEGER",
        [],
    )?;

    Ok(())
}
