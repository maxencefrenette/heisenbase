use std::cmp::Ordering;

use shakmaty::{Bitboard, Board, ByColor, Color, File, Rank};

/// Represents the pawn structure of a position, i.e. the pawns on the board.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PawnStructure(pub ByColor<Bitboard>);

impl PawnStructure {
    pub fn new(white_pawns: Bitboard, black_pawns: Bitboard) -> Self {
        Self(ByColor {
            white: white_pawns,
            black: black_pawns,
        })
    }

    pub fn from_board(board: &Board) -> Self {
        Self(ByColor {
            white: board.pawns() & board.white(),
            black: board.pawns() & board.black(),
        })
    }

    pub fn occupied(&self) -> Bitboard {
        self.0.white | self.0.black
    }

    pub fn pawn_count(&self) -> u32 {
        self.occupied().count() as u32
    }

    pub fn is_symmetric_sides(&self) -> bool {
        self.0.white == self.0.black.flip_vertical()
    }

    pub fn is_symmetric_horizontal(&self) -> bool {
        self.0.white == self.0.white.flip_horizontal()
            && self.0.black == self.0.black.flip_horizontal()
    }

    pub fn flip_sides(&self) -> Self {
        Self(ByColor {
            white: self.0.black.flip_vertical(),
            black: self.0.white.flip_vertical(),
        })
    }

    pub fn flip_horizontal(&self) -> Self {
        Self(ByColor {
            white: self.0.white.flip_horizontal(),
            black: self.0.black.flip_horizontal(),
        })
    }

    pub fn to_board(&self) -> Board {
        use shakmaty::{ByColor, ByRole, Role};

        let mut by_role = ByRole::default();
        by_role[Role::Pawn] = self.occupied();
        let by_color = ByColor {
            white: self.0.white,
            black: self.0.black,
        };
        Board::try_from_bitboards(by_role, by_color).unwrap()
    }

    /// Returns the pawn structures that can be reached from this pawn structure without changing the piece count.
    ///
    /// This includes:
    /// - Moving a pawn one or two squares forward.
    /// - Capturing a pawn with a piece (removes a pawn from the board)
    pub fn child_pawn_structures_no_piece_change(&self) -> Vec<PawnStructure> {
        fn one_sided_pushes(ps: &PawnStructure) -> impl Iterator<Item = PawnStructure> {
            let can_be_moved_one_square = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
                & ps.0.white
                & !ps.0.black.shift(-8);
            let can_be_moved_two_squares = Bitboard::from_rank(Rank::Second)
                & ps.0.white
                & !ps.0.black.shift(-8)
                & !ps.0.black.shift(-16);
            let can_capture_left = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
            .without(Bitboard::from_file(File::A))
                & ps.0.white
                & ps.0.black.shift(-7);
            let can_capture_right = (Bitboard::FULL
                .without(Bitboard::BACKRANKS)
                .without(Bitboard::from_rank(Rank::Seventh)))
            .without(Bitboard::from_file(File::H))
                & ps.0.white
                & ps.0.black.shift(-9);

            can_be_moved_one_square
                .into_iter()
                .map(|square| {
                    let mut child = ps.clone();
                    child.0.white.discard(square);
                    child.0.white.add(square.offset(8).unwrap());
                    child
                })
                .chain(can_be_moved_two_squares.into_iter().map(|square| {
                    let mut child = ps.clone();
                    child.0.white.discard(square);
                    child.0.white.add(square.offset(16).unwrap());
                    child
                }))
                .chain(can_capture_right.into_iter().map(|square| {
                    let mut child = ps.clone();
                    let target = square.offset(9).unwrap();
                    child.0.white.discard(square);
                    child.0.white.add(target);
                    child.0.black.discard(target);
                    child
                }))
                .chain(can_capture_left.into_iter().map(|square| {
                    let mut child = ps.clone();
                    let target = square.offset(7).unwrap();
                    child.0.white.discard(square);
                    child.0.white.add(target);
                    child.0.black.discard(target);
                    child
                }))
        }

        let pawn_captured_by_a_piece = self.occupied().into_iter().map(|square| {
            Self(ByColor {
                white: self.0.white.without(square),
                black: self.0.black.without(square),
            })
        });

        pawn_captured_by_a_piece
            .chain(one_sided_pushes(self))
            .chain(one_sided_pushes(&self.flip_sides()).map(|ps| ps.flip_sides()))
            .collect()
    }

