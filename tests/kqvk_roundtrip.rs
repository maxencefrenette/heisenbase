// These tests cover the full WDL roundtrip pipeline: one isolates the in-memory
// compression codec, the other (ignored) layers the file format and metadata on top.
use heisenbase::{
    compression::{compress_wdl, decompress_wdl},
    material_key::MaterialKey,
    position_map::{index_to_position, total_positions},
    wdl_file::{read_wdl_file, write_wdl_file},
    wdl_score_range::WdlScoreRange,
    wdl_table::WdlTable,
};
use shakmaty::Position;
use std::fs;

#[test]
#[ignore]
fn compress_decompress_kqvk_table_round_trip() {
    let material = MaterialKey::from_string("KQvK").unwrap();
    let total = total_positions(&material);
    let mut positions = Vec::with_capacity(total);
    for idx in 0..total {
        let wdl = match index_to_position(&material, idx) {
            Ok(position) => {
                if position.is_checkmate() {
                    WdlScoreRange::Loss
                } else if position.is_stalemate() || position.is_insufficient_material() {
                    WdlScoreRange::Draw
                } else {
                    WdlScoreRange::Unknown
                }
            }
            Err(_) => WdlScoreRange::Unknown,
        };
        positions.push(wdl);
    }
    let table = WdlTable {
        material,
        positions,
    };
    let compressed = compress_wdl(&table.positions);
    let decompressed = decompress_wdl(&compressed);
    assert_eq!(decompressed, table.positions);
}

#[test]
#[ignore]
fn write_read_round_trip() {
    let material = MaterialKey::from_string("KQvK").unwrap();
    let total = total_positions(&material);
    let mut positions = Vec::with_capacity(total);
    for idx in 0..total {
        let wdl = match index_to_position(&material, idx) {
            Ok(position) => {
                if position.is_checkmate() {
                    WdlScoreRange::Loss
                } else if position.is_stalemate() || position.is_insufficient_material() {
                    WdlScoreRange::Draw
                } else {
                    WdlScoreRange::Unknown
                }
            }
            Err(_) => WdlScoreRange::Unknown,
        };
        positions.push(wdl);
    }

    let table = WdlTable {
        material,
        positions,
    };

    let compressed = compress_wdl(&table.positions);

    let mut path = std::env::temp_dir();
    path.push("kqvk_test.hbt");
    write_wdl_file(&path, &table.material, &compressed).unwrap();

    let (read_material, read_data) = read_wdl_file(&path).unwrap();
    assert_eq!(read_material, table.material);
    let decompressed = decompress_wdl(&read_data);
    assert_eq!(decompressed, table.positions);

    fs::remove_file(path).unwrap();
}
