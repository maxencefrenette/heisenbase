use shakmaty::{Color, Role};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HbPieceRole {
    King,
    Queen,
    Rook,
    LightBishop,
    DarkBishop,
    Knight,
}

impl HbPieceRole {
    pub const ALL: [HbPieceRole; 6] = [
        HbPieceRole::King,
        HbPieceRole::Queen,
        HbPieceRole::Rook,
        HbPieceRole::DarkBishop,
        HbPieceRole::LightBishop,
        HbPieceRole::Knight,
    ];

    pub const CAPTURABLE: [HbPieceRole; 5] = [
        HbPieceRole::Queen,
        HbPieceRole::Rook,
        HbPieceRole::DarkBishop,
        HbPieceRole::LightBishop,
        HbPieceRole::Knight,
    ];

    pub fn token(self) -> &'static str {
        match self {
            HbPieceRole::King => "K",
            HbPieceRole::Queen => "Q",
            HbPieceRole::Rook => "R",
            HbPieceRole::DarkBishop => "Bd",
            HbPieceRole::LightBishop => "Bl",
            HbPieceRole::Knight => "N",
        }
    }

    pub fn from_token(tok: &str) -> Option<Self> {
        match tok {
            "K" => Some(HbPieceRole::King),
            "Q" => Some(HbPieceRole::Queen),
            "R" => Some(HbPieceRole::Rook),
            "Bd" => Some(HbPieceRole::DarkBishop),
            "Bl" => Some(HbPieceRole::LightBishop),
            "N" => Some(HbPieceRole::Knight),
            _ => None,
        }
    }

    pub fn role(self) -> Role {
        match self {
            HbPieceRole::King => Role::King,
            HbPieceRole::Queen => Role::Queen,
            HbPieceRole::Rook => Role::Rook,
            HbPieceRole::DarkBishop | HbPieceRole::LightBishop => Role::Bishop,
            HbPieceRole::Knight => Role::Knight,
        }
    }

    pub fn is_bishop(self) -> bool {
        match self {
            HbPieceRole::DarkBishop | HbPieceRole::LightBishop => true,
            _ => false,
        }
    }
}

/// Struct that represents a chess piece with a role and a color.
///
/// This is similar to the `Piece` struct in the `shakmaty` crate, except for the following differences:
/// - It differentiates between light and dark bishops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HbPiece {
    pub role: HbPieceRole,
    pub color: Color,
}

impl From<HbPiece> for shakmaty::Piece {
    fn from(hb_piece: HbPiece) -> shakmaty::Piece {
        shakmaty::Piece {
            role: hb_piece.role.role(),
            color: hb_piece.color,
        }
    }
}
