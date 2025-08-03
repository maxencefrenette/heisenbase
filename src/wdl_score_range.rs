use crate::score::{DtzScore, DtzScoreRange};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WdlScoreRange {
    /// Position that can be a win, draw or loss
    Unknown,
    WinOrDraw,
    DrawOrLoss,
    Win,
    Draw,
    Loss,
    /// This won't be used right now because the TableBuilder doesn't mark illegal positions
    IllegalPosition,
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
