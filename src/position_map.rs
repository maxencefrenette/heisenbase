use crate::material_key::{HbPieceRole, MaterialKey};
use shakmaty::{
    CastlingMode, Chess, Color, FromSetup, Position, PositionErrorKinds, Setup, Square,
};

#[derive(Clone)]
pub struct PositionIndexer {
    material_key: MaterialKey,
    total_positions: usize,
}

impl PositionIndexer {
    pub fn new(material_key: MaterialKey) -> Self {
        let mut total_positions = 2;
        for piece in material_key.pieces() {
            match piece.role {
                HbPieceRole::LightBishop | HbPieceRole::DarkBishop => {
                    total_positions *= 32;
                }
                _ => {
                    total_positions *= 64;
                }
            }
        }

        Self {
            material_key,
            total_positions,
        }
    }

    pub fn total_positions(&self) -> usize {
        self.total_positions
    }

    /// Convert an index into a [`Chess`] position.
    ///
    /// Every index less than [`self.total_positions()`] corresponds to a unique arrangement
    /// of the pieces described by the material key.  The mapping is purely
    /// combinatorial and intentionally ignores the rules of play, so some indices
    /// yield setups that are unreachable or illegal under normal chess rules — for
    /// example, kings adjacent to one another or a side to move that can
    /// immediately capture the opposing king.  When [`shakmaty`] rejects such a
    /// placement, this function returns [`Err(MaterialError::InvalidPosition)`].
    ///
    /// Indices greater than or equal to `total_positions()` return
    /// [`Err(MaterialError::IndexOutOfBounds)`].
    pub fn index_to_position(&self, index: usize) -> Result<Chess, PositionMappingError> {
        if index >= self.total_positions {
            return Err(PositionMappingError::IndexOutOfBounds);
        }

        let turn = match index % 2 {
            0 => Color::White,
            1 => Color::Black,
            _ => unreachable!(),
        };
        let mut remaining = index / 2;

        let mut setup = Setup::empty();
        setup.turn = turn;

        for piece in self.material_key.pieces() {
            let radix = match piece.role {
                HbPieceRole::LightBishop | HbPieceRole::DarkBishop => 32,
                _ => 64,
            };

            let position = remaining % radix;
            remaining /= radix;

            let square_index = match piece.role {
                HbPieceRole::LightBishop => position * 2 + 1,
                HbPieceRole::DarkBishop => position * 2,
                _ => position,
            };

            if setup
                .board
                .piece_at(Square::new(square_index as u32))
                .is_some()
            {
                return Err(PositionMappingError::TwoPiecesOnSameSquare);
            }

            setup
                .board
                .set_piece_at(Square::new(square_index as u32), piece.into());
        }

        debug_assert!(remaining == 0);

        Chess::from_setup(setup, CastlingMode::Standard)
            .map_err(|e| PositionMappingError::InvalidPosition(e.kinds()))
    }

