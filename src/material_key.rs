use std::fmt;
use std::ops::Deref;

use shakmaty::{Color, Piece, Role};

/// Represents a material configuration, e.g. `KQvK`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialKey {
    pieces: Vec<Piece>,
}

impl MaterialKey {
    /// Create a new material key from a list of pieces.
    pub fn new(pieces: Vec<Piece>) -> Self {
        Self { pieces }
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
