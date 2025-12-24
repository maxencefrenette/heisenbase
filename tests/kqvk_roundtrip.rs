// These tests cover the full WDL roundtrip pipeline: one isolates the in-memory
// compression codec, the other (ignored) layers the file format and metadata on top.
use heisenbase::{
    material_key::MaterialKey,
    position_indexer::PositionIndexer,
    wdl_file::{read_wdl_file, write_wdl_file},
    wdl_score_range::WdlScoreRange,
    wdl_table::WdlTable,
};
use shakmaty::Position;
use std::fs;

fn build_kqvk_table_one_iteration() -> WdlTable {
    let material = MaterialKey::from_string("KQvK").unwrap();
    let indexer = PositionIndexer::new(material.clone());
    let total = indexer.total_positions();
    let mut positions = Vec::with_capacity(total);
    for idx in 0..total {
        let wdl = match indexer.index_to_position(idx) {
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

    WdlTable {
        material,
        positions,
    }
}

#[test]
#[ignore]
fn write_read_round_trip() {
    let table = build_kqvk_table_one_iteration();

    let mut path = std::env::temp_dir();
    path.push("kqvk_test.hbt");
    write_wdl_file(&path, &table).unwrap();

    let read_table = read_wdl_file(&path).unwrap();
    assert_eq!(read_table, table);

    fs::remove_file(path).unwrap();
}
