use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::material_key::MaterialKey;
use crate::wdl_table::WdlTable;

const MAGIC: &[u8; 4] = b"HBWD";
const VERSION: u8 = 1;

/// Write a compressed WDL table to a file.
pub fn write_wdl_file<P: AsRef<Path>>(path: P, wdl_table: &WdlTable) -> io::Result<()> {
    let mut file = File::create(path)?;

    // Header
    file.write_all(MAGIC)?;
    file.write_all(&[VERSION])?;

    // Material key
    let mk_string = wdl_table.material.to_string();
    file.write_all(&[mk_string.len() as u8])?;
    file.write_all(mk_string.as_bytes())?;

    // WdlTable
    file.write_all(&wdl_table.positions.len().to_le_bytes())?;
    file.write_all(
        wdl_table
            .positions
            .iter()
            .map(|&wdl| wdl.into())
            .collect::<Vec<u8>>()
            .as_slice(),
    )?;

    Ok(())
}

/// Read a compressed WDL table from a file.
pub fn read_wdl_file<P: AsRef<Path>>(path: P) -> io::Result<WdlTable> {
    let mut file = File::open(path)?;

    // Header
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }

    let mut version = [0u8; 1];
    file.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported version",
        ));
    }

    // Material key
    let mut mk_len = [0u8; 1];
    file.read_exact(&mut mk_len)?;
    let mk_len = mk_len[0] as usize;
    let mut mk_bytes = vec![0u8; mk_len];
    file.read_exact(&mut mk_bytes)?;
    let mk_string = String::from_utf8(mk_bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid material key"))?;
    let material = MaterialKey::from_string(&mk_string)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid material key"))?;

    // WDL Table
    let mut buf = [0u8; 8];
    file.read_exact(&mut buf)?;
    let wdl_table_len = u64::from_le_bytes(buf) as usize;

    let mut buf = vec![0u8; wdl_table_len];
    file.read_exact(&mut buf)?;
    let positions = buf.iter().map(|&num| num.try_into().unwrap()).collect();

    Ok(WdlTable {
        material,
        positions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn read_wdl_file_rejects_bad_magic() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("heisenbase_bad_magic_{unique}.hbt"));

        {
            let mut file = File::create(&path).expect("failed to create temporary file");
            file.write_all(b"BAD!")
                .expect("failed to write incorrect magic to temporary file");
            file.write_all(&[0u8; 16])
                .expect("failed to write placeholder data to temporary file");
        }

        let result = read_wdl_file(&path);
        std::fs::remove_file(&path).expect("failed to remove temporary file");

        assert!(matches!(
            result,
            Err(ref e) if e.kind() == io::ErrorKind::InvalidData
        ));
    }
}
