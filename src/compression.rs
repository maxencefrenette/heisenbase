// Implementation of a simplified compression scheme inspired by the
// Syzygy tablebases.  It performs pair substitution followed by a
// canonical Huffman coding of the resulting symbol stream.

use crate::wdl_score_range::WdlScoreRange;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Node in the Huffman decoding tree.
///
/// Each leaf represents exactly one symbol and has no children. Internal nodes
/// have their `left`/`right` child set but keep `symbol` as `None`. Traversing
/// from the root by taking the left branch for a `0` bit and the right branch
/// for a `1` mirrors the canonical code assignment, and decoding relies on these
/// invariants to map a bit sequence back to the original symbol stream.
#[derive(Default)]
struct HuffmanNode {
    left: Option<usize>,
    right: Option<usize>,
    symbol: Option<u16>,
}

/// Result of compressing a sequence of `WdlScoreRange` values.
#[derive(Debug, Clone)]
pub struct CompressedWdl {
    /// Number of base symbols. For `WdlScoreRange` this is fixed but storing it
    /// makes the format self contained.
    pub base_symbols: u16,
    /// Pairs for newly created symbols. Symbol `i` (>= base_symbols) expands to
    /// `sym_pairs[i - base_symbols]`.
    pub sym_pairs: Vec<(u16, u16)>,
    /// Huffman code lengths for all symbols (base and generated).
    pub code_lens: Vec<u8>,
    /// Encoded bit stream.
    pub bitstream: Vec<u8>,
    /// Number of valid bits in `bitstream`.
    pub bit_len: usize,
    /// Length of the decompressed table.
    pub orig_len: usize,
}

/// Compress a slice of `WdlScoreRange` values using pair substitution and
/// Huffman coding.
pub fn compress_wdl(values: &[WdlScoreRange]) -> CompressedWdl {
    let base_symbols = 7u16; // number of possible WDL values
    let mut raw: Vec<u8> = values.iter().map(|&v| u8::from(v)).collect();
    let illegal_code = u8::from(WdlScoreRange::IllegalPosition);

    rewrite_illegal_runs(&mut raw, illegal_code);

    let seq: Vec<u16> = raw.into_iter().map(u16::from).collect();

    let (seq, sym_pairs) = pair_substitution(seq, base_symbols);

    let symbols_count = base_symbols as usize + sym_pairs.len();
    let code_lens = build_huffman_code_lengths(&seq, symbols_count);
    let codes = build_codes_from_lengths(&code_lens);

    // Encode the sequence
    let mut bits: Vec<u8> = Vec::new();
    for &sym in &seq {
        let (code, len) = codes[sym as usize];
        for i in (0..len).rev() {
            bits.push(((code >> i) & 1) as u8);
        }
    }
    let bit_len = bits.len();
    let mut bitstream = vec![0u8; (bit_len + 7) / 8];
    for (i, bit) in bits.into_iter().enumerate() {
        if bit == 1 {
            bitstream[i / 8] |= 1 << (7 - (i % 8));
        }
    }

    CompressedWdl {
        base_symbols,
        sym_pairs,
        code_lens,
        bitstream,
        bit_len,
        orig_len: values.len(),
    }
}

