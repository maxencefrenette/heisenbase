use crate::score::{DtzScore, DtzScoreRange};

/// Range of win/draw/loss values stored in a table.
///
/// The discriminants of this enum are important for compression as they are
/// treated as the initial alphabet for the pair‑substitution algorithm.  Keep
/// the values in sync with the `TryFrom<u8>` implementation below.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WdlScoreRange {
    /// Position that can be a win, draw or loss
    Unknown = 0,
    WinOrDraw = 1,
    DrawOrLoss = 2,
    Win = 3,
    Draw = 4,
    Loss = 5,
    /// This won't be used right now because the TableBuilder doesn't mark illegal positions
    IllegalPosition = 6,
}

impl From<WdlScoreRange> for u8 {
    fn from(value: WdlScoreRange) -> Self {
        value as u8
    }
}

impl core::convert::TryFrom<u8> for WdlScoreRange {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use WdlScoreRange::*;
        Ok(match value {
            0 => Unknown,
            1 => WinOrDraw,
            2 => DrawOrLoss,
            3 => Win,
            4 => Draw,
            5 => Loss,
            6 => IllegalPosition,
            _ => return Err(()),
        })
    }
}

impl From<DtzScoreRange> for WdlScoreRange {
    fn from(score: DtzScoreRange) -> Self {
        use WdlScoreRange::*;

        let zero = DtzScore::draw();

        if score.min > zero {
            Win
        } else if score.max < zero {
            Loss
        } else if score.min == zero && score.max == zero {
            Draw
        } else if score.min >= zero && score.max > zero {
            WinOrDraw
        } else if score.min < zero && score.max == zero {
            DrawOrLoss
        } else {
            Unknown
        }
    }
}
