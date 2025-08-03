# Heisenbase WDL Table File Format

This document describes the on‑disk format used for tables produced by the
`generate` command.  A file contains a single compressed WDL table.  All
multi‑byte integers are encoded in **little endian** byte order.

## Overview

| Offset | Size | Description |
| ------ | ---- | ----------- |
| 0      | 4    | Magic bytes `HBWD` identifying a WDL table |
| 4      | 1    | Format version.  Currently `1` |
| 5      | 1    | Length *N* of the material key string |
| 6      | N    | ASCII material key (e.g. `KQvK`) |
| 6+N    | 8    | Original table length (number of positions) |
| 14+N   | 2    | Number of base symbols used for encoding |
| 16+N   | 2    | Number *P* of substitution pairs |
| …      | 4×P  | Each pair as two `u16` values `(a, b)` |
| …      | 2    | Length *L* of the `code_lens` array |
| …      | L    | Huffman code lengths (one `u8` per symbol) |
| …      | 8    | Number of valid bits in the bitstream |
| …      | 4    | Length *B* of the bitstream in bytes |
| …      | B    | Huffman encoded data |

The exact size of sections after the header depends on the table and the
compression results.  The sections appear consecutively without padding.

### Material Key

The material key identifies which pieces are present in the table.  It is
stored as a compact ASCII string such as `KQvK` where `v` separates the
white and black pieces.  Its length is limited to 255 bytes.

### Symbol Pairs

Pair substitution replaces frequent adjacent value pairs with new symbols.
`P` indicates how many new symbols were created.  For each new symbol the
file contains the original pair `(a, b)` represented as two `u16` values.
Symbols are numbered sequentially starting at `base_symbols`.

### Code Lengths and Bitstream

The `code_lens` array specifies the Huffman code length for each symbol.
The encoded table is stored as a bitstream preceded by its exact bit
length.  The bitstream is padded to whole bytes.

## Versioning

The version byte allows future changes.  Readers should verify the value
before interpreting the rest of the file.  Files produced by the current
implementation use version `1`.

## Example

The following pseudo‑code illustrates how a file can be read:

```text
read magic, version
assert magic == "HBWD" && version == 1
read material_key_length
read material_key_string
read orig_len
read base_symbols
read pair_count and pair entries
read code_len_count and code_lens
read bit_len and bitstream
```

The `read_wdl_file` function in the codebase follows this specification.

