use heisenbase::{
    compression::{compress_wdl, decompress_wdl},
    material_key::MaterialKey,
    wdl_score_range::WdlScoreRange,
    wdl_table::WdlTable,
};
use shakmaty::Position;

#[test]
fn compress_decompress_kqvk_table_round_trip() {
    let material = MaterialKey::from_string("KQvK").unwrap();
    let total = material.total_positions();
    let mut positions = Vec::with_capacity(total);
    for idx in 0..total {
        if let Some(position) = material.index_to_position(idx) {
            let wdl = if position.is_checkmate() {
                WdlScoreRange::Loss
            } else if position.is_stalemate() || position.is_insufficient_material() {
                WdlScoreRange::Draw
            } else {
                WdlScoreRange::Unknown
            };
            positions.push(wdl);
        }
    }
    let table = WdlTable {
        material,
        positions,
    };
    let compressed = compress_wdl(&table.positions);
    let decompressed = decompress_wdl(&compressed);
    assert_eq!(decompressed, table.positions);
}
