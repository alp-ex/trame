use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq)]
pub enum ChunkType {
    Heading,
    Paragraph,
    CodeBlock,
    List,
    HorizontalRule,
}

impl ChunkType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChunkType::Heading => "heading",
            ChunkType::Paragraph => "paragraph",
            ChunkType::CodeBlock => "code_block",
            ChunkType::List => "list",
            ChunkType::HorizontalRule => "hr",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParsedChunk {
    pub chunk_type: ChunkType,
    pub heading_level: Option<u8>,
    pub content: String,
    pub start_offset: usize,
    pub end_offset: usize,
}

#[derive(Debug, Clone)]
pub struct ChunkWithHash {
    pub chunk: ParsedChunk,
    pub content_hash: String,
}

/// Compute SHA-256 hash of content (first 32 hex chars)
pub fn compute_hash(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.trim().as_bytes());
    let result = hasher.finalize();
    hex::encode(&result[..16]) // 16 bytes = 32 hex chars
}

/// Parse note content into logical chunks
pub fn parse_chunks(content: &str) -> Vec<ParsedChunk> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();

    while offset < len {
        // Skip leading whitespace
        while offset < len && (chars[offset] == ' ' || chars[offset] == '\t') {
            offset += 1;
        }

        // Skip newlines between chunks
        while offset < len && chars[offset] == '\n' {
            offset += 1;
        }

        if offset >= len {
            break;
        }

        // Check for fenced code block
        if offset + 2 < len && chars[offset] == '`' && chars[offset + 1] == '`' && chars[offset + 2] == '`' {
            let start = offset;
            offset += 3;
            // Skip language identifier line
            while offset < len && chars[offset] != '\n' {
                offset += 1;
            }
            if offset < len {
                offset += 1; // skip newline
            }
            // Find closing fence
            loop {
                if offset >= len {
                    break;
                }
                if offset + 2 < len && chars[offset] == '`' && chars[offset + 1] == '`' && chars[offset + 2] == '`' {
                    offset += 3;
                    // Skip rest of line
                    while offset < len && chars[offset] != '\n' {
                        offset += 1;
                    }
                    if offset < len {
                        offset += 1;
                    }
                    break;
                }
                offset += 1;
            }
            let content_str: String = chars[start..offset].iter().collect();
            chunks.push(ParsedChunk {
                chunk_type: ChunkType::CodeBlock,
                heading_level: None,
                content: content_str,
                start_offset: start,
                end_offset: offset,
            });
            continue;
        }

        // Check for heading
        if chars[offset] == '#' {
            let start = offset;
            let mut level = 0u8;
            while offset < len && chars[offset] == '#' && level < 6 {
                level += 1;
                offset += 1;
            }
            // Must have space after hashes for valid heading
            if offset < len && chars[offset] == ' ' {
                // Consume rest of line
                while offset < len && chars[offset] != '\n' {
                    offset += 1;
                }
                if offset < len {
                    offset += 1; // include newline
                }
                let content_str: String = chars[start..offset].iter().collect();
                chunks.push(ParsedChunk {
                    chunk_type: ChunkType::Heading,
                    heading_level: Some(level),
                    content: content_str.trim_end().to_string(),
                    start_offset: start,
                    end_offset: offset,
                });
                continue;
            } else {
                // Not a valid heading, reset and treat as paragraph
                offset = start;
            }
        }

        // Check for horizontal rule (---, ***, ___)
        if offset + 2 < len {
            let c = chars[offset];
            if (c == '-' || c == '*' || c == '_') && chars[offset + 1] == c && chars[offset + 2] == c {
                let start = offset;
                while offset < len && chars[offset] == c {
                    offset += 1;
                }
                // Skip to end of line
                while offset < len && chars[offset] != '\n' {
                    offset += 1;
                }
                if offset < len {
                    offset += 1;
                }
                let content_str: String = chars[start..offset].iter().collect();
                chunks.push(ParsedChunk {
                    chunk_type: ChunkType::HorizontalRule,
                    heading_level: None,
                    content: content_str.trim_end().to_string(),
                    start_offset: start,
                    end_offset: offset,
                });
                continue;
            }
        }

        // Check for list item
        if is_list_item(&chars, offset, len) {
            let start = offset;
            // Consume all consecutive list items
            while offset < len && is_list_item(&chars, offset, len) {
                // Consume line
                while offset < len && chars[offset] != '\n' {
                    offset += 1;
                }
                if offset < len {
                    offset += 1;
                }
                // Check for continuation (indented or next list item)
                // Skip empty lines within list
                while offset < len && chars[offset] == '\n' {
                    let peek = offset + 1;
                    if peek < len && is_list_item(&chars, peek, len) {
                        offset = peek;
                        break;
                    } else if peek < len && chars[peek] == '\n' {
                        // Double newline ends list
                        break;
                    } else {
                        break;
                    }
                }
            }
            let content_str: String = chars[start..offset].iter().collect();
            chunks.push(ParsedChunk {
                chunk_type: ChunkType::List,
                heading_level: None,
                content: content_str.trim_end().to_string(),
                start_offset: start,
                end_offset: offset,
            });
            continue;
        }

        // Default: paragraph (until double newline or special marker)
        let start = offset;
        loop {
            // Find end of line
            while offset < len && chars[offset] != '\n' {
                offset += 1;
            }
            if offset < len {
                offset += 1; // consume newline
            }

            // Check if next line starts a new block
            if offset >= len {
                break;
            }

            // Double newline ends paragraph
            if chars[offset] == '\n' {
                break;
            }

            // Check if next line is a special block
            if chars[offset] == '#'
                || (offset + 2 < len && chars[offset] == '`' && chars[offset + 1] == '`' && chars[offset + 2] == '`')
                || is_list_item(&chars, offset, len)
                || is_hr_start(&chars, offset, len)
            {
                break;
            }
        }

        if start < offset {
            let content_str: String = chars[start..offset].iter().collect();
            let trimmed = content_str.trim();
            if !trimmed.is_empty() {
                chunks.push(ParsedChunk {
                    chunk_type: ChunkType::Paragraph,
                    heading_level: None,
                    content: trimmed.to_string(),
                    start_offset: start,
                    end_offset: offset,
                });
            }
        }
    }

    chunks
}

