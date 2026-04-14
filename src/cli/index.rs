use anyhow::Result;

use heisenbase::storage;

pub fn run_index_init() -> Result<()> {
    let _ = storage::open_database()?;
    Ok(())
}
