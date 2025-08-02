use std::ops::{Add, AddAssign, Neg, Sub, SubAssign};

use shakmaty::{CastlingMode, Chess, FromSetup, Piece, Position, Setup, Square};

/// A DTZ score.
///
/// This score is from the perspective of the side to move.
///
/// +99 means the side to move wins and has a zeroing move immediately available
/// +1 means the side to move wins and has a zeroing move in 100 halfmoves
/// 0 means the side to move draws
/// -1 means the side to move loses and has a zeroing move immediately available
/// -99 means the side to move loses and has a zeroing move in 100 halfmoves
/// -100 means the side to move is checkmated
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct DtzScore(i8);

impl DtzScore {
    fn immediate_win() -> Self {
        Self(99)
    }

    fn immediate_loss() -> Self {
        Self(-100)
    }

    fn draw() -> Self {
        Self(0)
    }

    fn is_draw(&self) -> bool {
        self.0 == 0
    }
}

impl Neg for DtzScore {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl Add<i8> for DtzScore {
    type Output = Self;

    fn add(self, other: i8) -> Self::Output {
        Self(self.0 + other)
    }
}

impl AddAssign<i8> for DtzScore {
    fn add_assign(&mut self, other: i8) {
        self.0 += other;
    }
}

impl Sub<i8> for DtzScore {
    type Output = Self;

    fn sub(self, other: i8) -> Self::Output {
        Self(self.0 - other)
    }
}

impl SubAssign<i8> for DtzScore {
    fn sub_assign(&mut self, other: i8) {
        self.0 -= other;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScoreRange {
    pub min: DtzScore,
    pub max: DtzScore,
}

impl ScoreRange {
    pub fn unknown() -> Self {
        Self {
            min: DtzScore::immediate_loss(),
            max: DtzScore::immediate_win(),
        }
    }

    pub fn checkmate() -> Self {
        Self {
            min: DtzScore::immediate_loss(),
            max: DtzScore::immediate_loss(),
        }
    }

    pub fn draw() -> Self {
        Self {
            min: DtzScore::draw(),
            max: DtzScore::draw(),
        }
    }

    fn parent_move_score(&self) -> Self {
        let mut min = -self.max;
        let mut max = -self.min;

        if !min.is_draw() {
            min += 1;
        }
        if !max.is_draw() {
            max -= 1;
        }

        Self { min, max }
    }

    /// Used as part of a reduce call to find the best score.
    ///
    /// other is one halfmove in the future compared to self.
    fn negamax(&self, other: &Self) -> Self {
        let other_flipped = other.parent_move_score();

        let min = self.min.max(other_flipped.min);
        let max = self.max.max(other_flipped.max);

        Self { min, max }
    }
}

pub struct TableBuilder {
    material: Vec<Piece>,
    positions: Vec<ScoreRange>,
}

impl TableBuilder {
    pub fn new(material: Vec<Piece>) -> Self {
        let positions = Self::total_positions(&material);

        Self {
            material,
            positions: vec![ScoreRange::unknown(); positions],
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
                    self.positions[pos_index] = ScoreRange::checkmate();
                    continue;
                }

                if position.is_stalemate() || position.is_insufficient_material() {
                    self.positions[pos_index] = ScoreRange::draw();
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

    fn total_positions(material: &Vec<Piece>) -> usize {
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
            material: vec![
                Piece::from_char('K').unwrap(),
                Piece::from_char('Q').unwrap(),
                Piece::from_char('k').unwrap(),
            ],
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
