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

In a pawned position, only 4 valid transformations are allowed

- Identity
- Rotation by 180°
- Horizontal flip
- Vertical flip

## Material key canonicalization

### Sorting

Material keys are first sorted such that the strong side comes first and every piece is ordered in [K, Q, R, Bd, Bl, NP] order. The strong side is defined as the side that wins the first comparison in the following sequence:

1. Compare the number of queens.
2. If tied, compare the number of rooks.
3. If still tied, compare the total number of bishops.
4. If still tied, compare the number of knights.
5. If still tied, compare the number of pawns.

If all of these comparisons are tied, then the material is symmetrical and sides don't matter.

For instance:

- KvQK -> KQvK

### Flipping bishop colors

If the material key contains bishops, the colors of the bishops are flipped to prioritize the material key with the smallest lexicographic order.

For instance:

- KBlvKBd -> KBdvKBl

If this happens, flips are no longer allowed in subsequent steps.

## Position canonicalization

Once the material key is canonicalized, the position itself is canonicalized within that material key.

### King canonicalization

The two kings are canonicalized according to the transformations that are still allowed for a given material key. For each case, the weak king is placed such that it's not adjacent to the strong king.

Case 1: All 8 transformations allowed (pawnless positions, no bishops)

The strong side king is placed in a 10-square wedge in the bottom-left corner of the board (a1, a2, a3, b1, b2, b3, c1, c2, c3, d1). If the strong king is on the diagonal a1-d4, the weak king is placed in the a1-a8-h8 triangle. The weak king is placed such that it's not adjacent to the strong king.

Case 2: 4 rotations allowed (pawnless positions, bishops)

The strong side king is placed in the bottom-left quadrant of the board (16 possible squares). The weak king is placed anywhere else on the board such that it's not adjacent to the strong king.

Case 3: Only horizontal flips allowed (pawnful positions, no bishops)

The strong side king is placed in the left half of the board (32 possible squares). The weak king is placed anywhere else on the board such that it's not adjacent to the strong king.

Case 4: No transformations allowed (pawnful positions, bishops)

The strong side king is placed anywhere on the board. The weak king is placed anywhere else on the board such that it's not adjacent to the strong king.

### Other pieces canonicalization

Once the kings are canonicalized, for the vast majority of KK buckets, there is no remaining transformation allowed. For simplicity, we will not apply any further transformation even when such cases are allowed.
