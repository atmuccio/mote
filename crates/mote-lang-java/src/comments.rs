//! Comment collection (called by the walker for `line_comment` / `block_comment` nodes).

use crate::ctx::Ctx;
use mote_core::Comment;
use tree_sitter::Node;

pub(crate) fn collect(ctx: &mut Ctx<'_>, node: Node<'_>) {
    let src = ctx.src;
    let start = node.start_byte().min(src.len());
    let text = ctx.text(node).trim_end_matches(['\r', '\n']);
    let (start_line, start_col) = ctx.lines.line_col(src, start as u32);
    let end_line = if text.is_empty() {
        start_line
    } else {
        ctx.lines.line_of((start + text.len() - 1) as u32)
    };
    // Trailing when something other than blanks precedes the comment on its line. Scan backwards
    // over blanks only, so a huge single line stays cheap.
    let bytes = src.as_bytes();
    let mut i = start;
    while i > 0 && matches!(bytes.get(i - 1), Some(b' ' | b'\t' | 0x0c)) {
        i -= 1;
    }
    let trailing = i > 0 && !matches!(bytes.get(i - 1), Some(b'\n' | b'\r'));
    ctx.comments.push(Comment {
        start_line,
        end_line,
        start_col,
        text: text.to_string(),
        trailing,
    });
}