/// Decompress a previously compressed WDL table.
pub fn decompress_wdl(data: &CompressedWdl) -> Vec<WdlScoreRange> {
    let codes = build_codes_from_lengths(&data.code_lens);
    let nodes = build_decoding_tree(&codes);
    let seq = decode_bitstream(&data.bitstream, data.bit_len, &nodes, data.orig_len);

    // Expand symbols back to base values
    let mut output: Vec<u16> = Vec::new();
    for sym in seq {
        expand_symbol(sym, &data.sym_pairs, data.base_symbols, &mut output);
    }
    assert_eq!(output.len(), data.orig_len);

    output
        .into_iter()
        .map(|v| WdlScoreRange::try_from(v as u8).expect("invalid wdl value"))
        .collect()
}
/// Build a Huffman decoding tree from `(code, length)` pairs.
///
/// The tree mirrors the canonical code assignment: a `0` bit takes the `left`
/// branch and a `1` bit the `right`. Each code inserts a leaf node whose
/// `symbol` field holds exactly one value and has no children, while internal
/// nodes never store a `symbol`. Decoding walks this structure so that
/// consuming the bits of a code always ends at its corresponding leaf.
fn build_decoding_tree(codes: &[(u32, u8)]) -> Vec<HuffmanNode> {
    let mut nodes = vec![HuffmanNode::default()]; // root
    for (sym, &(code, len)) in codes.iter().enumerate() {
        if len == 0 {
            continue;
        }

        let mut idx = 0usize;
        for i in (0..len).rev() {
            let bit = (code >> i) & 1;
            let next = if bit == 0 {
                nodes[idx].left
            } else {
                nodes[idx].right
            };

            idx = if let Some(n) = next {
                n
            } else {
                nodes.push(HuffmanNode::default());
                let new_idx = nodes.len() - 1;
                if bit == 0 {
                    nodes[idx].left = Some(new_idx);
                } else {
                    nodes[idx].right = Some(new_idx);
                }
                new_idx
            };
        }
        nodes[idx].symbol = Some(sym as u16);
    }
    nodes
}

fn decode_bitstream(
    bitstream: &[u8],
    bit_len: usize,
    nodes: &[HuffmanNode],
    orig_len: usize,
) -> Vec<u16> {
    let mut seq: Vec<u16> = Vec::new();
    let mut idx = 0usize;
    for bit_index in 0..bit_len {
        let byte = bitstream[bit_index / 8];
        let bit = (byte >> (7 - (bit_index % 8))) & 1;
        idx = if bit == 0 {
            nodes[idx].left.expect("missing left child")
        } else {
            nodes[idx].right.expect("missing right child")
        };
        if let Some(sym) = nodes[idx].symbol {
            seq.push(sym);
            if seq.len() >= orig_len {
                break;
            }
            idx = 0;
        }
    }
    seq
}

fn expand_symbol(sym: u16, sym_pairs: &[(u16, u16)], base: u16, out: &mut Vec<u16>) {
    if sym < base {
        out.push(sym);
    } else {
        let (a, b) = sym_pairs[(sym - base) as usize];
        expand_symbol(a, sym_pairs, base, out);
        expand_symbol(b, sym_pairs, base, out);
    }
}

fn rewrite_illegal_runs(seq: &mut [u8], illegal_code: u8) {
    let len = seq.len();
    let fallback = u8::from(WdlScoreRange::Draw);
    let mut i = 0usize;
    while i < len {
        if seq[i] != illegal_code {
            i += 1;
            continue;
        }

        let start = i;
        while i < len && seq[i] == illegal_code {
            i += 1;
        }
        let end = i;

        let left = if start > 0 {
            Some(seq[start - 1])
        } else {
            None
        };
        let right = if end < len { Some(seq[end]) } else { None };

        let replacement = match (left, right) {
            (Some(l), Some(r)) if l == r => l,
            (Some(l), Some(r)) => {
                let mut left_len = 0usize;
                let mut idx = start;
                while idx > 0 {
                    idx -= 1;
                    if seq[idx] == l {
                        left_len += 1;
                    } else {
                        break;
                    }
                }

                let mut right_len = 0usize;
                let mut idx = end;
                while idx < len {
                    if seq[idx] == r {
                        right_len += 1;
                        idx += 1;
                    } else {
                        break;
                    }
                }

                if left_len >= right_len { l } else { r }
            }
            (Some(l), None) => l,
            (None, Some(r)) => r,
            (None, None) => fallback,
        };

        for value in &mut seq[start..end] {
            *value = replacement;
        }
    }
}

fn pair_substitution(mut seq: Vec<u16>, base: u16) -> (Vec<u16>, Vec<(u16, u16)>) {
    let mut sym_pairs: Vec<(u16, u16)> = Vec::new();
    let mut next_sym = base;

    loop {
        let mut freq: HashMap<(u16, u16), usize> = HashMap::new();
        for w in seq.windows(2) {
            *freq.entry((w[0], w[1])).or_insert(0) += 1;
        }
        let (pair, count) = match freq.into_iter().max_by_key(|(_, c)| *c) {
            Some((p, c)) => (p, c),
            None => break,
        };
        if count <= 1 {
            break;
        }
        let new_sym = next_sym;
        next_sym += 1;
        sym_pairs.push(pair);
        let mut new_seq: Vec<u16> = Vec::with_capacity(seq.len());
        let mut i = 0usize;
        while i < seq.len() {
            if i + 1 < seq.len() && (seq[i], seq[i + 1]) == pair {
                new_seq.push(new_sym);
                i += 2;
            } else {
                new_seq.push(seq[i]);
                i += 1;
            }
        }
        seq = new_seq;
    }

    (seq, sym_pairs)
}

