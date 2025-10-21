# Coding Conventions

This document outlines the recurring conventions we lean on across the codebase so engineers can interpret material descriptors, color semantics, and indices consistently.

- White is the side with more material in a table, and the strong side is encoded as white whenever we canonicalize positions. This keeps material keys and generated positions consistent regardless of the original orientation.
- Piece tokens follow chess notation with light/dark bishops distinguished by `Bl` and `Bd`, ensuring material strings stay unambiguous when we parse or format them.
- When storing color-indexed data, arrays use index `0` for white and `1` for black, an assumption relied upon throughout position encoding logic.
