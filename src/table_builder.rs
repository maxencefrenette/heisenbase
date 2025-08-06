use crate::material_key::MaterialKey;
use crate::score::DtzScoreRange;
use shakmaty::{Chess, Move, Position};

pub struct TableBuilder {
    pub(crate) material: MaterialKey,
    pub(crate) positions: Vec<DtzScoreRange>,
}

impl TableBuilder {
    pub fn new(material: MaterialKey) -> Self {
        let positions = material.total_positions();

        Self {
            material,
            positions: vec![DtzScoreRange::unknown(); positions],
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

            if old_score.is_certain() {
                continue;
            }

            let position = match self.material.index_to_position(pos_index) {
                Ok(p) => p,
                Err(_) => continue,
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

        if !mv.is_capture() {
            let child_index = self.material.position_to_index(&child_position).unwrap();
            self.positions[child_index].add_half_move()
        } else if child_position.is_checkmate() {
            DtzScoreRange::checkmate()
        } else if child_position.is_stalemate() || child_position.is_insufficient_material() {
            DtzScoreRange::draw()
        } else {
            unimplemented!("Probing child tables not implemented");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wdl_score_range::WdlScoreRange;
    use shakmaty::{CastlingMode, fen::Fen};

    #[test]
    fn position_index_roundtrip() {
        let tb = TableBuilder {
            material: MaterialKey::from_string("KQvK").unwrap(),
            positions: Vec::new(),
        };

        let position = "7k/8/8/8/8/8/8/KQ6 w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let index = tb.material.position_to_index(&position).unwrap();
        let reconstructed = tb
            .material
            .index_to_position(index)
            .expect("valid position");

        assert_eq!(
            tb.material.position_to_index(&reconstructed).unwrap(),
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

        let checkmate_idx = tb.material.position_to_index(&checkmate).unwrap();
        let stalemate_idx = tb.material.position_to_index(&stalemate).unwrap();

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
        let idx = tb.material.position_to_index(&mate_in_one).unwrap();

        // Identify the checkmate position reached after Qb7#.
        let checkmate = "k7/1Q6/2K5/8/8/8/8/8 b - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        let checkmate_idx = tb.material.position_to_index(&checkmate).unwrap();

        tb.positions[idx] = DtzScoreRange::unknown();
        tb.positions[checkmate_idx] = DtzScoreRange::unknown();

        // First step marks the checkmate child.
        tb.step();
        // Second step propagates to the parent position.
        tb.step();

        let wdl: WdlScoreRange = tb.positions[idx].into();
        assert_eq!(wdl, WdlScoreRange::Win);
    }
}
