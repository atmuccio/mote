//! YAML documents: `.mote.yaml`, the triage file and ESLint's YAML configurations.
//!
//! The text is parsed by [granit-parser](https://crates.io/crates/granit-parser), a YAML 1.2
//! event parser in safe Rust, into a tree of [`Node`]s that keeps each node's position, and read
//! through serde by this module's deserializer. The deserializer keeps the reading Mote's files
//! have always had (until 0.2 the parser was `serde_yaml_ng`, built on libyaml):
//!
//! * **Scalars follow the YAML 1.2 core schema** where the target type leaves a choice: `true`,
//!   `false` (also `True`, `TRUE`, …) are booleans, `null`, `~` and an empty value are null,
//!   `12`, `0x1F`, `0o17`, `0b101` integers, `1.5`, `1e3`, `.inf`, `.nan` floats. YAML 1.1
//!   forms are strings: `on`, `off`, `yes`, `no`, `y`, `n`, `012`, `1_000`, dates. A plain
//!   scalar is read as written where a string is expected (`mote: 0.1` is `"0.1"`); a quoted
//!   one is always a string (`release: "17"` is not a number).
//! * **Anchors and aliases** are expanded; merge keys (`<<`) are ordinary keys; a stream with
//!   more than one document is refused.
//! * **Errors** read like serde's (`invalid type: string "x", expected u32`), start with the
//!   path of the value in error (`gate.fail_on: …`) and end with its position (`at line 3
//!   column 12`), which [`Error::location`] also returns.
//!
//! A file can come from an untrusted pull request, so reading is bounded: the text's size
//! ([`MAX_BYTES`]), the number of nodes ([`MAX_NODES`]), the nodes added by expanding aliases
//! ([`MAX_ALIAS_NODES`], against "billion laughs" documents) and the nesting depth
//! ([`MAX_DEPTH`]). Past a bound the document is refused with an error, before any large
//! allocation.

mod de;
mod load;

use serde::de::DeserializeOwned;
use std::fmt;

pub(crate) use de::{canonical, check_generic, from_node, key_text, meaning, Meaning};
#[cfg(test)]
pub(crate) use load::load;
pub(crate) use load::load_partial;

/// The largest YAML text read, in bytes (16 MiB).
pub const MAX_BYTES: usize = 16 << 20;
/// The most nodes (scalars, sequences, mappings, keys included) a document may have, aliases
/// expanded.
pub const MAX_NODES: usize = 1_000_000;
/// The most nodes that expanding aliases may add to a document.
pub const MAX_ALIAS_NODES: usize = 100_000;
/// The deepest nesting of sequences and mappings, aliases expanded.
pub const MAX_DEPTH: usize = 256;

/// Deserialize a `T` from YAML text.
///
/// Errors come in reading order, as a streaming parser finds them: a value of the wrong type
/// before the place where the text stops being YAML is the error reported.
pub fn from_str<T: DeserializeOwned>(text: &str) -> Result<T, Error> {
    let (document, error) = load_partial(text);
    let value = from_node(&document)?;
    match error {
        Some(e) => Err(e),
        None => Ok(value),
    }
}

/// A position in the text: 1-based line and column (in characters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Mark {
    pub line: usize,
    pub column: usize,
}