#[derive(Clone, Copy)]
struct HuffNode {
    left: Option<usize>,
    right: Option<usize>,
    symbol: Option<usize>,
}

fn build_huffman_code_lengths(seq: &[u16], symbols_count: usize) -> Vec<u8> {
    let mut freqs = vec![0usize; symbols_count];
    for &s in seq {
        freqs[s as usize] += 1;
    }

    let mut heap: BinaryHeap<Reverse<(usize, usize)>> = BinaryHeap::new();
    let mut nodes: Vec<HuffNode> = Vec::new();
    for (sym, freq) in freqs.iter().enumerate() {
        if *freq > 0 {
            nodes.push(HuffNode {
                left: None,
                right: None,
                symbol: Some(sym),
            });
            heap.push(Reverse((*freq, nodes.len() - 1)));
        }
    }

    if heap.is_empty() {
        return vec![0u8; symbols_count];
    }

    while heap.len() > 1 {
        let Reverse((f1, i1)) = heap.pop().unwrap();
        let Reverse((f2, i2)) = heap.pop().unwrap();
        nodes.push(HuffNode {
            left: Some(i1),
            right: Some(i2),
            symbol: None,
        });
        heap.push(Reverse((f1 + f2, nodes.len() - 1)));
    }
    let root = heap.pop().unwrap().0.1;
    let mut lengths = vec![0u8; symbols_count];
    assign_lengths(&nodes, root, 0, &mut lengths);
    lengths
}

fn assign_lengths(nodes: &[HuffNode], idx: usize, depth: u8, lengths: &mut [u8]) {
    let node = nodes[idx];
    if let Some(sym) = node.symbol {
        lengths[sym] = if depth == 0 { 1 } else { depth };
    } else {
        assign_lengths(nodes, node.left.unwrap(), depth + 1, lengths);
        assign_lengths(nodes, node.right.unwrap(), depth + 1, lengths);
    }
}

fn build_codes_from_lengths(code_lens: &[u8]) -> Vec<(u32, u8)> {
    let mut entries: Vec<(usize, u8)> = code_lens
        .iter()
        .enumerate()
        .filter(|(_, l)| **l > 0)
        .map(|(i, &l)| (i, l))
        .collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));

    let mut codes = vec![(0u32, 0u8); code_lens.len()];
    let mut code: u32 = 0;
    let mut prev_len: u8 = 0;
    for (sym, len) in entries {
        code <<= (len - prev_len) as u32;
        codes[sym] = (code, len);
        code += 1;
        prev_len = len;
    }
    codes
}

#[cfg(test)]
mod tests {
    use super::*;
    use WdlScoreRange::*;

    #[test]
    fn round_trip_simple() {
        let data = vec![Win, Win, Draw, Draw, Win, Win, Draw, Draw];
        let compressed = compress_wdl(&data);
        let decompressed = decompress_wdl(&compressed);
        assert_eq!(decompressed, data);
    }

    #[test]
    fn round_trip_mixed_values() {
        let data = vec![Win, Draw, Loss, WinOrDraw, DrawOrLoss, Draw, Win, Loss];
        let compressed = compress_wdl(&data);
        let decompressed = decompress_wdl(&compressed);
        assert_eq!(decompressed, data);
    }

    #[test]
    fn compression_is_effective_for_repetition() {
        let data = vec![Win; 100];
        let compressed = compress_wdl(&data);
        // Bitstream should be smaller than the original 100 bytes.
        assert!(compressed.bitstream.len() < data.len());
        let decompressed = decompress_wdl(&compressed);
        assert_eq!(decompressed, data);
    }
}
