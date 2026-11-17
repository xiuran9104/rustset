pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        dot += x * y;
        aa += x * x;
        bb += y * y
    }
    if aa == 0.0 || bb == 0.0 {
        0.0
    } else {
        dot / (aa.sqrt() * bb.sqrt())
    }
}
pub fn split(content: &str, max_tokens: usize) -> Vec<String> {
    let max_chars = max_tokens.max(1) * 4;
    let chars = content.chars().collect::<Vec<_>>();
    chars
        .chunks(max_chars)
        .map(|x| x.iter().collect::<String>().trim().to_string())
        .filter(|x| !x.is_empty())
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cosine_identity() {
        assert!((cosine(&[1.0, 2.0], &[1.0, 2.0]) - 1.0).abs() < 0.0001)
    }
    #[test]
    fn splits_utf8_safely() {
        assert_eq!(split("你好世界", 1), vec!["你好世界"]);
    }
}