/// A node of a document, at the position where it starts (its anchor or tag, if any).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Node {
    pub value: Value,
    /// The resolved tag (`tag:yaml.org,2002:str` for `!!str`, `!x` for a local tag `!x`).
    pub tag: Option<String>,
    pub mark: Mark,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Value {
    /// The content of a stream without a document (only comments, say).
    Void,
    /// Where the text stops being YAML (or breaks a limit): reading this node gives the error.
    /// It ends the sequences and mappings that were open there.
    Missing(Box<Error>),
    Scalar(Scalar),
    Seq(Vec<Node>),
    /// Entries in document order; keys are not deduplicated.
    Map(Vec<(Node, Node)>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Scalar {
    pub text: String,
    pub style: Style,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Style {
    Plain,
    SingleQuoted,
    DoubleQuoted,
    Literal,
    Folded,
}

impl Node {
    /// A node without a position in any text.
    pub(crate) fn new(value: Value) -> Node {
        Node {
            value,
            tag: None,
            mark: Mark::default(),
        }
    }

    /// A plain string scalar (a stand-in value).
    pub(crate) fn string(text: &str) -> Node {
        Node::new(Value::Scalar(Scalar {
            text: text.to_string(),
            style: Style::DoubleQuoted,
        }))
    }
}

/// A YAML error: the text is not YAML, or it does not deserialize as the type asked for.
#[derive(Clone)]
pub struct Error(Box<ErrorKind>);

#[derive(Clone, Debug)]
enum ErrorKind {
    /// A deserialization error, with the position and path of the value in error once known.
    Message(String, Option<Pos>),
    /// The parser's error: what is wrong, where, and what was being read (from where).
    Syntax(String, Mark, Option<(String, Option<Mark>)>),
    /// A character YAML does not allow, at a byte offset.
    ControlCharacter(usize),
    TooLarge(usize),
    TooManyNodes,
    EndOfStream,
    MoreThanOneDocument,
    RecursionLimitExceeded(Mark),
    RepetitionLimitExceeded(Mark),
    BytesUnsupported,
}

#[derive(Clone, Debug)]
struct Pos {
    mark: Mark,
    path: String,
}

impl Error {
    fn new(kind: ErrorKind) -> Error {
        Error(Box::new(kind))
    }

    /// 1-based line and column of the error, when known.
    pub fn location(&self) -> Option<(usize, usize)> {
        self.mark().map(|m| (m.line, m.column))
    }

    fn mark(&self) -> Option<Mark> {
        match &*self.0 {
            ErrorKind::Message(_, Some(pos)) => Some(pos.mark),
            ErrorKind::Syntax(_, mark, _)
            | ErrorKind::RecursionLimitExceeded(mark)
            | ErrorKind::RepetitionLimitExceeded(mark) => Some(*mark),
            // As libyaml reported it: an offset, at the start of the text.
            ErrorKind::ControlCharacter(_) => Some(Mark { line: 1, column: 1 }),
            _ => None,
        }
        // A node built in code has no position.
        .filter(|m| m.line > 0)
    }

    /// Attach the position and path of the value whose deserialization failed, unless the
    /// error has them already (it was raised deeper).
    fn fix_mark(mut self, mark: Mark, path: &dyn fmt::Display) -> Error {
        if let ErrorKind::Message(_, pos @ None) = &mut *self.0 {
            *pos = Some(Pos {
                mark,
                path: path.to_string(),
            });
        }
        self
    }

    fn message(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &*self.0 {
            ErrorKind::Message(msg, None) => f.write_str(msg),
            ErrorKind::Message(msg, Some(Pos { path, .. })) => {
                if path != "." {
                    write!(f, "{path}: ")?;
                }
                f.write_str(msg)
            }
            ErrorKind::Syntax(msg, ..) => f.write_str(msg),
            ErrorKind::ControlCharacter(_) => f.write_str("control characters are not allowed"),
            ErrorKind::TooLarge(len) => write!(
                f,
                "the text is too large to read as YAML ({len} bytes; the limit is {MAX_BYTES})"
            ),
            ErrorKind::TooManyNodes => write!(
                f,
                "the document is too large to read as YAML (more than {MAX_NODES} values)"
            ),
            ErrorKind::EndOfStream => f.write_str("EOF while parsing a value"),
            ErrorKind::MoreThanOneDocument => f.write_str(
                "deserializing from YAML containing more than one document is not supported",
            ),
            ErrorKind::RecursionLimitExceeded(_) => write!(
                f,
                "recursion limit exceeded (sequences and mappings nested too deep)"
            ),
            ErrorKind::RepetitionLimitExceeded(_) => write!(
                f,
                "repetition limit exceeded (aliases expand to more than {MAX_ALIAS_NODES} values)"
            ),
            ErrorKind::BytesUnsupported => {
                f.write_str("serialization and deserialization of bytes in YAML is not implemented")
            }
        }
    }
}

impl fmt::Display for Error {
    /// The message, then ` at line L column C` (omitted at the very start of the text, as
    /// libyaml did), or ` at position N` for a character YAML does not allow; for a syntax error,
    /// then what was being read and from where (`, while parsing a flow sequence at line 1
    /// column 8`).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn at(f: &mut fmt::Formatter<'_>, m: Mark) -> fmt::Result {
            if m.line > 1 || m.column > 1 {
                write!(f, " at line {} column {}", m.line, m.column)?;
            }
            Ok(())
        }
        self.message(f)?;
        match &*self.0 {
            ErrorKind::ControlCharacter(offset) => {
                if *offset != 0 {
                    write!(f, " at position {offset}")?;
                }
            }
            ErrorKind::Syntax(_, mark, context) => {
                at(f, *mark)?;
                if let Some((context, context_mark)) = context {
                    write!(f, ", {context}")?;
                    if let Some(m) = context_mark.filter(|m| m != mark) {
                        at(f, m)?;
                    }
                }
            }
            _ => {
                if let Some(m) = self.mark() {
                    at(f, m)?;
                }
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Error({:?}", self.to_string())?;
        if let Some((l, c)) = self.location() {
            write!(f, ", line: {l}, column: {c}")?;
        }
        f.write_str(")")
    }
}

impl std::error::Error for Error {}

impl PartialEq for Error {
    fn eq(&self, other: &Error) -> bool {
        self.to_string() == other.to_string() && self.location() == other.location()
    }
}

impl serde::de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error::new(ErrorKind::Message(msg.to_string(), None))
    }
}

#[cfg(test)]
mod tests;
