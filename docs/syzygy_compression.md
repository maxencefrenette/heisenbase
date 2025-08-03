# Compression in the Syzygy Tablebase Generator (tb) Repository

## Overview

Syzygy tablebases provide perfect information for chess endgames. The tb repository implements tools for generating, compressing and verifying these tablebases for win‑draw‑loss (WDL) and distance‑to‑zeroing (DTZ) values. A tablebase encodes up to eight pieces, and its raw data are long arrays of small integers: WDL values range from 0–4 and DTZ values can be larger. To make the tables portable and efficient to probe, the generator compresses the arrays into .rtbw (WDL) and .rtbz (DTZ) files.

## Compression involves three major phases:

Pair substitution (grammar‑based) compression: frequently occurring adjacent value pairs in the table are replaced by new symbols, recursively building a symbol table of replacements. This reduces repetitive patterns in the data. The symtable holds the pattern for each symbol and the pairfreq array stores pair frequencies.

## Huffman coding

Huffman coding: after pair substitution, the sequence of symbols is encoded with a length‑limited Huffman code. Frequencies for each symbol are counted, a package‑merge algorithm assigns code lengths and the codebook is sorted. Lengths and offsets for each code length are saved in the file header so decoding can be constant‑time.

## Block packing and indexing

Block packing and indexing: the bitstream of Huffman codes is split into blocks of size 1<<blocksize. An index table provides fast random access by storing, for each main index, the block number and bit offset. A size table stores the compressed length of each block. These tables make it possible to probe positions without decompressing the entire file.

The remainder of this report details each phase and the file format.

## Pre‑processing and handling "don’t care" values

A table contains entries that are irrelevant for play (for example, positions where the side to move has only illegal moves or where different move counts yield the same result). Such entries are marked as "don’t care" values using a special symbol (>4 for WDL tables). The compression code runs a remove/replace‑don’t‑care pass that coalesces sequences of "don’t care" values and determines replacement pairs using a small lookup table. The remove_wdl_worker function iterates through the data, identifies sequences of values ≥5 and updates them based on adjacent context. For DTZ tables, similar logic fills gaps with best pairs to maximise compression.

## Pair frequency counting

After handling "don’t care" values, the compressor counts frequencies of all adjacent value pairs. Each thread starts at a different index; for WDL tables the count_pairs_wdl function reads the data and increments countfreq[s1][s2] for each adjacent pair (s1, s2). DTZ tables have separate counters for 8‑bit and 16‑bit values.

## Selecting and replacing pairs

The pair substitution algorithm is similar to Re‑Pair: it repeatedly identifies a pair with high frequency, assigns it a new symbol and replaces all non‑overlapping occurrences of that pair. The paircands array lists candidate pairs, sorted by decreasing frequency. When a pair (s1,s2) is chosen, its replacement symbol is recorded in symtable[sym] (with pattern [s1,s2] and len=2). After replacement, new frequencies are updated and the algorithm continues until no pair yields sufficient savings or the maximum number of symbols is reached.

The following diagram shows how frequent pairs are replaced by a new symbol S. The original sequence contains repeated occurrences of 1 1. These pairs are replaced by S, and the symbol table stores S → (1,1).

## Symbol table structure

symtable entries contain:

pattern[2] – either two 16‑bit indices referencing other symbols or a single 8‑bit value and an extra byte when the symbol represents a base value.

len – length of the pattern (1 for base symbols or >1 for replacement pairs).

Symbols are numbered so that base values appear first and replacement symbols follow. The compressor maintains arrays pairfirst and pairsecond mapping pairs to their first and second components and uses them to iteratively replace pairs.

## Huffman coding

After pair substitution, the table is now a sequence of symbols with lengths given by symtable[i].len. Frequencies for each symbol are counted and a Huffman code is constructed. The code uses a package‑merge algorithm (implemented in create_code) to create a prefix code with a limited maximum length. The function first builds a list of non‑zero frequency symbols and sorts them by increasing frequency. The package_merge routine combines the two lowest frequencies repeatedly, storing partial sums in lists, and produces a multiset of code lengths. After the merge, each symbol is assigned a length (depth in the Huffman tree) and the codes are sorted so that shorter codes map to more frequent symbols. The algorithm then computes offset[l] and base[l] arrays: offset[l] gives the index of the first symbol with code length l, and base[l] stores the threshold used during decoding (values smaller than base[l] require increasing l). These arrays and the minimum and maximum code lengths (min_len, max_len) are written to the file header.

The figure below illustrates Huffman coding for four example symbols. More frequent symbols receive shorter codes, producing efficient compression.

