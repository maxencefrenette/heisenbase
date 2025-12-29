use crate::material_key::{HbPieceRole, MaterialKey};
use shakmaty::{
    Bitboard, CastlingMode, Chess, Color, FromSetup, Position, PositionErrorKinds, Setup, Square,
};

fn nth_light_square(n: u32) -> Square {
    debug_assert!(n < 32);
    let rank = n / 4;
    let file_index = n % 4;
    let file = if rank % 2 == 0 {
        1 + 2 * file_index
    } else {
        2 * file_index
    };
    Square::new(rank * 8 + file)
}

fn nth_dark_square(n: u32) -> Square {
    debug_assert!(n < 32);
    let rank = n / 4;
    let file_index = n % 4;
    let file = if rank % 2 == 0 {
        2 * file_index
    } else {
        1 + 2 * file_index
    };
    Square::new(rank * 8 + file)
}

/// This struct is used to create a Gödel number mapping for all positions of a material key.
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
        setup.board = self.material_key.pawns.to_board();

        for piece in self.material_key.pieces() {
            let radix = match piece.role {
                HbPieceRole::LightBishop | HbPieceRole::DarkBishop => 32,
                _ => 64,
            };

            let position = remaining % radix;
            remaining /= radix;

            let square = match piece.role {
                HbPieceRole::LightBishop => nth_light_square(position as u32),
                HbPieceRole::DarkBishop => nth_dark_square(position as u32),
                _ => Square::new(position as u32),
            };

            if setup.board.piece_at(square).is_some() {
                return Err(PositionMappingError::TwoPiecesOnSameSquare);
            }

            setup.board.set_piece_at(square, piece.into());
        }

        debug_assert!(remaining == 0);

        Chess::from_setup(setup, CastlingMode::Standard)
            .map_err(|e| PositionMappingError::InvalidPosition(e.kinds()))
    }

    pub fn position_to_index(&self, position: &Chess) -> Result<usize, PositionMappingError> {
        let board = position.board();
        let white_pawns = board.pawns() & board.white();
        let black_pawns = board.pawns() & board.black();
        if white_pawns != self.material_key.pawns.0.white
            || black_pawns != self.material_key.pawns.0.black
        {
            return Err(PositionMappingError::MismatchedMaterial);
        }

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

            let mask = match piece.role {
                HbPieceRole::LightBishop => Bitboard::LIGHT_SQUARES,
                HbPieceRole::DarkBishop => Bitboard::DARK_SQUARES,
                _ => Bitboard::FULL,
            };
            let bitboard = mask & board.by_piece(piece.into());
            let square = bitboard
                .first()
                .ok_or(PositionMappingError::MismatchedMaterial)?;
            board.discard_piece_at(square);
            let square_index = square.to_usize();

            let position = match piece.role {
                HbPieceRole::LightBishop | HbPieceRole::DarkBishop => square_index / 2,
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
    use proptest::{prelude::*, string::string_regex};

    fn material_key_strategy() -> impl Strategy<Value = MaterialKey> {
        string_regex("K(Q|R|Bl|Bd|N){0,2}([a-h][2-7]){0,3}vK(Q|R|Bl|Bd|N){0,2}([a-h][2-7]){0,3}")
            .unwrap()
            .prop_filter_map("valid material key", |value| {
                MaterialKey::from_string(&value)
            })
    }

    fn indexed_material_strategy() -> impl Strategy<Value = (MaterialKey, usize)> {
        material_key_strategy().prop_flat_map(|mk| {
            let total = PositionIndexer::new(mk.clone()).total_positions();
            (Just(mk), 0..total)
        })
    }

    proptest! {
        #[test]
        fn roundtrip_indices((mk, index) in indexed_material_strategy()) {
            let indexer = PositionIndexer::new(mk);
            let Ok(pos) = indexer.index_to_position(index) else {
                return Ok(());
            };

            // The naive index can map 2 indices to the same position.
            let index = indexer
                .position_to_index(&pos)
                .expect("This position came from a valid index, so it should never fail");
            let pos2 = indexer
                .index_to_position(index)
                .expect("This index came from a valid position, so it should never fail");
            prop_assert_eq!(pos, pos2);
        }
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

    #[test]
    fn position_with_pawn_roundtrips() {
        use shakmaty::{CastlingMode, Square, fen::Fen};

        let mk = MaterialKey::from_string("Ka2vK").unwrap();
        let indexer = PositionIndexer::new(mk);
        let position = "8/8/8/8/8/8/P7/K6k w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        let index = indexer.position_to_index(&position).unwrap();
        let reconstructed = indexer.index_to_position(index).unwrap();
        let board = reconstructed.board();
        assert!(board.pawns().contains(Square::A2));
        assert!(board.white().contains(Square::A2));
    }

    #[test]
    fn position_to_index_rejects_missing_pawn() {
        use shakmaty::{CastlingMode, fen::Fen};

        let mk = MaterialKey::from_string("Ka2vK").unwrap();
        let indexer = PositionIndexer::new(mk);
        let position = "8/8/8/8/8/8/8/K6k w - - 0 1"
            .parse::<Fen>()
            .unwrap()
            .into_position(CastlingMode::Standard)
            .unwrap();

        assert!(matches!(
            indexer.position_to_index(&position),
            Err(PositionMappingError::MismatchedMaterial)
        ));
    }
}
