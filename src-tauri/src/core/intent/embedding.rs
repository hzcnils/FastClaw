//! 嵌入计算与相似度匹配
//!
//! 用归一化向量的内积（等价于余弦相似度）匹配用户意图与 MCP 工具描述。

/// 计算两个归一化向量的余弦相似度
///
/// 输入向量应为 L2 归一化后的结果，此时内积 = 余弦相似度。
#[inline]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "向量维度不一致");
    let mut dot = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
    }
    // 归一化向量点积范围 [-1, 1]，限制到 [0, 1]
    dot.max(0.0)
}

/// 在候选嵌入中查找最佳匹配
///
/// 返回 (索引, 相似度分数)，按分数降序排列的前 top_k 个结果。
pub fn find_best_matches(
    query: &[f32],
    candidates: &[Vec<f32>],
    top_k: usize,
) -> Vec<(usize, f32)> {
    let mut scores: Vec<(usize, f32)> = candidates
        .iter()
        .enumerate()
        .map(|(i, c)| (i, cosine_similarity(query, c)))
        .collect();

    // 按分数降序排列
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scores.truncate(top_k);
    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_identical() {
        let v = vec![1.0f32, 0.0, 0.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_cosine_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        assert!((cosine_similarity(&a, &b) - 0.0).abs() < 0.001);
    }

    #[test]
    fn test_find_best_matches() {
        let query = vec![1.0, 0.0, 0.0];
        let candidates = vec![
            vec![1.0, 0.0, 0.0],     // score = 1.0
            vec![0.0, 1.0, 0.0],     // score = 0.0
            vec![0.707, 0.707, 0.0], // score ≈ 0.707
        ];
        let matches = find_best_matches(&query, &candidates, 2);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].0, 0); // 第一个候选最匹配
        assert!((matches[0].1 - 1.0).abs() < 0.01);
    }
}
