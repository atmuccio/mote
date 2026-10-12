//! Minimal XML writing helpers shared by the Checkstyle and JUnit writers.
//!
//! Finding messages and paths quote scanned code, so everything that goes into the document is
//! escaped here: `&`, `<`, `>`, `"` and `'` become entities; tab, line feed and carriage return
//! become character references (an XML parser would otherwise turn them into spaces inside an
//! attribute); characters XML 1.0 cannot carry at all, even as references (other C0 controls,
//! U+FFFE, U+FFFF), become U+FFFD. Escaping `>` also neutralizes `]]>`.

/// The XML declaration every document starts with.
pub const DECLARATION: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";

/// Escape `s` for an attribute value or text content.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    push_escaped(&mut out, s);
    out
}

/// Append `s` escaped (see [`escape`]).
pub fn push_escaped(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            '\t' => out.push_str("&#9;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            c if is_forbidden(c) => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
}

/// Append `s` escaped for element text: like [`push_escaped`], but line feeds and tabs stay as
/// they are (text content keeps them), which keeps multi-line text readable in the file.
pub fn push_text(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '\n' | '\t' => out.push(c),
            c => {
                let mut buf = [0u8; 4];
                push_escaped(out, c.encode_utf8(&mut buf));
            }
        }
    }
}

/// Characters XML 1.0 does not allow in a document, not even as character references.
fn is_forbidden(c: char) -> bool {
    matches!(
        c,
        '\u{0}'..='\u{8}' | '\u{B}' | '\u{C}' | '\u{E}'..='\u{1F}' | '\u{FFFE}' | '\u{FFFF}'
    )
}

/// Append ` name="value"` with the value escaped.
pub fn attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    push_escaped(out, value);
    out.push('"');
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Parse `xml` with quick-xml and return every attribute value and non-blank text node,
    /// unescaped: the round trip proves the document is well-formed and the escaping lossless.
    pub(crate) fn parse_all(xml: &str) -> Vec<String> {
        use quick_xml::events::Event;
        let mut reader = quick_xml::Reader::from_str(xml);
        let mut out = Vec::new();
        let mut text = String::new();
        let flush = |text: &mut String, out: &mut Vec<String>| {
            if !text.trim().is_empty() {
                out.push(std::mem::take(text));
            }
            text.clear();
        };
        loop {
            match reader.read_event() {
                Ok(Event::Eof) => break,
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    flush(&mut text, &mut out);
                    for a in e.attributes() {
                        let a = a.expect("well-formed attribute");
                        let v = a
                            .normalized_value(quick_xml::XmlVersion::default())
                            .expect("attribute value unescapes");
                        out.push(v.into_owned());
                    }
                }
                Ok(Event::End(_)) => flush(&mut text, &mut out),
                Ok(Event::Text(t)) => text.push_str(&t.xml10_content()),
                Ok(Event::GeneralRef(r)) => {
                    let raw = format!("&{};", r.xml10_content());
                    text.push_str(&quick_xml::escape::unescape(&raw).expect("known entity"));
                }
                Ok(_) => {}
                Err(e) => panic!("malformed XML at {}: {e}\n{xml}", reader.buffer_position()),
            }
        }
        out
    }

    #[test]
    fn escapes_markup_whitespace_and_forbidden_characters() {
        assert_eq!(
            escape("a<b>&\"c\"'d' ]]> x"),
            "a&lt;b&gt;&amp;&quot;c&quot;&apos;d&apos; ]]&gt; x"
        );
        assert_eq!(escape("l1\nl2\r\tx"), "l1&#10;l2&#13;&#9;x");
        assert_eq!(
            escape("nul\u{0}bell\u{7}esc\u{1b}"),
            "nul\u{FFFD}bell\u{FFFD}esc\u{FFFD}"
        );
        assert_eq!(escape("\u{FFFE}\u{FFFF}ü€😀"), "\u{FFFD}\u{FFFD}ü€😀");
        let mut t = String::new();
        push_text(&mut t, "a\n\tb\r<c>\u{1}");
        assert_eq!(t, "a\n\tb&#13;&lt;c&gt;\u{FFFD}");
        let mut s = String::from("<e");
        attr(&mut s, "m", "</script><img src=x onerror=alert(1)>\nnext");
        s.push_str(">x &amp; ]]&gt;</e>");
        assert_eq!(
            parse_all(&s),
            ["</script><img src=x onerror=alert(1)>\nnext", "x & ]]>"]
        );
    }
}
