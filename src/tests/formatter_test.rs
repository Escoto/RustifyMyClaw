use super::*;
use crate::config::{ChunkStrategy, OutputConfig};
use crate::types::ResponseChunk;

fn config(max_chars: usize) -> OutputConfig {
    OutputConfig {
        max_message_chars: max_chars,
        file_upload_threshold_bytes: 51200,
        chunk_strategy: ChunkStrategy::Natural,
    }
}

fn config_with_threshold(max_chars: usize, threshold: usize) -> OutputConfig {
    OutputConfig {
        max_message_chars: max_chars,
        file_upload_threshold_bytes: threshold,
        chunk_strategy: ChunkStrategy::Natural,
    }
}

fn fixed_config(max_chars: usize) -> OutputConfig {
    OutputConfig {
        max_message_chars: max_chars,
        file_upload_threshold_bytes: 51200,
        chunk_strategy: ChunkStrategy::Fixed,
    }
}

fn text_chunks(resp: FormattedResponse) -> Vec<String> {
    resp.chunks
        .into_iter()
        .filter_map(|c| match c {
            ResponseChunk::Text(t) => Some(t),
            _ => None,
        })
        .collect()
}

fn is_file(chunk: &ResponseChunk) -> bool {
    matches!(chunk, ResponseChunk::File { .. })
}

#[test]
fn under_limit_single_chunk() {
    let resp = format("hello world", &config(4000));
    let chunks = text_chunks(resp);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "hello world");
}

#[test]
fn over_limit_multiple_chunks() {
    let long = "a".repeat(10);
    let resp = format(&long, &config(3));
    let chunks = text_chunks(resp);
    assert!(chunks.len() > 1);
    for c in &chunks {
        assert!(c.len() <= 3, "chunk too large: {}", c.len());
    }
}

#[test]
fn over_file_threshold_sends_file() {
    let big = "x".repeat(100);
    let resp = format(&big, &config_with_threshold(4000, 50));
    assert_eq!(resp.chunks.len(), 1);
    assert!(is_file(&resp.chunks[0]));
}

#[test]
fn code_block_not_split() {
    let text = "before\n```rust\nfn main() {}\n```\nafter";
    let resp = format(text, &config(4000));
    let joined: String = resp
        .chunks
        .iter()
        .filter_map(|c| match c {
            ResponseChunk::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("");
    assert!(joined.contains("```rust\nfn main() {}\n```"));
}

#[test]
fn oversized_code_block_becomes_file() {
    let code = format!("```\n{}\n```", "x".repeat(200));
    let resp = format(&code, &config(100));
    assert!(resp.chunks.iter().any(is_file));
}

#[test]
fn chunks_respect_paragraph_breaks() {
    let text = format!("{}\n\n{}", "a".repeat(30), "b".repeat(30));
    let resp = format(&text, &config(40));
    let chunks = text_chunks(resp);
    assert!(
        chunks.len() >= 2,
        "expected multiple chunks, got {}",
        chunks.len()
    );
}

#[test]
fn empty_input_produces_one_empty_chunk() {
    let resp = format("", &config(4000));
    let chunks = text_chunks(resp);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "");
}

#[test]
fn unicode_does_not_panic() {
    let text = "こんにちは世界 🌍".repeat(5);
    let _ = format(&text, &config(20));
}

#[test]
fn all_chunks_within_max_chars() {
    let text = "word ".repeat(1000);
    let resp = format(&text, &config(100));
    for chunk in resp.chunks {
        if let ResponseChunk::Text(t) = chunk {
            assert!(t.len() <= 100, "chunk too large: {}", t.len());
        }
    }
}

#[test]
fn unicode_chunks_within_max_chars() {
    let text = "こ".repeat(20);
    let resp = format(&text, &config(10));
    for chunk in resp.chunks {
        if let ResponseChunk::Text(t) = chunk {
            assert!(t.len() <= 10, "chunk too large: {}", t.len());
            assert!(std::str::from_utf8(t.as_bytes()).is_ok());
        }
    }
}

// ── Sentence boundary tests ─────────────────────────────────────────────────

#[test]
fn sentence_boundary_preferred_over_hard_cut() {
    // Two sentences totalling > 30 chars; limit 30 — should split at ". " not mid-word.
    let text = "Hello world. This is a test sentence.";
    let resp = format(text, &config(30));
    let chunks = text_chunks(resp);
    assert!(chunks.len() >= 2);
    // First chunk must end right after the period+space split point.
    assert!(
        chunks[0].ends_with("world.") || chunks[0].ends_with("world. "),
        "expected split at sentence boundary, got: {:?}",
        chunks[0]
    );
}

#[test]
fn multiple_sentences_split_into_multiple_chunks() {
    let text = "First sentence. Second sentence. Third sentence.";
    let resp = format(text, &config(20));
    let chunks = text_chunks(resp);
    assert!(chunks.len() >= 2);
    // Reassembly should restore the original.
    let joined = chunks.join("");
    assert_eq!(joined, text);
}

#[test]
fn no_sentence_boundary_falls_through_to_hard_cut() {
    // No `. ` within the limit — must hard-cut at char boundary.
    let text = "abcdefghijklmnopqrstuvwxyz";
    let resp = format(text, &config(10));
    let chunks = text_chunks(resp);
    assert!(chunks.len() > 1);
    for c in &chunks {
        assert!(c.len() <= 10);
    }
    assert_eq!(chunks.join(""), text);
}

// ── Fixed strategy tests ────────────────────────────────────────────────────

#[test]
fn fixed_under_limit_single_chunk() {
    let resp = format("hello", &fixed_config(100));
    let chunks = text_chunks(resp);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "hello");
}

