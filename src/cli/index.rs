use anyhow::Result;

use heisenbase::storage::Database;

pub fn run_index_init() -> Result<()> {
    let _ = Database::open_default()?;
    Ok(())
}
