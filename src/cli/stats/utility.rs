use anyhow::{Result, anyhow};
use clap::Args;
use heisenbase::generation_targets::compute_utility_stats;
use heisenbase::material_key::MaterialKey;
use heisenbase::storage::Database;

#[derive(Args)]
#[command(
    about = "Explain utility for one material key",
    long_about = "Explain direct and transitive utility for one material key, including every solved parent contribution used by the generation scheduler."
)]
pub(crate) struct UtilityArgs {
    /// Material key to explain, e.g. `KQvK`.
    material_key: String,
}

pub(crate) fn run(args: UtilityArgs) -> Result<()> {
    let material = MaterialKey::from_string(&args.material_key)
        .map_err(|err| anyhow!("invalid material key: {}: {}", args.material_key, err))?;
    let db = Database::open_default()?;
    let stats = compute_utility_stats(&db, &material)?;

    println!("material_key: {}", stats.material_key);
    println!("has_table: {}", stats.has_table);
    println!("has_pgn_index: {}", stats.has_pgn_index);
    println!("num_games: {}", stats.num_games);
    println!("material_key_size: {}", stats.material_key_size);
    println!("utility: {}", format_float(stats.utility));
    println!("utility_rank: {}", format_float(stats.utility_rank));
    println!(
        "transitive_utility: {}",
        format_float(stats.transitive_utility)
    );
    println!(
        "transitive_utility_rank: {}",
        format_float(stats.transitive_utility_rank)
    );
    println!("contributing_parents: {}", stats.contributions.len());
    println!(
        "parent\tparent_utility\tparent_total\tparent_unresolved\tunresolved_fraction\tchild_count\tcontribution"
    );

    for contribution in stats.contributions {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}",
            contribution.parent,
            format_float(contribution.parent_utility),
            contribution.parent_total,
            contribution.parent_unresolved,
            format_float(contribution.unresolved_fraction),
            contribution.child_count,
            format_float(contribution.contribution),
        );
    }

    Ok(())
}

fn format_float(value: f64) -> String {
    format!("{value:.12e}")
}
