use crate::compression::decompress_wdl;
use crate::material_key::{MaterialError, MaterialKey};
use crate::position_map::PositionIndexer;
use crate::score::DtzScoreRange;
use crate::wdl_file::read_wdl_file;
use crate::wdl_score_range::WdlScoreRange;
use shakmaty::{Chess, Move, Position};
use std::collections::HashMap;
use std::path::Path;

pub struct TableBuilder {
    pub(crate) material: MaterialKey,
    pub(crate) position_indexer: PositionIndexer,
    pub(crate) positions: Vec<DtzScoreRange>,
    pub(crate) child_tables: HashMap<MaterialKey, Vec<WdlScoreRange>>,
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
        let mut loaded_child_tables = Vec::new();
        let mut missing_child_tables = Vec::new();

        for child_key in material.child_material_keys() {
            let path = data_dir.join(format!("{}.hbt", child_key));
            match read_wdl_file(&path) {
                Ok((file_key, compressed)) => {
                    if file_key != child_key {
                        // Skip mismatched tables but record them as missing to avoid surprises.
                        missing_child_tables.push(child_key);
                        continue;
                    }
                    let table = decompress_wdl(&compressed);
                    child_tables.insert(child_key.clone(), table);
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
            loaded_child_tables,
            missing_child_tables,
        }
    }

    pub fn solve(&mut self) {
        const MAX_STEPS: usize = 101;

        for step in 0..MAX_STEPS {
            let updates = self.step();
            println!("Step {}: {} updates", step + 1, updates);
            if updates == 0 {
                break;
            }
            if step == MAX_STEPS - 1 {
                panic!("table build exceeded {} steps", MAX_STEPS);
            }
        }
    }

    /// Perform one iteration of the table builder.
    ///
    /// This performs one bellman update on every position in the table and returns the number of
    /// positions that changed.
    fn step(&mut self) -> usize {
        let mut updates = 0;
        for pos_index in 0..self.positions.len() {
            let old_score = self.positions[pos_index];

            if old_score.is_illegal() {
                continue;
            }

            if old_score.is_certain() {
                continue;
            }

            let position = match self.position_indexer.index_to_position(pos_index) {
                Ok(p) => p,
                Err(MaterialError::InvalidPosition(_)) => {
                    if !self.positions[pos_index].is_illegal() {
                        self.positions[pos_index] = DtzScoreRange::illegal();
                        updates += 1;
                    }
                    continue;
                }
                Err(MaterialError::IndexOutOfBounds) => {
                    debug_assert!(false, "index {} unexpectedly out of bounds", pos_index);
                    continue;
                }
                Err(MaterialError::MismatchedMaterial) => {
                    debug_assert!(false, "index {} has mismatched material", pos_index);
                    continue;
                }
            };

            if position.is_checkmate() {
                self.positions[pos_index] = DtzScoreRange::checkmate();
                updates += 1;
                continue;
            }

            if position.is_stalemate() || position.is_insufficient_material() {
                self.positions[pos_index] = DtzScoreRange::draw();
                updates += 1;
                continue;
            }

            let new_score = position
                .legal_moves()
                .into_iter()
                .map(|mv| self.evaluate_move(&position, mv).flip())
                .reduce(|a, b| a.max(&b))
                .expect("every non-terminal position should have at least one legal move");

            if new_score != old_score {
                self.positions[pos_index] = new_score;
                updates += 1;
            }
        }
        updates
    }

    fn evaluate_move(&self, position: &Chess, mv: Move) -> DtzScoreRange {
        let mut child_position = position.clone();
        child_position.play_unchecked(mv);

        let is_promotion = mv.promotion().is_some();

        if !mv.is_capture() && !is_promotion {
            let child_index = self
                .position_indexer
                .position_to_index(&child_position)
                .unwrap();
            self.positions[child_index].add_half_move()
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
            if let Some(table) = self.child_tables.get(&child_key) {
                let child_indexer = PositionIndexer::new(child_key.clone());
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
    use crate::compression::compress_wdl;
    use crate::position_map::PositionIndexer;
    use crate::wdl_file::write_wdl_file;
    use crate::wdl_score_range::WdlScoreRange;
    use shakmaty::{CastlingMode, fen::Fen};
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

        tb.step();

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

        // First step marks the checkmate child.
        tb.step();
        // Second step propagates to the parent position.
        tb.step();

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
        let compressed = compress_wdl(&positions);
        let path = data_dir.join("KvK.hbt");
        write_wdl_file(&path, &child_key, &compressed).unwrap();

        let tb = TableBuilder::new_with_data_dir(material, &data_dir);
        let loaded: Vec<String> = tb
            .loaded_child_materials()
            .iter()
            .map(|k| k.to_string())
            .collect();
        assert_eq!(loaded, vec!["KvK".to_string()]);
        assert!(tb.missing_child_materials().is_empty());

        fs::remove_file(path).unwrap();
        fs::remove_dir_all(data_dir).unwrap();
    }
}