## Block packing and indexing

After Huffman coding, the bitstream of codes is packed into blocks of size 2^blocksize bytes. A block may contain multiple symbols, but the compressor ensures that the start of a block aligns with a Huffman code boundary for efficient decoding. Two auxiliary tables provide random access:

Index table (main index) – Each entry is 6 bytes: a 4‑byte block number and a 2‑byte bit offset. The idxbits value determines how many entries exist; 2^{idxbits‑1} bytes of uncompressed data correspond to one index entry. When decompressing, the code divides the desired position index by 2^{idxbits} to find the main index, uses the block number and offset to locate the correct compressed block, and adjusts within the block.

Size table – A 2‑byte value for each block storing its compressed length minus 1. This makes it possible to skip blocks during decompression or probing. If the compressed data in a block does not fill the entire block, the remaining bytes are padded to the next 64‑byte boundary.

The compressed data follow the index and size tables. Each block contains a 64‑bit aligned bitstream of Huffman codes; decoders treat it as a big‑endian integer and extract codes by comparing the top bits against the base array.

The diagram below shows how the compressed bitstream is split into blocks and how the index and size tables map positions into blocks.

## File format and header

A .rtbw or .rtbz file stores one or more sub‑tables (for symmetry reasons there may be separate tables for each ordering of pieces or each combination of pawns). The header for each sub‑table includes:

Flags (1 byte): indicate whether the table stores a single value (0x80 set), whether it is a WDL or DTZ table, which side to move is to be probed and whether the values are ply‑accurate.

Blocksize (1 byte): blocksize[i], controlling the number of bytes per block (log₂ block size).

Index bits (1 byte): idxbits[i], controlling the distance between index entries.

Number of blocks (4 bytes) and the difference between the real and nominal number of blocks (1 byte); this difference accounts for trailing zero blocks that can be omitted.

Huffman code description: max_len and min_len (1 byte each) followed by offset[l] for each length between min_len and max_len.

Symbol table: The number of symbols and, for each symbol in sorted order, a three‑byte record encoding either a single base value or two symbol indices (with 12‑bit fields).

DTZ maps/WDL flags: For DTZ tables, a map translates internal values to DTZ numbers. For WDL tables, flags record whether each of the five WDL values (loss, cursed win, draw, blessed win, win) appears.

After the headers of all sub‑tables, the file aligns to a 2‑byte boundary. Then the index table and the size table are concatenated. Finally, the compressed block data are written and padded to a 64‑byte boundary.

## Decompression

The decompression code mirrors the compressor. When opening a table, open_tb memory‑maps the .rtbw/.rtbz file and reads the header. For each sub‑table, decomp_setup_pairs reads the Huffman code description, symbol table and populates a PairsData structure storing min_len, blocksize, idxbits, sizetable, indextable and the symbol tree. The symbol lengths (symlen) and patterns (sympat) are pre‑computed using calc_symlen.

decompress_table returns the uncompressed array for a given side‑to‑move. It divides the work across threads; each thread calls decompress_worker, which performs the following steps:

Main index lookup: The desired index is split into a main index and local index. The main index gives the block number and offset in the block.

Adjust for literal index: If the local index falls before the start of the block or after its end, the algorithm walks backwards or forwards using the size table to find the correct block and bit offset.

Decode the bitstream: It reads 64‑bit chunks from the block, uses the base array to determine the code length (l) for the next symbol and obtains the symbol index via offset[l] + ((code – base[l]) >> (64 – l)). After each symbol, bits are shifted left and more bits are fetched as needed.

Expand symbols: The recursive function expand_symbol (from compress_tmpl.c) expands non‑terminal symbols into their two component symbols until base values are produced. These values are written into the destination array. expand_symbol uses sympat and symlen to traverse the replacement tree.

Advance to next block: After decoding all symbols in a block, the decoder moves to the next block using the size table.

The index and size tables mean that the decompressor can start decoding near any position, enabling engines to probe tablebases without decompressing earlier positions. The process is summarised in the flow diagram below.

## Conclusion

The Syzygy tablebase generator uses a sophisticated combination of grammar‑based pair compression, length‑limited Huffman coding and block‑based indexing to achieve high compression ratios while maintaining efficient random access. Frequent adjacent pairs of endgame values are replaced by new symbols and recursively reduced; the resulting symbol stream is encoded with an optimal Huffman code; and the bitstream is partitioned into blocks with a main index so individual positions can be retrieved quickly. The header stores all information needed for decompression: code lengths, symbol table, block sizes and offsets. This design produces small, portable tablebases that can be probed rapidly by chess engines.