fn is_list_item(chars: &[char], offset: usize, len: usize) -> bool {
    if offset >= len {
        return false;
    }

    // Unordered list: -, *, +
    if (chars[offset] == '-' || chars[offset] == '*' || chars[offset] == '+')
        && offset + 1 < len
        && chars[offset + 1] == ' '
    {
        return true;
    }

    // Ordered list: digit followed by . or )
    if chars[offset].is_ascii_digit() {
        let mut i = offset + 1;
        while i < len && chars[i].is_ascii_digit() {
            i += 1;
        }
        if i < len && (chars[i] == '.' || chars[i] == ')') && i + 1 < len && chars[i + 1] == ' ' {
            return true;
        }
    }

    false
}

fn is_hr_start(chars: &[char], offset: usize, len: usize) -> bool {
    if offset + 2 >= len {
        return false;
    }
    let c = chars[offset];
    (c == '-' || c == '*' || c == '_') && chars[offset + 1] == c && chars[offset + 2] == c
}

/// Parse and hash all chunks
pub fn chunk_and_hash(content: &str) -> Vec<ChunkWithHash> {
    parse_chunks(content)
        .into_iter()
        .map(|chunk| {
            let hash = compute_hash(&chunk.content);
            ChunkWithHash {
                chunk,
                content_hash: hash,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_content() {
        let chunks = parse_chunks("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_single_paragraph() {
        let chunks = parse_chunks("Hello world");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, ChunkType::Paragraph);
        assert_eq!(chunks[0].content, "Hello world");
    }

    #[test]
    fn test_heading() {
        let chunks = parse_chunks("# Title\n\nSome text");
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].chunk_type, ChunkType::Heading);
        assert_eq!(chunks[0].heading_level, Some(1));
        assert_eq!(chunks[0].content, "# Title");
        assert_eq!(chunks[1].chunk_type, ChunkType::Paragraph);
    }

    #[test]
    fn test_multiple_headings() {
        let chunks = parse_chunks("# H1\n## H2\n### H3");
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].heading_level, Some(1));
        assert_eq!(chunks[1].heading_level, Some(2));
        assert_eq!(chunks[2].heading_level, Some(3));
    }

    #[test]
    fn test_code_block() {
        let content = "```rust\nfn main() {}\n```";
        let chunks = parse_chunks(content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, ChunkType::CodeBlock);
    }

    #[test]
    fn test_list() {
        let content = "- item 1\n- item 2\n- item 3";
        let chunks = parse_chunks(content);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_type, ChunkType::List);
    }

    #[test]
    fn test_horizontal_rule() {
        let content = "text\n\n---\n\nmore text";
        let chunks = parse_chunks(content);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[1].chunk_type, ChunkType::HorizontalRule);
    }

    #[test]
    fn test_hash_consistency() {
        let hash1 = compute_hash("Hello world");
        let hash2 = compute_hash("Hello world");
        let hash3 = compute_hash("Hello world ");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1, hash3); // trimmed
    }

    #[test]
    fn test_hash_difference() {
        let hash1 = compute_hash("Hello");
        let hash2 = compute_hash("World");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_chunk_and_hash() {
        let chunks = chunk_and_hash("# Title\n\nParagraph");
        assert_eq!(chunks.len(), 2);
        assert!(!chunks[0].content_hash.is_empty());
        assert_eq!(chunks[0].content_hash.len(), 32);
    }

    #[test]
    fn test_complex_document() {
        let content = r#"# My Document

This is the intro paragraph.

## Section 1

Some content here.

- List item 1
- List item 2

```python
print("hello")
```

---

## Section 2

Final thoughts."#;

        let chunks = parse_chunks(content);
        assert!(chunks.len() >= 7);
        assert_eq!(chunks[0].chunk_type, ChunkType::Heading);
        assert_eq!(chunks[0].heading_level, Some(1));
    }
}
