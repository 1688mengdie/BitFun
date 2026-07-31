//! Chunker — 文本分块引擎。
//!
//! 支持三种分块策略：Fixed（固定字符+重叠）、Paragraph（段落）、Sentence（句子）。
//!
//! 参考: gbrain (MIT) chunkers/ — R-5-201 — Rust 翻译实现

use taiji_types::knowledge::ChunkStrategy;

/// 未分配的文本分块（分块后的原始片段）。
#[derive(Debug, Clone)]
pub struct ChunkInput {
    pub seq: usize,
    pub text: String,
}

/// 分块引擎（R-5-201）。
pub struct Chunker;

impl Chunker {
    /// 将文本按策略分块。
    ///
    /// # 参数
    /// - `text`: 原始文本
    /// - `strategy`: 分块策略
    ///
    /// # 返回
    /// 按顺序排列的 ChunkInput 列表，每个包含段序和文本。
    ///
    /// 参考: gbrain (MIT) chunkers/ — R-5-201 — 三种分块策略 Rust 翻译
    pub fn chunk_text(text: &str, strategy: &ChunkStrategy) -> Vec<ChunkInput> {
        match strategy {
            ChunkStrategy::Fixed {
                chunk_size,
                overlap,
            } => Self::chunk_fixed(text, *chunk_size, *overlap),
            ChunkStrategy::Paragraph => Self::chunk_paragraph(text),
            ChunkStrategy::Sentence => Self::chunk_sentence(text),
        }
    }

    /// 固定字符数分块，支持滑动重叠。
    fn chunk_fixed(text: &str, chunk_size: usize, overlap: usize) -> Vec<ChunkInput> {
        if text.is_empty() || chunk_size == 0 {
            return vec![];
        }
        let effective_overlap = overlap.min(chunk_size.saturating_sub(1));
        let step = chunk_size.saturating_sub(effective_overlap);
        let len = text.chars().count();
        let mut chunks = Vec::new();
        let mut pos = 0usize;
        let mut seq = 0usize;

        while pos < len {
            let end = (pos + chunk_size).min(len);
            // 使用 char 索引取子串
            let chunk_text: String = text.chars().skip(pos).take(end - pos).collect();
            if !chunk_text.trim().is_empty() {
                chunks.push(ChunkInput {
                    seq,
                    text: chunk_text,
                });
                seq += 1;
            }
            if end >= len {
                break;
            }
            pos += step;
        }

        chunks
    }

    /// 按段落（\n\n）分块。
    fn chunk_paragraph(text: &str) -> Vec<ChunkInput> {
        if text.is_empty() {
            return vec![];
        }
        text.split("\n\n")
            .filter(|s| !s.trim().is_empty())
            .enumerate()
            .map(|(i, s)| ChunkInput {
                seq: i,
                text: s.trim().to_string(),
            })
            .collect()
    }

    /// 按句子（. ! ?）分块。
    fn chunk_sentence(text: &str) -> Vec<ChunkInput> {
        if text.is_empty() {
            return vec![];
        }
        let mut chunks = Vec::new();
        let mut current = String::new();
        let mut seq = 0usize;

        for ch in text.chars() {
            current.push(ch);
            if ch == '.' || ch == '!' || ch == '?' {
                let trimmed = current.trim();
                if !trimmed.is_empty() && trimmed.len() > 1 {
                    chunks.push(ChunkInput {
                        seq,
                        text: trimmed.to_string(),
                    });
                    seq += 1;
                }
                current.clear();
            }
        }

        // 剩余文本
        let remaining = current.trim();
        if !remaining.is_empty() {
            chunks.push(ChunkInput {
                seq,
                text: remaining.to_string(),
            });
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Fixed ──

    #[test]
    fn test_fixed_empty() {
        let chunks = Chunker::chunk_text("", &ChunkStrategy::Fixed { chunk_size: 10, overlap: 2 });
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_fixed_no_overlap() {
        let text = "abcdefghijklmnopqrstuvwxyz"; // 26 chars
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Fixed { chunk_size: 10, overlap: 0 });
        assert_eq!(chunks.len(), 3, "26 chars / 10 = 3 chunks");
        assert_eq!(chunks[0].text, "abcdefghij");
        assert_eq!(chunks[1].text, "klmnopqrst");
        assert_eq!(chunks[2].text, "uvwxyz");
    }

    #[test]
    fn test_fixed_with_overlap() {
        let text = "a".repeat(30);
        let chunks = Chunker::chunk_text(&text, &ChunkStrategy::Fixed { chunk_size: 10, overlap: 3 });
        // step = 10 - 3 = 7
        // pos: 0, 7, 14, 21 → 4 chunks (28 is 2 chars from end, makes 5th but last chunk is partial)
        assert!(!chunks.is_empty(), "should produce chunks");
        assert_eq!(chunks.len(), 4, "30 chars with step=7 should give 4 full chunks");
    }

    #[test]
    fn test_fixed_shorter_than_chunk_size() {
        let text = "short";
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Fixed { chunk_size: 100, overlap: 0 });
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "short");
    }

    #[test]
    fn test_fixed_overlap_not_exceeding_chunk_size() {
        let text = "hello world";
        // overlap > chunk_size should be clamped
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Fixed { chunk_size: 5, overlap: 10 });
        assert!(!chunks.is_empty(), "should still produce chunks");
    }

    // ── Paragraph ──

    #[test]
    fn test_paragraph_empty() {
        let chunks = Chunker::chunk_text("", &ChunkStrategy::Paragraph);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_paragraph_basic() {
        let text = "第一段。\n\n第二段。\n\n第三段。";
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Paragraph);
        assert_eq!(chunks.len(), 3);
        assert!(chunks[0].text.contains("第一段"));
        assert!(chunks[1].text.contains("第二段"));
    }

    #[test]
    fn test_paragraph_single() {
        let text = "只有一段";
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Paragraph);
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_paragraph_trim_empty_paragraphs() {
        let text = "一段\n\n\n\n二段";
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Paragraph);
        assert_eq!(chunks.len(), 2, "empty paragraphs should be filtered");
    }

    // ── Sentence ──

    #[test]
    fn test_sentence_empty() {
        let chunks = Chunker::chunk_text("", &ChunkStrategy::Sentence);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_sentence_basic() {
        let text = "Hello world. How are you? I am fine!";
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Sentence);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].text, "Hello world.");
        assert_eq!(chunks[1].text, "How are you?");
        assert_eq!(chunks[2].text, "I am fine!");
    }

    #[test]
    fn test_sentence_no_punctuation() {
        let text = "这是一个没有标点的长文本";
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Sentence);
        // No sentence-ending punctuation → single chunk
        assert_eq!(chunks.len(), 1);
    }

    #[test]
    fn test_sentence_mixed_punctuation() {
        let text = "First sentence. Second! Third? Fourth.";
        let chunks = Chunker::chunk_text(text, &ChunkStrategy::Sentence);
        assert_eq!(chunks.len(), 4);
    }
}