    pub fn position_to_index(&self, position: &Chess) -> Result<usize, PositionMappingError> {
        let mut index = 0;
        let mut multiplier = 1;

        let turn_index = match position.turn() {
            Color::White => 0,
            Color::Black => 1,
        };
        index += multiplier * turn_index;
        multiplier *= 2;

        let mut board = position.board().clone();

        for piece in self.material_key.pieces() {
            let radix = match piece.role {
                HbPieceRole::LightBishop | HbPieceRole::DarkBishop => 32,
                _ => 64,
            };

            let bitboard = board.by_piece(piece.into());
            let square = bitboard
                .first()
                .ok_or(PositionMappingError::MismatchedMaterial)?;
            board.discard_piece_at(square);
            let square_index = square.to_usize();

            let position = match piece.role {
                HbPieceRole::LightBishop => (square_index - 1) / 2,
                HbPieceRole::DarkBishop => square_index / 2,
                _ => square_index,
            };

            index += multiplier * position;
            multiplier *= radix;
        }

        debug_assert!(index < self.total_positions);
        debug_assert!(multiplier == self.total_positions);

        Ok(index)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionMappingError {
    /// The material key does not match the position.
    MismatchedMaterial,
    /// The index is out of bounds.
    IndexOutOfBounds,
    /// The index corresponds to a position where two pieces are on the same square.
    TwoPiecesOnSameSquare,
    /// The resulting position fits on the board, but is invalid due to some rule of chess.
    InvalidPosition(PositionErrorKinds),
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng, rngs::StdRng};

    fn roundtrip_random_indices(mk: MaterialKey, seed: u64) {
        let indexer = PositionIndexer::new(mk.clone());
        let mut rng = StdRng::seed_from_u64(seed);
        let total = indexer.total_positions();
        for _ in 0..10 {
            let index = rng.gen_range(0..total);

            let pos = match indexer.index_to_position(index) {
                Ok(pos) => pos,
                Err(_) => continue,
            };

            // The naive index can map 2 indices to the same position
            let index = indexer
                .position_to_index(&pos)
                .expect("This position came from a valid index, so it should never fail");
            let pos2 = indexer
                .index_to_position(index)
                .expect("This index came from a vald position, so it should never fail");
            assert_eq!(pos, pos2);
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
    fn canonicalizes_table_builder_positions() {
        use shakmaty::{CastlingMode, fen::Fen};

        let mk = MaterialKey::from_string("KQvK").unwrap();
        let indexer = PositionIndexer::new(mk.clone());
        let fen = "k7/1Q6/2K5/8/8/8/8/8 b - - 0 1";
        let position = fen
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();
        assert!(indexer.position_to_index(&position).is_ok());
    }

    #[test]
    fn roundtrip_krvkb() {
        let mk = MaterialKey::from_string("KRvKBd").unwrap();
        roundtrip_random_indices(mk, 2);
    }

    #[test]
    fn roundtrip_kqvkr() {
        let mk = MaterialKey::from_string("KQvKR").unwrap();
        roundtrip_random_indices(mk, 3);
    }

    #[test]
    fn roundtrip_kbnvkq() {
        let mk = MaterialKey::from_string("KBdNvKQ").unwrap();
        roundtrip_random_indices(mk, 4);
    }

    #[test]
    fn roundtrip_knnvk() {
        let mk = MaterialKey::from_string("KNNvK").unwrap();
        roundtrip_random_indices(mk, 5);
    }

    #[test]
    fn index_to_position_out_of_bounds() {
        let mk = MaterialKey::from_string("KQvK").unwrap();
        let indexer = PositionIndexer::new(mk.clone());
        let index = indexer.total_positions();
        assert!(matches!(
            indexer.index_to_position(index),
            Err(PositionMappingError::IndexOutOfBounds)
        ));
    }

    #[test]
    fn exhaustive_roundtrip_kvk() {
        let mk = MaterialKey::from_string("KvK").unwrap();
        let indexer = PositionIndexer::new(mk.clone());
        for idx in 0..indexer.total_positions() {
            if let Ok(pos) = indexer.index_to_position(idx) {
                let roundtrip = indexer.position_to_index(&pos).unwrap();
                assert_eq!(idx, roundtrip);
            }
        }
    }

    #[test]
    fn position_to_index_mismatched_material() {
        use shakmaty::{CastlingMode, Chess, Color, Piece, Role, Setup, Square};

        let mk = MaterialKey::from_string("KQvK").unwrap();
        let indexer = PositionIndexer::new(mk.clone());
        let mut setup = Setup::empty();
        setup.board.set_piece_at(
            Square::E1,
            Piece {
                role: Role::King,
                color: Color::White,
            },
        );
        setup.board.set_piece_at(
            Square::E8,
            Piece {
                role: Role::King,
                color: Color::Black,
            },
        );
        setup.turn = Color::White;

        let pos = Chess::from_setup(setup, CastlingMode::Standard).unwrap();
        assert!(matches!(
            indexer.position_to_index(&pos),
            Err(PositionMappingError::MismatchedMaterial)
        ));
    }
}
