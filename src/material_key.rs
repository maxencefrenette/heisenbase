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

    /// Parse a [`MaterialKey`] from its textual representation.
    ///
    /// # Syntax
    /// The expected form is `<white pieces>v<black pieces>`, where each side is
    /// a sequence of piece characters and exactly one `v` separates the two
    /// sides.
    ///
    /// # Piece characters and colors
    /// Supported characters are `K`, `Q`, `R`, `B`, `N` and `P` for king,
    /// queen, rook, bishop, knight and pawn respectively. All characters must
    /// be uppercase. Pieces appearing before the separator are interpreted as
    /// white, while those after it are treated as black.
    ///
    /// # Use cases
    /// This is primarily useful for tests and simple user interfaces that need
    /// to describe a set of pieces without board coordinates.
    ///
    /// # Errors
    /// Returns `None` if the string is malformed, contains unsupported
    /// characters, has a missing or extra separator, or is otherwise ambiguous.
    pub fn from_string(s: &str) -> Option<Self> {
        let mut parts = s.split('v');
        let white = parts.next()?;
        let black = parts.next()?;

        // Only one separator is allowed.
        if parts.next().is_some() {
            return None;
        }

        let mut pieces = Vec::new();

        fn push_pieces(out: &mut Vec<Piece>, chars: &str, color: Color) -> Option<()> {
            for ch in chars.chars() {
                let role = match ch {
                    'K' => Role::King,
                    'Q' => Role::Queen,
                    'R' => Role::Rook,
                    'B' => Role::Bishop,
                    'N' => Role::Knight,
                    'P' => Role::Pawn,
                    _ => return None,
                };

                out.push(Piece { role, color });
            }

            Some(())
        }

        push_pieces(&mut pieces, white, Color::White)?;
        push_pieces(&mut pieces, black, Color::Black)?;

        Some(Self::new(pieces))
    }

    /// Total number of mappable positions for this material configuration.
    ///
    /// Each index corresponds to a unique permutation where all pieces appear
    /// on distinct squares. Since squares cannot be reused, the total count is
    /// `64 * 63 * 62 * ...` for as many pieces as are in the key.
    pub fn total_positions(&self) -> usize {
        (0..self.len()).fold(1, |acc, i| acc * (64 - i))
    }

    /// Convert an index into a [`Chess`] position.
    ///
    /// Returns `None` only when the index exceeds the number of mappable
    /// positions. Every index less than [`total_positions`](Self::total_positions)
    /// yields a unique placement with all pieces on distinct squares.
    pub fn index_to_position(&self, mut pos_index: usize) -> Option<Chess> {
        if pos_index >= self.total_positions() {
            return None;
        }

        let mut setup = Setup::empty();
        let mut squares: Vec<Square> = (0..64).map(|i| Square::new(i as u32)).collect();

        for piece in self.iter() {
            let base = squares.len();
            let idx = pos_index % base;
            pos_index /= base;
            let square = squares.remove(idx);
            setup.board.set_piece_at(square, *piece);
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
        let mut index = 0usize;
        let mut multiplier = 1usize;
        let mut squares: Vec<Square> = (0..64).map(|i| Square::new(i as u32)).collect();

        for piece in self.iter() {
            let base = squares.len();
            let square = position.board().by_piece(*piece).first().unwrap();
            let idx = squares
                .iter()
                .position(|&s| s == square)
                .expect("piece square must exist");
            index += idx * multiplier;
            squares.remove(idx);
            multiplier *= base;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};
    use shakmaty::Piece;

    #[test]
    fn parses_kqvk() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(
            mk,
            MaterialKey::new(vec![
                Piece::from_char('K').unwrap(),
                Piece::from_char('Q').unwrap(),
                Piece::from_char('k').unwrap(),
            ])
        );
    }

    #[test]
    fn rejects_invalid_char() {
        assert!(MaterialKey::from_string("KXvK").is_none());
    }

    #[test]
    fn rejects_missing_separator() {
        assert!(MaterialKey::from_string("KQK").is_none());
    }

    #[test]
    fn total_positions_without_overlap() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        assert_eq!(mk.total_positions(), 64 * 63 * 62);
    }

    fn roundtrip_random_indices(mk: MaterialKey, seed: u64) {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut successes = 0;
        while successes < 10 {
            let index = rng.gen_range(0..mk.total_positions());
            if let Some(pos) = mk.index_to_position(index) {
                let roundtrip = mk.position_to_index(&pos);
                assert_eq!(index, roundtrip);
                successes += 1;
            }
        }
    }

    #[test]
    fn roundtrip_kvk() {
        let mk = MaterialKey::from_string("KvK").unwrap();
        roundtrip_random_indices(mk, 0);
    }

    #[test]
    fn roundtrip_kqvk() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        roundtrip_random_indices(mk, 1);
    }

    #[test]
    fn roundtrip_krvkb() {
        let mk = MaterialKey::from_string("KRvKB").unwrap();
        roundtrip_random_indices(mk, 2);
    }

    #[test]
    fn roundtrip_kqvkr() {
        let mk = MaterialKey::from_string("KQvKR").unwrap();
        roundtrip_random_indices(mk, 3);
    }

    #[test]
    fn roundtrip_kbnvkq() {
        let mk = MaterialKey::from_string("KBNvKQ").unwrap();
        roundtrip_random_indices(mk, 4);
    }
}
