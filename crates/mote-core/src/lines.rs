//! Byte-offset → line/column conversion.

use crate::model::TextRange;

/// Maps byte offsets to 1-based lines and 1-based columns (Unicode scalar values).
pub struct LineIndex<'a> {
    text: &'a str,
    line_starts: Vec<u32>,
    /// Per line: true if the line contains a non-ASCII byte (columns then need char counting).
    non_ascii: Vec<bool>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        // Built for every file (often twice: by the plugin and by the engine), so the text is
        // read eight bytes at a time: one test finds the chunks without a newline or a non-ASCII
        // byte, which is nearly all of them.
        const LO7: u64 = 0x7F7F_7F7F_7F7F_7F7F;
        const HIGH: u64 = 0x8080_8080_8080_8080;
        const NEWLINES: u64 = 0x0A0A_0A0A_0A0A_0A0A;
        let bytes = text.as_bytes();
        let mut line_starts = Vec::with_capacity(text.len() / 32 + 1);
        let mut non_ascii = Vec::with_capacity(text.len() / 32 + 1);
        line_starts.push(0);
        let mut current_non_ascii = false;
        let mut chunks = bytes.chunks_exact(8);
        let mut base = 0usize;
        for chunk in &mut chunks {
            let mut word = [0u8; 8];
            word.copy_from_slice(chunk);
            let w = u64::from_le_bytes(word);
            // The high bit of each byte equal to `\n` (exact: no carry crosses a byte).
            let t = w ^ NEWLINES;
            let mut newlines = !(((t & LO7) + LO7) | t | LO7);
            // The high bit of each non-ASCII byte.
            let mut high = w & HIGH;
            while newlines != 0 {
                let bit = newlines.trailing_zeros();
                line_starts.push((base + (bit / 8) as usize) as u32 + 1);
                // Non-ASCII bytes before this newline belong to the line it ends.
                let before = (1u64 << bit) - 1;
                non_ascii.push(current_non_ascii || high & before != 0);
                current_non_ascii = false;
                high &= !before;
                newlines &= newlines - 1;
            }
            current_non_ascii |= high != 0;
            base += 8;
        }
        for (i, &b) in chunks.remainder().iter().enumerate() {
            if b == b'\n' {
                line_starts.push((base + i) as u32 + 1);
                non_ascii.push(current_non_ascii);
                current_non_ascii = false;
            } else if b >= 0x80 {
                current_non_ascii = true;
            }
        }
        non_ascii.push(current_non_ascii);
        LineIndex {
            text,
            line_starts,
            non_ascii,
        }
    }

    /// Byte offset of the start of every line, ascending: `0`, then one past each `\n` (so a
    /// text ending with a newline has an empty last entry at its end, which
    /// [`line_count`](Self::line_count) does not count).
    pub fn line_starts(&self) -> &[u32] {
        &self.line_starts
    }

    /// Number of lines (a trailing newline does not start a new counted line).
    pub fn line_count(&self) -> u32 {
        let n = self.line_starts.len() as u32;
        if self.text.ends_with('\n') {
            n - 1
        } else {
            n
        }
    }

    /// 1-based line containing `offset`.
    pub fn line_of(&self, offset: u32) -> u32 {
        match self.line_starts.binary_search(&offset) {
            Ok(i) => i as u32 + 1,
            Err(i) => i as u32,
        }
    }

    /// 1-based (line, column) of `offset`.
    pub fn line_col(&self, offset: u32) -> (u32, u32) {
        let offset = offset.min(self.text.len() as u32);
        let line = self.line_of(offset);
        let start = self.line_starts[(line - 1) as usize] as usize;
        if !self.non_ascii[(line - 1) as usize] {
            // ASCII line: byte offset == char offset, O(1).
            return (line, (offset as usize - start) as u32 + 1);
        }
        let mut end = offset as usize;
        while end > start && !self.text.is_char_boundary(end) {
            end -= 1;
        }
        let col = self.text[start..end].chars().count() as u32 + 1;
        (line, col)
    }

    /// Range for a `[start, end)` byte span.
    pub fn range(&self, start: u32, end: u32) -> TextRange {
        let (start_line, start_col) = self.line_col(start);
        let (end_line, end_col) = self.line_col(end.max(start));
        TextRange {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    /// Text of a 1-based line without its line terminator.
    pub fn line_text(&self, line: u32) -> &'a str {
        if line == 0 || line as usize > self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[(line - 1) as usize] as usize;
        let end = self
            .line_starts
            .get(line as usize)
            .map(|&e| e as usize)
            .unwrap_or(self.text.len());
        self.text[start..end].trim_end_matches(['\n', '\r'])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte-at-a-time definition of the index.
    fn reference(text: &str) -> (Vec<u32>, Vec<bool>) {
        let (mut starts, mut non_ascii, mut current) = (vec![0u32], Vec::new(), false);
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i as u32 + 1);
                non_ascii.push(current);
                current = false;
            } else if b >= 0x80 {
                current = true;
            }
        }
        non_ascii.push(current);
        (starts, non_ascii)
    }

    #[test]
    fn index_matches_the_reference_at_every_alignment() {
        let pieces = ["a", "\n", "é", "\r\n", "€", "bcdefgh", "\n\n", "😀", " "];
        let mut texts = vec![String::new()];
        // Every short sequence of pieces, so newlines and non-ASCII bytes land at every position
        // of an 8-byte chunk and on both sides of chunk boundaries.
        for len in 1..=4 {
            let mut idx = vec![0usize; len];
            loop {
                texts.push(idx.iter().map(|&i| pieces[i]).collect());
                let mut k = 0;
                while k < len {
                    idx[k] += 1;
                    if idx[k] < pieces.len() {
                        break;
                    }
                    idx[k] = 0;
                    k += 1;
                }
                if k == len {
                    break;
                }
            }
        }
        for prefix in ["", "x", "xxxxxxx", "\u{e9}xxxxx"] {
            for t in texts.clone() {
                texts.push(format!("{prefix}{t}{t}"));
            }
        }
        for t in &texts {
            let idx = LineIndex::new(t);
            assert_eq!(
                (idx.line_starts.clone(), idx.non_ascii.clone()),
                reference(t),
                "{t:?}"
            );
        }
    }

    #[test]
    fn line_col_basics() {
        let idx = LineIndex::new("ab\ncé\r\nx");
        assert_eq!(idx.line_count(), 3);
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_col(3), (2, 1));
        assert_eq!(idx.line_col(6), (2, 3)); // after 'é' (2 bytes)
        assert_eq!(idx.line_text(2), "cé");
        assert_eq!(idx.line_text(3), "x");
    }
}
