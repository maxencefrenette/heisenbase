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

    Ok(())
}
