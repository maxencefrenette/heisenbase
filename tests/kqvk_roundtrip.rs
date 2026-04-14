// These tests cover the WDL roundtrip codec directly.
use heisenbase::{
    material_key::MaterialKey,
    position_indexer::PositionIndexer,
    wdl_file::{decode_wdl_bytes, encode_wdl_bytes},
    wdl_score_range::WdlScoreRange,
    wdl_table::WdlTable,
};
use shakmaty::Position;

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
    let bytes = encode_wdl_bytes(&table).unwrap();
    let read_table = decode_wdl_bytes(&bytes).unwrap();
    assert_eq!(read_table, table);
}
