use std::io::{self, Cursor, Read};

use crate::material_key::MaterialKey;
use crate::wdl_table::WdlTable;

const MAGIC: &[u8; 4] = b"HBWD";
const VERSION: u8 = 1;

pub fn encode_wdl_bytes(wdl_table: &WdlTable) -> io::Result<Vec<u8>> {
    let mk_string = wdl_table.material.to_string();
    if mk_string.len() > u8::MAX as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "material key is too long to encode",
        ));
    }

    let mut bytes = Vec::with_capacity(4 + 1 + 1 + mk_string.len() + 8 + wdl_table.positions.len());
    bytes.extend_from_slice(MAGIC);
    bytes.push(VERSION);
    bytes.push(mk_string.len() as u8);
    bytes.extend_from_slice(mk_string.as_bytes());
    bytes.extend_from_slice(&(wdl_table.positions.len() as u64).to_le_bytes());
    bytes.extend(wdl_table.positions.iter().map(|&wdl| u8::from(wdl)));
    Ok(bytes)
}

pub fn decode_wdl_bytes(bytes: &[u8]) -> io::Result<WdlTable> {
    let mut cursor = Cursor::new(bytes);

    let mut magic = [0u8; 4];
    cursor.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid magic"));
    }

    let mut version = [0u8; 1];
    cursor.read_exact(&mut version)?;
    if version[0] != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported version",
        ));
    }

    let mut mk_len = [0u8; 1];
    cursor.read_exact(&mut mk_len)?;
    let mut mk_bytes = vec![0u8; mk_len[0] as usize];
    cursor.read_exact(&mut mk_bytes)?;
    let mk_string = String::from_utf8(mk_bytes)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid material key"))?;
    let material = MaterialKey::from_string(&mk_string)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid material key"))?;

    let mut len_buf = [0u8; 8];
    cursor.read_exact(&mut len_buf)?;
    let table_len = u64::from_le_bytes(len_buf) as usize;

    let mut positions_buf = vec![0u8; table_len];
    cursor.read_exact(&mut positions_buf)?;
    let positions = positions_buf
        .into_iter()
        .map(|num| num.try_into().unwrap())
        .collect();

    Ok(WdlTable {
        material,
        positions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::material_key::MaterialKey;
    use crate::position_indexer::PositionIndexer;
    use crate::wdl_score_range::WdlScoreRange;

    #[test]
    fn read_wdl_file_rejects_bad_magic() {
        let result = decode_wdl_bytes(b"BAD!\0\0\0\0\0\0\0\0\0");
        assert!(matches!(
            result,
            Err(ref e) if e.kind() == io::ErrorKind::InvalidData
        ));
    }

    #[test]
    fn encode_decode_round_trip() {
        let material = MaterialKey::from_string("KQvK").unwrap();
        let positions =
            vec![WdlScoreRange::Draw; PositionIndexer::new(material.clone()).total_positions()];
        let table = WdlTable {
            material,
            positions,
        };
        let bytes = encode_wdl_bytes(&table).unwrap();
        let decoded = decode_wdl_bytes(&bytes).unwrap();
        assert_eq!(decoded, table);
    }
}
