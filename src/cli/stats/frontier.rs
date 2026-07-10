use anyhow::Result;
use clap::Args;
use heisenbase::generation_targets::compute_generation_targets;
use heisenbase::storage::Database;

#[derive(Args)]
#[command(
    about = "Show the next generate-many targets",
    long_about = "Show the material keys that generate-many would process next, using the same stale-table and transitive-utility ranking pipeline as generation."
)]
pub(crate) struct FrontierArgs {
    /// Maximum number of generation targets to print.
    #[arg(long, default_value_t = 20)]
    limit: usize,
    /// Minimum number of indexed PGN games required.
    #[arg(long, default_value_t = 100)]
    min_games: u64,
    /// Maximum total number of pieces allowed.
    #[arg(long)]
    max_pieces: u32,
}

pub(crate) fn run(args: FrontierArgs) -> Result<()> {
    let db = Database::open_default()?;
    let targets = compute_generation_targets(&db, args.min_games, args.max_pieces)?;

    println!("limit: {}", args.limit);
    println!("min_games: {}", args.min_games);
    println!("max_pieces: {}", args.max_pieces);
    println!(
        "material_key\taction\tis_stale\tnum_games\tmaterial_key_size\tutility\tutility_rank\ttransitive_utility\ttransitive_utility_rank"
    );

    for target in targets.into_iter().take(args.limit) {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            target.material_key,
            if target.is_stale {
                "regenerate"
            } else {
                "generate"
            },
            target.is_stale,
            target.num_games,
            target.material_key_size,
            format_float(target.utility),
            format_float(target.utility_rank),
            format_float(target.transitive_utility),
            format_float(target.transitive_utility_rank),
        );
    }

    Ok(())
}

fn format_float(value: f64) -> String {
    format!("{value:.12e}")
}
