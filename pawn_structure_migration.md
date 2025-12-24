# Pawn Structure Material Key Migration

## Background (current code)

- `MaterialKey` is count-based only (`KQvK`, `KPvK`, …) and stores piece counts per color in `counts`.
- Canonicalization currently:
  - Swaps sides so the stronger side is encoded as white.
  - Optionally flips bishop colors via a left-to-right mirror **only when there are no pawns** (see `MaterialKey::should_swap_bishops`).
- `PositionIndexer` enumerates placements for **all** pieces returned by `MaterialKey::pieces()` (including pawns) with a naive 64/32 radix; overlaps are rejected at runtime.
- WDL files use version `1` with a serialized material key string.

## Goal

Make pawn locations explicit in `MaterialKey` so each pawn square is part of the key (e.g. a lone pawn on `e7` becomes `Ke7vK`). A pawn count without squares is invalid, so keys like `KPvK` are no longer accepted.

## Specification

- **Data model**
  - Extend `MaterialKey` with `pawn_bitboards: [shakmaty::Bitboard; 2]` (or equivalent) to encode exact pawn locations per color.
  - Keep `counts` for compatibility, but derive pawn counts from the bitboards (assert or normalize `counts[color][Pawn] == pawn_bitboards[color].count()`).
  - Ensure `Eq`, `Ord`, and `Hash` include pawn bitboards.
- **Textual encoding**
  - Each side is serialized in canonical order: `K, Q, R, Bd, Bl, N` followed by zero or more pawn squares (`a2`, `h7`, …).
  - **No `P` token** in the new format. Any `P` token should be rejected so `KPvK` is invalid.
  - Pawn squares are emitted in ascending `Square::to_index()` order for stable strings.
  - Parser rules:
    - Uppercase tokens (`K`, `Q`, `R`, `Bd`, `Bl`, `N`) are pieces.
    - Lowercase file/rank pairs (`a1`..`h8`) are pawns.
    - Reject malformed squares, duplicate pawn squares, or unknown tokens.
- **Canonicalization**
  - Keep existing canonicalization behavior:
    - Strong side becomes white (swap `counts` **and** pawn bitboards).
    - Bishop color swap is still allowed **only when no pawns exist**; with pawns present, do not mirror.
- **API changes**
  - `MaterialKey::from_string` parses square tokens for pawns; `P` tokens are invalid.
  - `Display` emits pawn squares instead of `P` tokens.
  - `MaterialKey::from_position` collects pawn bitboards directly from the board scan.
  - Add `pawn_bitboard(color) -> Bitboard` (or `pawn_squares(color) -> impl Iterator<Item=Square>`) for ergonomic use.
  - Update `MaterialKey::pieces()` to **exclude pawns**.
  - Update `MaterialKey::child_material_keys` to take into account possible pawn moves
    - Piece capturing a piece (as before)
    - Piece capturing a pawn
    - Pawn capturing a piece
    - Pawn capturing a piece (and promoting)
    - Pawn moving by 2
    - Pawn moving by 1
    - Pawn moving by 1 (and promoting)
- **Downstream components**
  - `PositionIndexer` must pre-place fixed pawns from the bitboards before indexing other pieces.
    - `index_to_position` should set pawns in `Setup` first, then place the remaining pieces.
    - `position_to_index` should verify pawns occupy the expected squares in debug mode (mismatches → `MismatchedMaterial`)
  - `TableBuilder` should continue using `MaterialKey::from_position` for capture/promotion transitions; child tables now reflect pawn-square-aware keys.
  - `index_pgn` will now count entries keyed by full pawn structures (expect a much larger key space).
  - `wdl_file` should keep the format version to `1`. **no backwards compatibility** is required;
  - CLI and docs should accept the new syntax (e.g. `cargo run --release -- generate Ke7vK`).
- **Validation**
  - Update tests for parsing/formatting and canonicalization with pawn squares.
  - Update `PositionIndexer` tests to cover fixed pawns (both round-trip and mismatched-material cases).
  - Update any tests that reference `KPvK` to use an explicit square (e.g. `Ke7vK`).

## Implementation Plan

1. **MaterialKey data model**
   - Add pawn bitboards, update `has_pawns`, `mirror_sides`, and `canonicalize` to swap them when needed.
   - Update derives and equality/ordering to include pawn bitboards.
2. **Parsing/formatting**
   - Replace `P` parsing with pawn square parsing.
   - Emit pawn squares in deterministic order; adjust tests in `src/material_key/mod.rs`.
3. **Indexing changes**
   - Update `MaterialKey::pieces()` or add a non-pawn iterator.
   - Update `PositionIndexer` to pre-place pawns and ignore them during indexing.
4. **Child key generation**
   - Update `child_material_keys` to account for explicit pawn squares (remove specific squares on capture/promotion).
5. **File format**
   - Wipe all current files because the format changed.
6. **PGN + table generation**
   - Regenerate WDL tables and PGN index under the new key space.
   - The changes to material key names should be reflected in the new file names.
7. **Testing**
   - Run `cargo fmt`, `cargo test`, and (if this is a large change) `cargo test -- --ignored`.
