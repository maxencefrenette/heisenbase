use crate::material_key::MaterialKey;
use crate::table_builder::TableBuilder;
use crate::wdl_score_range::WdlScoreRange;

pub struct WdlTable {
    pub material: MaterialKey,
    pub positions: Vec<WdlScoreRange>,
}

impl From<TableBuilder> for WdlTable {
    fn from(tb: TableBuilder) -> Self {
        let positions = tb.positions.into_iter().map(WdlScoreRange::from).collect();

        Self {
            material: tb.material,
            positions,
        }
    }
}
