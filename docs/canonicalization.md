# Canonicalization

Positions need to be canonicalized to ensure that each position's information is stored only once. This is a two-stage process. First, the material key is canonicalized, then the position itself is canonicalized within that material key.

## Side to move canonicalization

In positions where each side has exactly the same material (e.g. KQvKQ, KRvKR, KBdvKBd, etc.), we can flip the side to move. This is orthogonal to everything that follows. The long-term plan is to handle this by ignoring the side to move when mapping positions to an index in the table. The current implementation still records the side to move, so symmetric positions for opposite players map to distinct indices.

## Starting point

In a pawnless position, there are 8 valid transformations that can be made without loss of generality:

- Identity
- Rotation by 90°
- Rotation by 180°
- Rotation by 270°
- Horizontal flip
- Vertical flip
- Diagonal flip
- Anti-diagonal flip

When bishops are present but no pawns exist, the diagonal flips are no longer available and only the four rotations remain.

When pawns are on the board, they constrain which transforms are legal because their exact locations are preserved in the material key:

- Identity
- Horizontal flip (no bishops)

If bishops and pawns are both present, no geometric transforms remain; the identity transform is the only legal choice.

## Material key canonicalization

### Sorting

Material keys are first sorted such that the strong side comes first and every non-pawn piece is ordered in [K, Q, R, Bd, Bl, N] order. The strong side is defined as the side that wins the first comparison in the following sequence:

1. Compare the number of queens.
2. If tied, compare the number of rooks.
3. If still tied, compare the total number of bishops.
4. If still tied, compare the number of knights.
5. If still tied, compare the number of pawns.

If all of these comparisons are tied, then the material is symmetrical and sides don't matter.

For instance:

- KvQK -> KQvK

### Bishops and pawns

If the material key contains bishops **and no pawns**, the colors of the bishops are flipped to prioritize the material key with the smallest lexicographic order.

For instance:

- KBlvKBd -> KBdvKBl

If this happens, flips are no longer allowed in subsequent steps.

When pawns are present we never flip bishop colors because that would change the pawn structure.

### Pawn serialization

Pawns are encoded by listing their algebraic squares (e.g. `a2`, `h7`) after all piece tokens for that side. The squares are taken directly from the pawn bitboards and emitted in ascending order by their numeric index (`Square::to_u32`). Examples:

- `Ke7vK` – strong side king and a pawn on e7.
- `KQb7c7vKRg2` – strong side queen with pawns on b7 and c7, weak side king with a pawn on g2.

### Pawn orientation minimization

After sorting and any bishop flip, the canonicalizer evaluates every transform permitted by the material (see the "Starting point" section). It applies each transform to the pawn bitboards and picks the lexicographically smallest serialization. The selected transform defines the canonical orientation for the entire key; both the pawn bitboards and the implicit choice of transform are carried forward so that downstream consumers use the same orientation.

## Position canonicalization

Once the material key is canonicalized, the position itself is canonicalized within that material key.

### King canonicalization

The two kings are canonicalized according to the transformations that are still allowed for a given material key. For each case, the weak king is placed such that it's not adjacent to the strong king.

Case 1: All 8 transformations allowed (pawnless positions, no bishops)

The strong side king is placed in a 10-square wedge in the bottom-left corner of the board (a1, a2, a3, b1, b2, b3, c1, c2, c3, d1). If the strong king is on the diagonal a1-d4, the weak king is placed in the a1-a8-h8 triangle. The weak king is placed such that it's not adjacent to the strong king.

Case 2: 4 rotations allowed (pawnless positions, bishops)

The strong side king is placed in the bottom-left quadrant of the board (16 possible squares). The weak king is placed anywhere else on the board such that it's not adjacent to the strong king.

Case 3: Only horizontal flips allowed (positions with pawns and no bishops)

The strong side king is placed in the left half of the board (32 possible squares). The weak king is placed anywhere else on the board such that it's not adjacent to the strong king. The horizontal flip is only applied if it keeps the pawns on the squares chosen during material canonicalization.

Case 4: No transformations allowed (positions with pawns and bishops)

The strong side king is placed anywhere on the board. The weak king is placed anywhere else on the board such that it's not adjacent to the strong king.

### Other pieces canonicalization

Once the kings are canonicalized, the pawn bitboards from the material key are pre-placed on their fixed squares. For the vast majority of KK buckets, no transformation remains; we therefore distribute the remaining non-pawn pieces without applying additional symmetry reductions.
