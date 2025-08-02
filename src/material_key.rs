use std::fmt;
use std::ops::Deref;

use shakmaty::{CastlingMode, Chess, Color, FromSetup, Piece, Position, Role, Setup, Square};

/// Represents a material configuration, e.g. `KQvK`, and provides the
/// bidirectional mapping between permutations of those pieces and compact
/// indices.
///
/// The mapping only covers legal permutations where every piece occupies a
/// unique square. Illegal arrangements, such as overlapping pieces or
/// duplicate-piece ambiguity, are considered invalid and are never assigned an
/// index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialKey {
    pieces: Vec<Piece>,
}

impl MaterialKey {
    /// Create a new material key from a list of pieces.
    pub fn new(pieces: Vec<Piece>) -> Self {
        Self { pieces }
    }

    /// Total number of mappable positions for this material configuration.
    ///
    /// Each index corresponds to a unique permutation where all pieces appear
    /// on distinct squares. Invalid permutations that would place multiple
    /// pieces on the same square are skipped and receive no index.
    pub fn total_positions(&self) -> usize {
        64usize.pow(self.len() as u32)
    }

    /// Convert an index into a [`Chess`] position.
    ///
    /// Returns `None` when the index refers to an illegal placement, such as
    /// two pieces occupying the same square or duplicated material that is not
    /// yet supported. Only indices for legal permutations yield a position.
    ///
    /// TODO:
    /// Don't assign indices to invalid positions.
    /// Add support for duplicated material (e.g. 2 knights)
    pub fn index_to_position(&self, mut pos_index: usize) -> Option<Chess> {
        let mut setup = Setup::empty();

        for piece in self.iter() {
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

    /// Convert a [`Chess`] position back into its index within this material
    /// mapping.
    ///
    /// The position must contain each piece from this key exactly once on a
    /// distinct square. Positions that violate this requirement are outside the
    /// mapping and result in undefined behaviour.
    pub fn position_to_index(&self, position: &Chess) -> usize {
        let mut index = 0;

        for piece in self.iter().rev() {
            let square = position.board().by_piece(*piece).first().unwrap();
            index = index * 64 + square.to_usize();
        }

        index
    }
}

impl Deref for MaterialKey {
    type Target = [Piece];

    fn deref(&self) -> &Self::Target {
        &self.pieces
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut white: Vec<char> = Vec::new();
        let mut black: Vec<char> = Vec::new();

        for piece in &self.pieces {
            let ch = match piece.role {
                Role::King => 'K',
                Role::Queen => 'Q',
                Role::Rook => 'R',
                Role::Bishop => 'B',
                Role::Knight => 'N',
                Role::Pawn => 'P',
            };

            if piece.color == Color::White {
                white.push(ch);
            } else {
                black.push(ch);
            }
        }

        for c in white {
            write!(f, "{}", c)?;
        }

        write!(f, "v")?;

        for c in black {
            write!(f, "{}", c)?;
        }

        Ok(())
    }
}
