use crate::material_key::MaterialKey;
use crate::position_indexer::{PositionIndexer, PositionMappingError};
use crate::score::DtzScoreRange;
use crate::wdl_file::read_wdl_file;
use crate::wdl_score_range::WdlScoreRange;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use shakmaty::{Chess, Move, Position, Role};
use std::collections::HashMap;
use std::path::Path;

pub struct TableBuilder {
    pub(crate) material: MaterialKey,
    pub(crate) position_indexer: PositionIndexer,
    pub(crate) positions: Vec<DtzScoreRange>,
    pub(crate) child_tables: HashMap<MaterialKey, Vec<WdlScoreRange>>,
    pub(crate) child_indexers: HashMap<MaterialKey, PositionIndexer>,
    pub(crate) loaded_child_tables: Vec<MaterialKey>,
    pub(crate) missing_child_tables: Vec<MaterialKey>,
}

impl TableBuilder {
    pub fn new(material: MaterialKey) -> Self {
        Self::with_data_dir(material, Path::new("./data/heisenbase"))
    }

    #[cfg(test)]
    pub(crate) fn new_with_data_dir<P: AsRef<Path>>(material: MaterialKey, data_dir: P) -> Self {
        Self::with_data_dir(material, data_dir.as_ref())
    }

    fn with_data_dir(material: MaterialKey, data_dir: &Path) -> Self {
        let position_indexer = PositionIndexer::new(material.clone());
        let positions_len = position_indexer.total_positions();
        let mut child_tables = HashMap::new();
        let mut child_indexers = HashMap::new();
        let mut loaded_child_tables = Vec::new();
        let mut missing_child_tables = Vec::new();

        for child_key in material.child_material_keys() {
            let path = data_dir.join(format!("{}.hbt", child_key));
            match read_wdl_file(&path) {
                Ok(table) => {
                    child_tables.insert(child_key.clone(), table.positions);
                    child_indexers
                        .entry(child_key.clone())
                        .or_insert_with(|| PositionIndexer::new(child_key.clone()));
                    loaded_child_tables.push(child_key);
                }
                Err(_) => {
                    missing_child_tables.push(child_key);
                }
            }
        }

        Self {
            material,
            position_indexer,
            positions: vec![DtzScoreRange::unknown(); positions_len],
            child_tables,
            child_indexers,
            loaded_child_tables,
            missing_child_tables,
        }
    }

    pub fn solve(&mut self) {
        const MAX_STEPS: usize = 101;
        let mut positions_next = vec![DtzScoreRange::unknown(); self.positions.len()];

        for it in 0..MAX_STEPS {
            let progress_bar = self.create_iteration_progress_bar(it + 1);
            let updates;
            (updates, positions_next) = self.step(positions_next, progress_bar);
            println!("Iteration {:>3}: {} updates", it + 1, updates);

            if updates == 0 {
                break;
            }
            if it == MAX_STEPS - 1 {
                panic!("table build exceeded {} iterations", MAX_STEPS);
            }
        }
    }

    fn create_iteration_progress_bar(&self, iteration: usize) -> ProgressBar {
        let total_positions = self.positions.len() as u64;
        let progress = ProgressBar::new(total_positions);
        let style = ProgressStyle::with_template(
            "{msg} {bar:40.cyan/blue} {pos}/{len} [{elapsed_precise}<{eta_precise}]",
        )
        .unwrap();
        progress.set_style(style);
        progress.set_message(format!("Iteration {iteration:>3}"));
        progress
    }

    fn step(
        &mut self,
        mut positions_next: Vec<DtzScoreRange>,
        progress_bar: ProgressBar,
    ) -> (usize, Vec<DtzScoreRange>) {
        let updates = positions_next
            .par_iter_mut()
            .progress_with(progress_bar)
            .enumerate()
            .map(|(pos_index, new_score_cell)| {
                let old_score = self.positions[pos_index];
                let new_score = self.score_position(&self.positions, pos_index);
                *new_score_cell = new_score;

                (new_score != old_score) as usize
            })
            .sum::<usize>();

        std::mem::swap(&mut self.positions, &mut positions_next);
        (updates, positions_next)
    }

