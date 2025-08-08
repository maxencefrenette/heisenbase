use heisenbase::wdl_score_range::WdlScoreRange;

#[test]
fn certainty_checks() {
    assert!(WdlScoreRange::Win.is_certain());
    assert!(WdlScoreRange::Draw.is_certain());
    assert!(WdlScoreRange::Loss.is_certain());

    assert!(WdlScoreRange::WinOrDraw.is_uncertain());
    assert!(WdlScoreRange::DrawOrLoss.is_uncertain());
    assert!(WdlScoreRange::Unknown.is_uncertain());
}
