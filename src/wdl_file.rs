use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::compression::CompressedWdl;
use crate::material_key::MaterialKey;

const MAGIC: &[u8; 4] = b"HBWD";
const VERSION: u8 = 2;

/// Write a compressed WDL table to a file.
pub fn write_wdl_file<P: AsRef<Path>>(
    path: P,
    material: &MaterialKey,
    data: &CompressedWdl,
) -> io::Result<()> {
    let mut file = File::create(path)?;

    // Header
    file.write_all(MAGIC)?;
    file.write_all(&[VERSION])?;

    // Material key
    let mk_string = material.to_string();
    let mk_len = mk_string.len();
    if mk_len > u8::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "material key too long",
        ));
    }
    file.write_all(&[mk_len as u8])?;
    file.write_all(mk_string.as_bytes())?;

    // Table metadata
    file.write_all(&(data.orig_len as u64).to_le_bytes())?;
    file.write_all(&data.base_symbols.to_le_bytes())?;

    // Symbol pairs
    file.write_all(&(data.sym_pairs.len() as u16).to_le_bytes())?;
    for &(a, b) in &data.sym_pairs {
        file.write_all(&a.to_le_bytes())?;
        file.write_all(&b.to_le_bytes())?;
    }

    // Code lengths
    file.write_all(&(data.code_lens.len() as u16).to_le_bytes())?;
    file.write_all(&data.code_lens)?;

    // Bitstream
    file.write_all(&(data.bit_len as u64).to_le_bytes())?;
    file.write_all(&(data.bitstream.len() as u32).to_le_bytes())?;
    file.write_all(&data.bitstream)?;

    Ok(())
}

/// Read a compressed WDL table from a file.
pub fn read_wdl_file<P: AsRef<Path>>(path: P) -> io::Result<(MaterialKey, CompressedWdl)> {
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

    // Table metadata
    let mut buf8 = [0u8; 8];
    file.read_exact(&mut buf8)?;
    let orig_len = u64::from_le_bytes(buf8) as usize;

    let mut buf2 = [0u8; 2];
    file.read_exact(&mut buf2)?;
    let base_symbols = u16::from_le_bytes(buf2);

    // Symbol pairs
    file.read_exact(&mut buf2)?;
    let pair_len = u16::from_le_bytes(buf2) as usize;
    let mut sym_pairs = Vec::with_capacity(pair_len);
    for _ in 0..pair_len {
        file.read_exact(&mut buf2)?;
        let a = u16::from_le_bytes(buf2);
        file.read_exact(&mut buf2)?;
        let b = u16::from_le_bytes(buf2);
        sym_pairs.push((a, b));
    }

    // Code lengths
    file.read_exact(&mut buf2)?;
    let lens_len = u16::from_le_bytes(buf2) as usize;
    let mut code_lens = vec![0u8; lens_len];
    file.read_exact(&mut code_lens)?;

    // Bitstream
    file.read_exact(&mut buf8)?;
    let bit_len = u64::from_le_bytes(buf8) as usize;
    let mut buf4 = [0u8; 4];
    file.read_exact(&mut buf4)?;
    let bs_len = u32::from_le_bytes(buf4) as usize;
    let mut bitstream = vec![0u8; bs_len];
    file.read_exact(&mut bitstream)?;

    Ok((
        material,
        CompressedWdl {
            base_symbols,
            sym_pairs,
            code_lens,
            bitstream,
            bit_len,
            orig_len,
        },
    ))
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
