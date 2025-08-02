mod material_key;
mod score;
mod table_builder;

use material_key::MaterialKey;
use shakmaty::Piece;
use table_builder::TableBuilder;

fn main() {
    let material = MaterialKey::new(vec![
        Piece::from_char('K').unwrap(),
        Piece::from_char('Q').unwrap(),
        Piece::from_char('k').unwrap(),
    ]);

    let mut table_builder = TableBuilder::new(material);

    table_builder.solve();
}
