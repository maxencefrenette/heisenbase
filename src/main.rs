mod table_builder;

use shakmaty::Piece;
use table_builder::TableBuilder;

fn main() {
    let mut table_builder = TableBuilder::new(vec![
        Piece::from_char('K').unwrap(),
        Piece::from_char('Q').unwrap(),
        Piece::from_char('k').unwrap(),
    ]);

    table_builder.solve();
}
