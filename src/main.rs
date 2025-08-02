mod material_key;
mod score;
mod table_builder;

use material_key::MaterialKey;
use table_builder::TableBuilder;

fn main() {
    let material = MaterialKey::from_string("KQvK").unwrap();

    let mut table_builder = TableBuilder::new(material);

    table_builder.solve();
}
