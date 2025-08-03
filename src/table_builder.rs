use crate::material_key::MaterialKey;
use crate::score::DtzScoreRange;
use shakmaty::Position;

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
            println!("Step {}: {} updates", step, updates);
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
            let old = self.positions[pos_index];
            let position = self.material.index_to_position(pos_index);

            // If the position is invalid, skip it.
            if let Some(position) = position {
                let new_score = if position.is_checkmate() {
                    DtzScoreRange::checkmate()
                } else if position.is_stalemate() || position.is_insufficient_material() {
                    DtzScoreRange::draw()
                } else {
                    position
                        .legal_moves()
                        .into_iter()
                        .map(|chess_move| {
                            let mut child_position = position.clone();
                            child_position.play_unchecked(chess_move);
                            let child_index = self.material.position_to_index(&child_position);
                            self.positions[child_index]
                        })
                        .fold(self.positions[pos_index], |a, b| a.negamax(&b))
                };

                if new_score != old {
                    self.positions[pos_index] = new_score;
                    updates += 1;
                }
            }
        }
        updates
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

        let index = tb.material.position_to_index(&position);
        let reconstructed = tb
            .material
            .index_to_position(index)
            .expect("valid position");

        assert_eq!(tb.material.position_to_index(&reconstructed), index);
    }
}