    fn score_position(&self, prev_positions: &[DtzScoreRange], pos_index: usize) -> DtzScoreRange {
        let old_score = prev_positions[pos_index];
        if old_score.is_illegal() || old_score.is_certain() {
            return old_score;
        }

        let position = match self.position_indexer.index_to_position(pos_index) {
            Ok(p) => p,
            Err(PositionMappingError::InvalidPosition(_))
            | Err(PositionMappingError::TwoPiecesOnSameSquare) => {
                return DtzScoreRange::illegal();
            }
            Err(PositionMappingError::IndexOutOfBounds) => {
                debug_assert!(false, "index {} unexpectedly out of bounds", pos_index);
                return old_score;
            }
            Err(PositionMappingError::MismatchedMaterial) => {
                debug_assert!(false, "index {} has mismatched material", pos_index);
                return old_score;
            }
        };

        if position.is_checkmate() {
            return DtzScoreRange::checkmate();
        }

        if position.is_stalemate() || position.is_insufficient_material() {
            return DtzScoreRange::draw();
        }

        position
            .legal_moves()
            .into_iter()
            .map(|mv| self.evaluate_move(prev_positions, &position, mv).flip())
            .reduce(|a, b| a.max(&b))
            .expect("every non-terminal position should have at least one legal move")
    }

    fn evaluate_move(
        &self,
        prev_positions: &[DtzScoreRange],
        position: &Chess,
        mv: Move,
    ) -> DtzScoreRange {
        let mut child_position = position.clone();
        child_position.play_unchecked(mv);

        let is_promotion = mv.promotion().is_some();
        let is_pawn_move = mv.role() == Role::Pawn;

        if !mv.is_capture() && !is_promotion && !is_pawn_move {
            let child_index = self
                .position_indexer
                .position_to_index(&child_position)
                .unwrap();
            prev_positions[child_index].add_half_move()
        } else if child_position.is_checkmate() {
            DtzScoreRange::checkmate()
        } else if child_position.is_stalemate() || child_position.is_insufficient_material() {
            DtzScoreRange::draw()
        } else {
            if child_position.is_checkmate() {
                return DtzScoreRange::checkmate();
            }
            if child_position.is_stalemate() || child_position.is_insufficient_material() {
                return DtzScoreRange::draw();
            }

            let child_key = match MaterialKey::from_position(&child_position) {
                Some(key) => key,
                None => return DtzScoreRange::unknown(),
            };
            if let (Some(table), Some(child_indexer)) = (
                self.child_tables.get(&child_key),
                self.child_indexers.get(&child_key),
            ) {
                match child_indexer.position_to_index(&child_position) {
                    Ok(idx) => DtzScoreRange::from(table[idx]),
                    Err(_) => DtzScoreRange::unknown(),
                }
            } else {
                DtzScoreRange::unknown()
            }
        }
    }

    pub fn loaded_child_materials(&self) -> &[MaterialKey] {
        &self.loaded_child_tables
    }

