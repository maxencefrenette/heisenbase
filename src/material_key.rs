use std::fmt;

use shakmaty::{Color, PositionErrorKinds, Role};

/// Represents a material configuration, e.g. `KQvK`.

#[derive(Clone, Copy)]
pub(crate) enum PieceDescriptor {
    King,
    Queen,
    Rook,
    LightBishop,
    DarkBishop,
    Knight,
    Pawn,
}

impl PieceDescriptor {
    fn token(self) -> &'static str {
        match self {
            PieceDescriptor::King => "K",
            PieceDescriptor::Queen => "Q",
            PieceDescriptor::Rook => "R",
            PieceDescriptor::LightBishop => "Bl",
            PieceDescriptor::DarkBishop => "Bd",
            PieceDescriptor::Knight => "N",
            PieceDescriptor::Pawn => "P",
        }
    }

    pub(crate) fn role(self) -> Role {
        match self {
            PieceDescriptor::King => Role::King,
            PieceDescriptor::Queen => Role::Queen,
            PieceDescriptor::Rook => Role::Rook,
            PieceDescriptor::LightBishop | PieceDescriptor::DarkBishop => Role::Bishop,
            PieceDescriptor::Knight => Role::Knight,
            PieceDescriptor::Pawn => Role::Pawn,
        }
    }

    pub(crate) fn light(self) -> Option<bool> {
        match self {
            PieceDescriptor::LightBishop => Some(true),
            PieceDescriptor::DarkBishop => Some(false),
            _ => None,
        }
    }

    fn from_token(tok: &str) -> Option<Self> {
        match tok {
            "K" => Some(PieceDescriptor::King),
            "Q" => Some(PieceDescriptor::Queen),
            "R" => Some(PieceDescriptor::Rook),
            "Bl" => Some(PieceDescriptor::LightBishop),
            "Bd" => Some(PieceDescriptor::DarkBishop),
            "N" => Some(PieceDescriptor::Knight),
            "P" => Some(PieceDescriptor::Pawn),
            _ => None,
        }
    }
}

pub(crate) const PIECES: [PieceDescriptor; 7] = [
    PieceDescriptor::King,
    PieceDescriptor::Queen,
    PieceDescriptor::Rook,
    PieceDescriptor::LightBishop,
    PieceDescriptor::DarkBishop,
    PieceDescriptor::Knight,
    PieceDescriptor::Pawn,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MaterialKey {
    /// Piece counts indexed by color then piece descriptor.
    pub(crate) counts: [[u8; PIECES.len()]; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialError {
    MismatchedMaterial,
    IndexOutOfBounds,
    InvalidPosition(PositionErrorKinds),
}

impl MaterialKey {
    /// Parse a [`MaterialKey`] from its textual representation.
    ///
    /// # Syntax
    /// The expected form is `<white pieces>v<black pieces>`, where each side is
    /// a sequence of piece tokens and exactly one `v` separates the two
    /// sides.
    ///
    /// # Piece tokens and colors
    /// Supported tokens are `K`, `Q`, `R`, `Bl`, `Bd`, `N` and `P` for king,
    /// queen, rook, light-squared bishop, dark-squared bishop, knight and pawn
    /// respectively. Pieces appearing before the separator are interpreted as
    /// white, while those after it are treated as black.
    ///
    /// # Use cases
    /// This is primarily useful for tests and simple user interfaces that need
    /// to describe a set of pieces without board coordinates.
    ///
    /// # Errors
    /// Returns `None` if the string is malformed, contains unsupported
    /// tokens, has a missing or extra separator, or is otherwise ambiguous.
    pub fn from_string(s: &str) -> Option<Self> {
        let mut parts = s.split('v');
        let white = parts.next()?;
        let black = parts.next()?;

        // Only one separator is allowed.
        if parts.next().is_some() {
            return None;
        }

        let mut counts = [[0u8; PIECES.len()]; 2];

        fn push_pieces(out: &mut [[u8; PIECES.len()]; 2], s: &str, color: Color) -> Option<()> {
            let color_idx = match color {
                Color::White => 0,
                Color::Black => 1,
            };

            let bytes = s.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let token = match bytes[i] as char {
                    'B' => {
                        if i + 1 >= bytes.len() {
                            return None;
                        }
                        match bytes[i + 1] as char {
                            'l' => {
                                i += 2;
                                "Bl"
                            }
                            'd' => {
                                i += 2;
                                "Bd"
                            }
                            _ => return None,
                        }
                    }
                    'K' => {
                        i += 1;
                        "K"
                    }
                    'Q' => {
                        i += 1;
                        "Q"
                    }
                    'R' => {
                        i += 1;
                        "R"
                    }
                    'N' => {
                        i += 1;
                        "N"
                    }
                    'P' => {
                        i += 1;
                        "P"
                    }
                    _ => return None,
                };

                let pd = PieceDescriptor::from_token(token)?;
                out[color_idx][pd as usize] += 1;
            }

            Some(())
        }

        push_pieces(&mut counts, white, Color::White)?;
        push_pieces(&mut counts, black, Color::Black)?;

        Some(Self { counts })
    }
}

impl fmt::Display for MaterialKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for color_idx in 0..2 {
            if color_idx == 1 {
                write!(f, "v")?;
            }

            for (i, pd) in PIECES.iter().enumerate() {
                for _ in 0..self.counts[color_idx][i] {
                    write!(f, "{}", pd.token())?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kqvk() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(mk.to_string(), "KQvK");
    }

    #[test]
    fn parses_light_and_dark_bishops() {
        let mk = MaterialKey::from_string("BlBdvK").unwrap();
        assert_eq!(mk.to_string(), "BlBdvK");
    }

    #[test]
    fn rejects_invalid_char() {
        assert!(MaterialKey::from_string("KXvK").is_none());
    }

    #[test]
    fn rejects_missing_separator() {
        assert!(MaterialKey::from_string("KQK").is_none());
    }
}
