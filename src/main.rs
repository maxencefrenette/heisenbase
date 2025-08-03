use clap::{Parser, Subcommand};
use heisenbase::compression::compress_wdl;
use heisenbase::material_key::MaterialKey;
use heisenbase::table_builder::TableBuilder;
use heisenbase::wdl_file::write_wdl_file;
use heisenbase::wdl_table::WdlTable;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a table for a given material key.
    Generate {
        /// Material key describing pieces, e.g. `KQvK`.
        material_key: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Generate { material_key } => {
            let material = MaterialKey::from_string(&material_key).expect("invalid material key");
            let mut table_builder = TableBuilder::new(material);
            table_builder.solve();
            let wdl_table: WdlTable = table_builder.into();
            let compressed = compress_wdl(&wdl_table.positions);
            let filename = format!("./data/{}.hbt", wdl_table.material);
            write_wdl_file(&filename, &wdl_table.material, &compressed)
                .expect("failed to write table file");
            println!("Wrote table to {}", filename);
        }
    }
}
