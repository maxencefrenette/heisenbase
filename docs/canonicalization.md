# Canonicalization

Positions need to be canonicalized to ensure that each position's information is stored only once. For now, only the material key is canonicalized.

## Pawn-structure canonicalization

When pawn structures differ between sides, we canonicalize by ordering the pawn bitboards. We compare White pawns against a vertically flipped view of Black pawns; if the white pawn structure is lexicographically smaller, we swap sides so the smaller pawn structure is encoded as white. When the pawn structures are not equal after the vertical flip, this ordering is the only canonicalization applied: the stronger side is **not** forced to be white in asymmetric pawn cases.

## Side to move canonicalization

In positions where each side has exactly the same material (e.g. KQvKQ, KRvKR, KBdvKBd, etc.), we can flip the side to move. This is orthogonal to everything that follows. The long-term plan is to handle this by ignoring the side to move when mapping positions to an index in the table. The current implementation still records the side to move, so symmetric positions for opposite players map to distinct indices.

## Bishop color canonicalization

For bishop material, the color of the squares the bishops live on can be canonicalized by mirroring the board left-to-right. A left-to-right mirror only swaps the color of all bishops in a material key and does nothing else. When a position contains only bishops of one color complex (e.g. KBvK, KBvKB, KBBvK, or KBBvKB where all bishops are on light squares or all on dark squares), we can mirror the position so that the bishops are always treated as being on a single canonical color. By convention, we mirror such cases so that we get light-squared bishops. This reduces material keys duplicates between otherwise identical positions that differ only by a global color flip.

When both color complexes are present (e.g. opposite-colored bishops, or a side has both bishops), no bishop color canonicalization is applied because a global mirror does not preserve the material distribution.
Even when only one color complex is present, bishop color canonicalization is applied only if the pawn structure is horizontally symmetric; otherwise a left-to-right mirror would change the pawn layout.