    /// Returns the pawn structures that can be reached from this pawn structure when `color` makes a move
    /// by capturing a piece with a pawn without promoting a pawn.
    pub fn child_pawn_structures_with_piece_capture(&self, color: Color) -> Vec<PawnStructure> {
        let from_color_perspective = match color {
            Color::White => self,
            Color::Black => &self.flip_sides(),
        };

        let can_capture_left = (Bitboard::FULL
            .without(Bitboard::BACKRANKS)
            .without(Bitboard::from_rank(Rank::Seventh)))
        .without(Bitboard::from_file(File::A))
            & from_color_perspective.0.white
            & !from_color_perspective.0.black.shift(-7);
        let can_capture_right = (Bitboard::FULL
            .without(Bitboard::BACKRANKS)
            .without(Bitboard::from_rank(Rank::Seventh)))
        .without(Bitboard::from_file(File::H))
            & from_color_perspective.0.white
            & !from_color_perspective.0.black.shift(-9);

        can_capture_right
            .into_iter()
            .map(|square| {
                let mut child = from_color_perspective.clone();
                child.0.white.discard(square);
                child.0.white.add(square.offset(9).unwrap());
                child
            })
            .chain(can_capture_left.into_iter().map(|square| {
                let mut child = from_color_perspective.clone();
                child.0.white.discard(square);
                child.0.white.add(square.offset(7).unwrap());
                child
            }))
            .collect()
    }

    /// Returns the pawn structures that can be reached from this pawn structure when `color` promotes a pawn.
    ///
    /// Since the backrank is always free of pawns, we don't need to differentiate between promoting with or without
    /// piece captures. They are the same set of child pawn structures.
    #[allow(dead_code)]
    pub fn child_pawn_structures_with_promotion(&self, color: Color) -> Vec<PawnStructure> {
        let from_color_perspective = match color {
            Color::White => self,
            Color::Black => &self.flip_sides(),
        };

        let can_promote = Bitboard::from_rank(Rank::Seventh) & from_color_perspective.0.white;

        can_promote
            .into_iter()
            .map(|square| {
                let mut child = from_color_perspective.clone();
                child.0.white.discard(square);
                child
            })
            .collect()
    }
}

impl PartialOrd for PawnStructure {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PawnStructure {
    fn cmp(&self, other: &Self) -> Ordering {
        let white_cmp = self.0.white.cmp(&other.0.white);
        if white_cmp != Ordering::Equal {
            return white_cmp;
        }
        self.0.black.cmp(&other.0.black)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use shakmaty::{Bitboard, Board, Square};

    #[test]
    fn empty_pawn_structure_generates_no_moves() {
        let parent = PawnStructure::new(Bitboard::EMPTY, Bitboard::EMPTY);
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_change()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"[]"
        );
    }

    #[test]
    fn child_pawn_structures_generates_moves_for_both_colors() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E2),
            Bitboard::from_square(Square::E7),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . p . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . P . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(parent.child_pawn_structures_no_piece_change().into_iter().map(|ps| ps.to_board()).collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            ,
        ]
        ");
    }

    #[test]
    fn child_pawn_structures_blocked_by_opponent_generates_no_moves() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E2),
            Bitboard::from_square(Square::E3),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . p . . .
        . . . . P . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_change()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_blocked_double_step_still_allows_single_step() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E2),
            Bitboard::from_square(Square::E4),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . p . . .
        . . . . . . . .
        . . . . P . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_change()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . P . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . P . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_includes_pawn_captures() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E4),
            Bitboard::from_square(Square::D5),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . p . . . .
        . . . . P . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_change()
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . p . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . p P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . P . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . p P . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_seventh_rank_generates_no_moves() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::A7),
            Bitboard::from_square(Square::H2),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        P . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . p
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_no_piece_change()
                .into_iter()
                .map(|ps| ps.to_board())
            .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            P . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . p
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_with_piece_captures_generates_moves_for_white() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E4),
            Bitboard::from_square(Square::E5),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . p . . .
        . . . . P . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_piece_capture(Color::White)
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . p P . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . P p . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_with_piece_captures_excludes_capturing_pawns() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::E4),
            Bitboard::from_square(Square::D5) | Bitboard::from_square(Square::F5),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . p . p . .
        . . . . P . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_piece_capture(Color::White)
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"[]"
        );
    }

    #[test]
    fn child_pawn_structures_with_piece_captures_generates_moves_for_black() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::B2),
            Bitboard::from_square(Square::G7),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . p .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . P . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_piece_capture(Color::Black)
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . p . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . P
            . . . . . . . .
            . . . . . . . .
            ,
            . . . . . . . .
            . p . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . P . .
            . . . . . . . .
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_with_promotions_removes_promoting_pawn_for_white() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::A7),
            Bitboard::from_square(Square::H2),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        P . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . p
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_promotion(Color::White)
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . p
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_with_promotions_from_black_perspective() {
        let parent = PawnStructure::new(
            Bitboard::from_square(Square::H7),
            Bitboard::from_square(Square::A2),
        );
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . P
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        p . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_promotion(Color::Black)
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"
        [
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . .
            . . . . . . . p
            . . . . . . . .
            ,
        ]
        "
        );
    }

    #[test]
    fn child_pawn_structures_no_promotion_possible() {
        let parent = PawnStructure::new(Bitboard::from_square(Square::D3), Bitboard::EMPTY);
        assert_debug_snapshot!(parent.to_board(), @"
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . . . . . .
        . . . P . . . .
        . . . . . . . .
        . . . . . . . .
        ");
        assert_debug_snapshot!(
            parent
                .child_pawn_structures_with_promotion(Color::Black)
                .into_iter()
                .map(|ps| ps.to_board())
                .collect::<Vec<Board>>(), @"[]"
        );
    }
}