#[test]
fn fixed_empty_input_produces_one_empty_chunk() {
    let resp = format("", &fixed_config(100));
    let chunks = text_chunks(resp);
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "");
}

#[test]
fn fixed_hard_cut_at_boundary() {
    // 10 ascii chars, limit 3 → should produce ceil(10/3) = 4 chunks
    let resp = format("abcdefghij", &fixed_config(3));
    let chunks = text_chunks(resp);
    assert_eq!(chunks.len(), 4);
    assert_eq!(chunks[0], "abc");
    assert_eq!(chunks[1], "def");
    assert_eq!(chunks[2], "ghi");
    assert_eq!(chunks[3], "j");
}

#[test]
fn fixed_mid_word_split() {
    // Fixed strategy splits without regard for word boundaries.
    let resp = format("hello world", &fixed_config(4));
    let chunks = text_chunks(resp);
    assert!(chunks.len() > 1);
    // Verify reassembly restores the original string.
    let joined: String = chunks.concat();
    assert_eq!(joined, "hello world");
}

#[test]
fn fixed_all_chunks_within_max_chars() {
    let text = "word ".repeat(200);
    let resp = format(&text, &fixed_config(17));
    for chunk in resp.chunks {
        if let ResponseChunk::Text(t) = chunk {
            assert!(t.len() <= 17, "chunk too large: {}", t.len());
        }
    }
}

#[test]
fn fixed_unicode_at_boundary_valid_utf8() {
    // "こ" is 3 bytes. With max_chars=5 the cut must land on a char boundary.
    let text = "こんにちは世界";
    let resp = format(text, &fixed_config(5));
    for chunk in resp.chunks {
        if let ResponseChunk::Text(t) = chunk {
            assert!(t.len() <= 5, "chunk too large: {}", t.len());
            assert!(std::str::from_utf8(t.as_bytes()).is_ok(), "invalid UTF-8");
        }
    }
}

#[test]
fn fixed_reassembly_matches_original() {
    let text = "こんにちは world 🌍 test";
    let resp = format(text, &fixed_config(7));
    let joined: String = text_chunks(resp).concat();
    assert_eq!(joined, text);
}

#[test]
fn fixed_over_file_threshold_still_uploads_file() {
    let big = "x".repeat(100);
    let resp = format(
        &big,
        &OutputConfig {
            max_message_chars: 4000,
            file_upload_threshold_bytes: 50,
            chunk_strategy: ChunkStrategy::Fixed,
        },
    );
    assert_eq!(resp.chunks.len(), 1);
    assert!(is_file(&resp.chunks[0]));
}
