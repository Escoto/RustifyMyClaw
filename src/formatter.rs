use crate::config::OutputConfig;
use crate::types::{FormattedResponse, ResponseChunk};

const FILE_NAME: &str = "response.txt";

/// Format raw CLI output into a `FormattedResponse` ready to send.
///
/// If total bytes exceed `file_upload_threshold_bytes`, the entire output is uploaded
/// as a file. Otherwise text is chunked using the configured strategy:
/// - `Natural`: respects code block and paragraph boundaries.
/// - `Fixed`: hard cut at `max_message_chars`, UTF-8 safe.
pub fn format(output: &str, config: &OutputConfig) -> FormattedResponse {
    use crate::config::ChunkStrategy;

    if output.len() > config.file_upload_threshold_bytes {
        return FormattedResponse {
            chunks: vec![ResponseChunk::File {
                name: FILE_NAME.to_string(),
                content: output.as_bytes().to_vec(),
            }],
        };
    }

    let chunks = match config.chunk_strategy {
        ChunkStrategy::Fixed => fixed_chunks(output, config.max_message_chars),
        ChunkStrategy::Natural => natural_chunks(output, config.max_message_chars),
    };
    FormattedResponse { chunks }
}

/// Split `text` into chunks by hard-cutting at `max_chars` byte positions.
///
/// Always rounds to a valid UTF-8 char boundary using `char_boundary_floor()`.
/// Will split mid-word and mid-code-block — use the Natural strategy to avoid that.
fn fixed_chunks(text: &str, max_chars: usize) -> Vec<ResponseChunk> {
    if text.is_empty() {
        return vec![ResponseChunk::Text(String::new())];
    }

    let mut result = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_chars {
            result.push(ResponseChunk::Text(remaining.to_string()));
            break;
        }
        let cut = char_boundary_floor(remaining, max_chars);
        // Guard against max_chars=0 or a single char > max_chars to avoid infinite loop.
        let cut = if cut == 0 {
            remaining
                .char_indices()
                .nth(1)
                .map(|(i, _)| i)
                .unwrap_or(remaining.len())
        } else {
            cut
        };
        let (chunk, rest) = remaining.split_at(cut);
        result.push(ResponseChunk::Text(chunk.to_string()));
        remaining = rest;
    }

    result
}

/// Split `text` into chunks ≤ `max_chars` bytes using natural boundary detection.
///
/// Code blocks (``` fences) are treated as atomic units — they are never split.
/// An oversized single code block is promoted to a File chunk.
/// Prose between code blocks is split on paragraph → line → char-boundary hard cut.
fn natural_chunks(text: &str, max_chars: usize) -> Vec<ResponseChunk> {
    if text.is_empty() {
        return vec![ResponseChunk::Text(String::new())];
    }

    let mut result: Vec<ResponseChunk> = Vec::new();

    for segment in split_into_segments(text) {
        match segment {
            Segment::Code(block) => {
                if block.len() > max_chars {
                    result.push(ResponseChunk::File {
                        name: FILE_NAME.to_string(),
                        content: block.into_bytes(),
                    });
                } else {
                    result.push(ResponseChunk::Text(block));
                }
            }
            Segment::Prose(prose) => {
                for chunk in split_prose(&prose, max_chars) {
                    if !chunk.is_empty() {
                        result.push(ResponseChunk::Text(chunk));
                    }
                }
            }
        }
    }

    if result.is_empty() {
        result.push(ResponseChunk::Text(String::new()));
    }
    result
}

/// Split prose text (no code blocks) into chunks ≤ `max_chars` bytes.
///
/// Split priority: paragraph break (`\n\n`) → line break (`\n`) → sentence end (`. `) →
/// hard cut at a valid UTF-8 char boundary. Never panics on multibyte input.
fn split_prose(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let mut result = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if remaining.len() <= max_chars {
            result.push(remaining.to_string());
            break;
        }

        let limit = char_boundary_floor(remaining, max_chars);
        let candidate = &remaining[..limit];

        let (chunk, rest) = if let Some(pos) = candidate.rfind("\n\n") {
            remaining.split_at(pos + 2)
        } else if let Some(pos) = candidate.rfind('\n') {
            remaining.split_at(pos + 1)
        } else if let Some(pos) = candidate.rfind(". ") {
            remaining.split_at(pos + 2)
        } else if limit == 0 {
            // First char alone exceeds max_chars — emit it to avoid infinite loop.
            let char_end = remaining
                .char_indices()
                .nth(1)
                .map(|(i, _)| i)
                .unwrap_or(remaining.len());
            remaining.split_at(char_end)
        } else {
            remaining.split_at(limit)
        };

        result.push(chunk.to_string());
        remaining = rest;
    }

    result
}

/// Return the largest byte index ≤ `pos` that is a valid UTF-8 char boundary in `s`.
fn char_boundary_floor(s: &str, pos: usize) -> usize {
    let pos = pos.min(s.len());
    let mut b = pos;
    while b > 0 && !s.is_char_boundary(b) {
        b -= 1;
    }
    b
}

#[derive(Debug)]
enum Segment {
    Code(String),
    Prose(String),
}

/// Split text into alternating prose and fenced code-block segments.
fn split_into_segments(text: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if let Some(start) = remaining.find("```") {
            if start > 0 {
                segments.push(Segment::Prose(remaining[..start].to_string()));
            }
            remaining = &remaining[start..];

            // Find the closing fence (search after the opening ```)
            if let Some(end_rel) = remaining[3..].find("```") {
                let end = end_rel + 3 + 3; // past both ``` markers
                segments.push(Segment::Code(remaining[..end].to_string()));
                remaining = &remaining[end..];
            } else {
                // Unclosed fence — treat remainder as prose
                segments.push(Segment::Prose(remaining.to_string()));
                break;
            }
        } else {
            segments.push(Segment::Prose(remaining.to_string()));
            break;
        }
    }
    segments
}

/// Build an error prefix string for a nonzero CLI exit code.
pub fn format_error(exit_code: i32, stderr: &str) -> String {
    if stderr.is_empty() {
        format!("[exit {exit_code}]")
    } else {
        format!("[exit {exit_code}]\n{stderr}")
    }
}

#[cfg(test)]
#[path = "tests/formatter_test.rs"]
mod tests;