    pub fn missing_child_materials(&self) -> &[MaterialKey] {
        &self.missing_child_tables
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::position_indexer::PositionIndexer;
    use crate::wdl_file::write_wdl_file;
    use crate::wdl_score_range::WdlScoreRange;
    use crate::wdl_table::WdlTable;
    use shakmaty::{CastlingMode, Role, Square, fen::Fen};
    use std::collections::HashMap;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn position_index_roundtrip() {
        let material = MaterialKey::from_string("KQvK").unwrap();
        let tb = TableBuilder {
            material: material.clone(),
            position_indexer: PositionIndexer::new(material),
            positions: Vec::new(),
            child_tables: HashMap::new(),
            child_indexers: HashMap::new(),
            loaded_child_tables: Vec::new(),
            missing_child_tables: Vec::new(),
        };

        let position = "7k/8/8/8/8/8/8/KQ6 w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let index = tb.position_indexer.position_to_index(&position).unwrap();
        let reconstructed = tb
            .position_indexer
            .index_to_position(index)
            .expect("valid position");

        assert_eq!(
            tb.position_indexer
                .position_to_index(&reconstructed)
                .unwrap(),
            index
        );
    }

    #[test]
    fn terminal_positions_scored_in_first_step() {
        let material = MaterialKey::from_string("KQvK").unwrap();
        let mut tb = TableBuilder::new(material);

        // Mark all positions as draws so the step only evaluates the targets.
        tb.positions.fill(DtzScoreRange::draw());

        let checkmate = "k7/1Q6/2K5/8/8/8/8/8 b - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        let stalemate = "k7/8/1QK5/8/8/8/8/8 b - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let checkmate_idx = tb.position_indexer.position_to_index(&checkmate).unwrap();
        let stalemate_idx = tb.position_indexer.position_to_index(&stalemate).unwrap();

        tb.positions[checkmate_idx] = DtzScoreRange::unknown();
        tb.positions[stalemate_idx] = DtzScoreRange::unknown();

        tb.step(
            vec![DtzScoreRange::unknown(); tb.positions.len()],
            ProgressBar::hidden(),
        );

        assert_eq!(tb.positions[checkmate_idx], DtzScoreRange::checkmate());
        assert_eq!(tb.positions[stalemate_idx], DtzScoreRange::draw());
    }

    #[test]
    fn mate_in_one_scored_after_two_steps() {
        let material = MaterialKey::from_string("KQvK").unwrap();
        let mut tb = TableBuilder::new(material);

        // Pre-fill positions with draws so only relevant indices are processed.
        tb.positions.fill(DtzScoreRange::draw());

        let mate_in_one = "k7/8/1QK5/8/8/8/8/8 w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        let idx = tb.position_indexer.position_to_index(&mate_in_one).unwrap();

        // Identify the checkmate position reached after Qb7#.
        let checkmate = "k7/1Q6/2K5/8/8/8/8/8 b - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        let checkmate_idx = tb.position_indexer.position_to_index(&checkmate).unwrap();

        tb.positions[idx] = DtzScoreRange::unknown();
        tb.positions[checkmate_idx] = DtzScoreRange::unknown();

        let mut positions_next = vec![DtzScoreRange::unknown(); tb.positions.len()];
        // First step marks the checkmate child.
        (_, positions_next) = tb.step(positions_next, ProgressBar::hidden());
        // Second step propagates to the parent position.
        tb.step(positions_next, ProgressBar::hidden());

        let wdl: WdlScoreRange = tb.positions[idx].into();
        assert_eq!(wdl, WdlScoreRange::Win);
    }

    fn temp_data_dir(prefix: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!("heisenbase_{prefix}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn child_tables_report_missing_when_unavailable() {
        let material = MaterialKey::from_string("KQvK").unwrap();
        let data_dir = temp_data_dir("missing_child");
        let tb = TableBuilder::new_with_data_dir(material, &data_dir);
        assert!(tb.loaded_child_materials().is_empty());
        let missing: Vec<String> = tb
            .missing_child_materials()
            .iter()
            .map(|k| k.to_string())
            .collect();
        assert_eq!(missing, vec!["KvK".to_string()]);
        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn child_tables_load_available_files() {
        let material = MaterialKey::from_string("KQvK").unwrap();
        let data_dir = temp_data_dir("load_child");

        let child_key = MaterialKey::from_string("KvK").unwrap();
        let child_indexer = PositionIndexer::new(child_key.clone());
        let positions = vec![WdlScoreRange::Draw; child_indexer.total_positions()];
        let kvk_wdl_table = WdlTable {
            material: child_key,
            positions,
        };
        let path = data_dir.join("KvK.hbt");
        write_wdl_file(&path, &kvk_wdl_table).unwrap();

        let tb = TableBuilder::new_with_data_dir(material, &data_dir);
        let loaded: Vec<String> = tb
            .loaded_child_materials()
            .iter()
            .map(|k| k.to_string())
            .collect();
        assert_eq!(loaded, vec!["KvK".to_string()]);
        assert!(tb.missing_child_materials().is_empty());
        assert!(tb.child_indexers.contains_key(&kvk_wdl_table.material));

        fs::remove_file(path).unwrap();
        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn pawn_move_uses_child_table() {
        let material = MaterialKey::from_string("Ka2vK").unwrap();
        let data_dir = temp_data_dir("pawn_move_child");

        let child_key = MaterialKey::from_string("Ka3vK").unwrap();
        let child_indexer = PositionIndexer::new(child_key.clone());
        let positions = vec![WdlScoreRange::Draw; child_indexer.total_positions()];
        let child_table = WdlTable {
            material: child_key,
            positions,
        };
        let child_path = data_dir.join("Ka3vK.hbt");
        write_wdl_file(&child_path, &child_table).unwrap();

        let tb = TableBuilder::new_with_data_dir(material, &data_dir);

        let position: Chess = "8/8/8/8/8/8/P7/K6k w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let pawn_move = position
            .legal_moves()
            .into_iter()
            .find(|mv| mv.role() == Role::Pawn && mv.to() == Square::A3)
            .expect("expected a2a3 pawn move");
        let result = tb.evaluate_move(&tb.positions, &position, pawn_move);
        assert_eq!(result, DtzScoreRange::draw());

        fs::remove_dir_all(data_dir).unwrap();
    }
}
