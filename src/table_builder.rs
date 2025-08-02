use crate::material_key::MaterialKey;
use crate::score::DtzScoreRange;
use shakmaty::{CastlingMode, Chess, FromSetup, Piece, Position, Setup, Square};

pub struct TableBuilder {
    material: MaterialKey,
    positions: Vec<DtzScoreRange>,
}

impl TableBuilder {
    pub fn new(material: MaterialKey) -> Self {
        let positions = Self::total_positions(&material);

        Self {
            material,
            positions: vec![DtzScoreRange::unknown(); positions],
        }
    }

    pub fn solve(&mut self) {
        for i in 0..100 {
            println!("Step {}", i);
            self.step();
        }
    }

    /// Perform one iteration of the table builder.
    ///
    /// This performs one bellman update on every position in the table.
    fn step(&mut self) {
        for pos_index in 0..self.positions.len() {
            let position = self.index_to_position(pos_index);

            // If the position is invalid, skip it.
            if let Some(position) = position {
                if position.is_checkmate() {
                    self.positions[pos_index] = DtzScoreRange::checkmate();
                    continue;
                }

                if position.is_stalemate() || position.is_insufficient_material() {
                    self.positions[pos_index] = DtzScoreRange::draw();
                    continue;
                }

                let score = position
                    .legal_moves()
                    .into_iter()
                    .map(|chess_move| {
                        let mut child_position = position.clone();
                        child_position.play_unchecked(chess_move);
                        let child_index = self.position_to_index(&child_position);
                        self.positions[child_index]
                    })
                    .fold(self.positions[pos_index], |a, b| a.negamax(&b));

                self.positions[pos_index] = score;
            }
        }
    }

    fn total_positions(material: &MaterialKey) -> usize {
        64usize.pow(material.len() as u32)
    }

    /// Make a position from a position index.
    ///
    /// Returns `None` if the position is invalid.
    ///
    /// TODO:
    /// Don't assign indices to invalid positions.
    /// Add support for duplicated material (e.g. 2 knights)
    fn index_to_position(&self, mut pos_index: usize) -> Option<Chess> {
        let mut setup = Setup::empty();

        for piece in self.material.iter() {
            let index = pos_index % 64;
            let square = Square::new(index as u32);

            if setup.board.piece_at(square).is_some() {
                return None;
            }

            setup.board.set_piece_at(square, *piece);
            pos_index /= 64;
        }

        Chess::from_setup(setup, CastlingMode::Standard).ok()
    }

    fn position_to_index(&self, position: &Chess) -> usize {
        let mut index = 0;

        for piece in self.material.iter().rev() {
            let square = position.board().by_piece(*piece).first().unwrap();
            index = index * 64 + square.to_usize();
        }

        index
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use shakmaty::fen::Fen;

    #[test]
    fn position_index_roundtrip() {
        let tb = TableBuilder {
            material: MaterialKey::new(vec![
                Piece::from_char('K').unwrap(),
                Piece::from_char('Q').unwrap(),
                Piece::from_char('k').unwrap(),
            ]),
            positions: Vec::new(),
        };

        let position = "7k/8/8/8/8/8/8/KQ6 w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let index = tb.position_to_index(&position);
        let reconstructed = tb.index_to_position(index).expect("valid position");

        assert_eq!(tb.position_to_index(&reconstructed), index);
    }
}
