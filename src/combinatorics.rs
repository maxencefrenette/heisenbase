pub fn n_choose_k(n: usize, k: usize) -> usize {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut result: usize = 1;
    for i in 1..=k {
        result = result * (n - (k - i)) / i;
    }
    result
}

pub fn unrank_combination(n: usize, k: usize, mut rank: usize) -> Vec<usize> {
    let mut combo = Vec::with_capacity(k);
    let mut x = 0usize;
    for i in 0..k {
        let mut c = x;
        loop {
            let count = n_choose_k(n - c - 1, k - i - 1);
            if count <= rank {
                rank -= count;
                c += 1;
            } else {
                combo.push(c);
                x = c + 1;
                break;
            }
        }
    }
    combo
}

pub fn rank_combination(n: usize, indices: &[usize]) -> usize {
    let k = indices.len();
    let mut rank = 0usize;
    for (i, &c) in indices.iter().enumerate() {
        let start = if i == 0 { 0 } else { indices[i - 1] + 1 };
        for j in start..c {
            rank += n_choose_k(n - j - 1, k - i - 1);
        }
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::{n_choose_k, rank_combination, unrank_combination};

    #[test]
    fn binomial_values() {
        assert_eq!(n_choose_k(0, 0), 1);
        assert_eq!(n_choose_k(5, 2), 10);
        assert_eq!(n_choose_k(5, 3), 10);
        assert_eq!(n_choose_k(5, 6), 0);
    }

    #[test]
    fn combination_roundtrip() {
        let n = 8;
        let k = 3;
        for rank in 0..n_choose_k(n, k) {
            let combo = unrank_combination(n, k, rank);
            assert_eq!(combo.len(), k);
            assert!(combo.windows(2).all(|w| w[0] < w[1]));
            let reranked = rank_combination(n, &combo);
            assert_eq!(rank, reranked);
        }
    }
}
