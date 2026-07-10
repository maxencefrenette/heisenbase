use anyhow::Result;
use clap::Args;
use heisenbase::storage::Database;

#[derive(Args)]
#[command(
    about = "Show exact-label coverage in the current tablebase",
    long_about = "Show exact Win/Draw/Loss label coverage overall and by piece count, together with bounded, unknown, illegal, and material-level PGN coverage."
)]
pub(crate) struct CoverageArgs {}

pub(crate) fn run(_: CoverageArgs) -> Result<()> {
    let db = Database::open_default()?;
    let rows = load_coverage(&db)?;
    let total = rows.iter().fold(CoverageRow::default(), |mut total, row| {
        total.tables += row.tables;
        total.total_positions += row.total_positions;
        total.illegal_positions += row.illegal_positions;
        total.exact_labels += row.exact_labels;
        total.bounded_positions += row.bounded_positions;
        total.unknown_positions += row.unknown_positions;
        total.pgn_position_fraction += row.pgn_position_fraction;
        total
    });

    println!("generated_tables: {}", total.tables);
    println!("tablebase.total_positions: {}", total.total_positions);
    println!("tablebase.legal_positions: {}", total.legal_positions());
    println!("training.exact_labels: {}", total.exact_labels);
    println!(
        "training.exact_label_fraction_of_legal: {}",
        format_ratio(total.exact_labels, total.legal_positions())
    );
    println!("tablebase.bounded_positions: {}", total.bounded_positions);
    println!("tablebase.unknown_positions: {}", total.unknown_positions);
    println!("tablebase.illegal_positions: {}", total.illegal_positions);
    println!(
        "coverage.generated_pgn_position_fraction: {}",
        format_float(total.pgn_position_fraction)
    );
    println!(
        "num_pieces\ttables\ttotal_positions\tlegal_positions\texact_labels\tbounded_positions\tunknown_positions\tillegal_positions\texact_label_fraction_of_legal\tpgn_position_fraction"
    );

    for row in rows {
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            row.num_pieces,
            row.tables,
            row.total_positions,
            row.legal_positions(),
            row.exact_labels,
            row.bounded_positions,
            row.unknown_positions,
            row.illegal_positions,
            format_ratio(row.exact_labels, row.legal_positions()),
            format_float(row.pgn_position_fraction),
        );
    }

    Ok(())
}

fn load_coverage(db: &Database) -> Result<Vec<CoverageRow>> {
    let mut stmt = db.conn().prepare(
        "SELECT
            m.num_pieces,
            COUNT(*),
            COALESCE(SUM(m.total), 0),
            COALESCE(SUM(m.illegal), 0),
            COALESCE(SUM(m.win + m.draw + m.loss), 0),
            COALESCE(SUM(m.win_or_draw + m.draw_or_loss), 0),
            COALESCE(SUM(m.unknown), 0),
            COALESCE(SUM(p.utility), 0.0)
         FROM material_keys m
         LEFT JOIN pgn_index p ON p.material_key = m.name
         GROUP BY m.num_pieces
         ORDER BY m.num_pieces",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CoverageRow {
            num_pieces: row.get(0)?,
            tables: row.get(1)?,
            total_positions: row.get(2)?,
            illegal_positions: row.get(3)?,
            exact_labels: row.get(4)?,
            bounded_positions: row.get(5)?,
            unknown_positions: row.get(6)?,
            pgn_position_fraction: row.get(7)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[derive(Default)]
struct CoverageRow {
    num_pieces: i64,
    tables: i64,
    total_positions: i64,
    illegal_positions: i64,
    exact_labels: i64,
    bounded_positions: i64,
    unknown_positions: i64,
    pgn_position_fraction: f64,
}

impl CoverageRow {
    fn legal_positions(&self) -> i64 {
        self.total_positions - self.illegal_positions
    }
}

fn format_ratio(numerator: i64, denominator: i64) -> String {
    if denominator == 0 {
        return "0.000000".to_string();
    }
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn format_float(value: f64) -> String {
    format!("{value:.12e}")
}

#[cfg(test)]
mod tests {
    use super::load_coverage;
    use heisenbase::storage::{Database, MaterialStatsRow, PgnIndexRow};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn groups_exact_label_coverage_by_piece_count() {
        let db_path = temp_db_path("coverage");
        let mut db = Database::open_at(&db_path).unwrap();
        db.upsert_material_stats(&material_row("KQvK", 3, 100, 10, 20, 10, 5, 15, 10, 30))
            .unwrap();
        db.upsert_material_stats(&material_row("KQvKR", 5, 200, 20, 40, 20, 10, 30, 20, 60))
            .unwrap();
        db.replace_pgn_index(&[
            pgn_row("KQvK", 3, 100, 0.25),
            pgn_row("KQvKR", 5, 200, 0.10),
        ])
        .unwrap();

        let rows = load_coverage(&db).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].num_pieces, 3);
        assert_eq!(rows[0].legal_positions(), 90);
        assert_eq!(rows[0].exact_labels, 35);
        assert_eq!(rows[0].bounded_positions, 25);
        assert_eq!(rows[0].unknown_positions, 30);
        assert_eq!(rows[0].pgn_position_fraction, 0.25);
        assert_eq!(rows[1].num_pieces, 5);
        assert_eq!(rows[1].exact_labels, 70);

        drop(db);
        fs::remove_file(&db_path).unwrap();
    }

    #[allow(clippy::too_many_arguments)]
    fn material_row(
        name: &str,
        num_pieces: i64,
        total: i64,
        illegal: i64,
        win: i64,
        draw: i64,
        loss: i64,
        win_or_draw: i64,
        draw_or_loss: i64,
        unknown: i64,
    ) -> MaterialStatsRow {
        MaterialStatsRow {
            name: name.to_string(),
            children: Vec::new(),
            num_pieces,
            num_pawns: 0,
            num_non_pawns: num_pieces,
            total,
            illegal,
            win,
            draw,
            loss,
            win_or_draw,
            draw_or_loss,
            unknown,
            updated_at: 0,
        }
    }

    fn pgn_row(
        material_key: &str,
        num_pieces: i64,
        material_key_size: i64,
        utility: f64,
    ) -> PgnIndexRow {
        PgnIndexRow {
            material_key: material_key.to_string(),
            num_games: 1,
            num_positions: 1,
            total_games: 1,
            total_positions: 1,
            material_key_size,
            num_pieces,
            num_pawns: 0,
            num_non_pawns: num_pieces,
            utility,
        }
    }

    fn temp_db_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("heisenbase-{label}-{unique}.db"))
    }
}
