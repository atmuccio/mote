//! serde over a [`Node`] tree.
//!
//! Scalars are read by the target type where it asks for one (`deserialize_str` takes any
//! scalar's text, `deserialize_u64` a plain integer) and by the YAML 1.2 core schema where it
//! does not (`deserialize_any`). An error raised while a node is read gets that node's position
//! and path (`rules.java/x.params`); errors raised deeper keep theirs. A local tag (`!name
//! value`) selects an enum variant. Nesting deeper than 128 sequences and mappings is refused,
//! as before.

use super::{Error, ErrorKind, Mark, Node, Scalar, Style, Value};
use serde::de::{
    self, value::StrDeserializer, Deserialize, DeserializeOwned, DeserializeSeed, Expected,
    IntoDeserializer, Unexpected, Visitor,
};
use std::collections::HashSet;
use std::fmt;
use std::num::ParseIntError;

const TAG_BOOL: &str = "tag:yaml.org,2002:bool";
const TAG_INT: &str = "tag:yaml.org,2002:int";
const TAG_FLOAT: &str = "tag:yaml.org,2002:float";
const TAG_NULL: &str = "tag:yaml.org,2002:null";

/// How deep sequences and mappings may nest while deserializing.
const RECURSION_LIMIT: u8 = 128;

/// Deserialize a `T` from `node`.
pub(crate) fn from_node<T: DeserializeOwned>(node: &Node) -> Result<T, Error> {
    T::deserialize(De::new(node, &Path::Root, RECURSION_LIMIT))
}

/// The path to a value, as in error messages: `gate.fail_on`, `exclude[1]`.
#[derive(Clone, Copy)]
enum Path<'a> {
    Root,
    Seq {
        parent: &'a Path<'a>,
        index: usize,
    },
    Map {
        parent: &'a Path<'a>,
        key: &'a str,
    },
    /// A value under a key that is not a scalar.
    Unknown {
        parent: &'a Path<'a>,
    },
}

impl fmt::Display for Path<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Parent<'a>(&'a Path<'a>);
        impl fmt::Display for Parent<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                match self.0 {
                    Path::Root => Ok(()),
                    p => write!(f, "{p}."),
                }
            }
        }
        match self {
            Path::Root => f.write_str("."),
            Path::Seq { parent, index } => write!(f, "{parent}[{index}]"),
            Path::Map { parent, key } => write!(f, "{}{key}", Parent(parent)),
            Path::Unknown { parent } => write!(f, "{}?", Parent(parent)),
        }
    }
}

/// The enum variant a local tag selected, while its value is read.
#[derive(Clone, Copy)]
struct CurrentEnum<'de> {
    name: Option<&'static str>,
    tag: &'de str,
}

/// Reads one node.
struct De<'de, 'p> {
    node: &'de Node,
    path: &'p Path<'p>,
    remaining_depth: u8,
    current_enum: Option<CurrentEnum<'de>>,
}

impl<'de, 'p> De<'de, 'p> {
    fn new(node: &'de Node, path: &'p Path<'p>, remaining_depth: u8) -> Self {
        De {
            node,
            path,
            remaining_depth,
            current_enum: None,
        }
    }

    fn mark(&self) -> Mark {
        self.node.mark
    }

    fn fix(&self, e: Error) -> Error {
        e.fix_mark(self.node.mark, self.path)
    }

    fn tagged_already(&self) -> bool {
        self.current_enum.is_some()
    }

    fn nested(&self) -> Result<u8, Error> {
        self.remaining_depth
            .checked_sub(1)
            .ok_or_else(|| Error::new(ErrorKind::RecursionLimitExceeded(self.mark())))
    }

    fn visit_seq<V: Visitor<'de>>(
        &self,
        items: &'de [Node],
        visitor: V,
    ) -> Result<V::Value, Error> {
        let mut seq = SeqAccess {
            items,
            index: 0,
            path: self.path,
            remaining_depth: self.nested()?,
        };
        let value = visitor.visit_seq(&mut seq)?;
        if let Some(e) = items[seq.index..].last().and_then(truncated) {
            return Err(e);
        }
        if seq.index < items.len() {
            return Err(de::Error::invalid_length(
                items.len(),
                &ExpectedLen(seq.index, "sequence of", "element", "elements"),
            ));
        }
        Ok(value)
    }

    fn visit_map<V: Visitor<'de>>(
        &self,
        entries: &'de [(Node, Node)],
        visitor: V,
    ) -> Result<V::Value, Error> {
        let mut map = MapAccess {
            entries,
            index: 0,
            path: self.path,
            remaining_depth: self.nested()?,
        };
        let value = visitor.visit_map(&mut map)?;
        if let Some(e) = entries[map.index..]
            .last()
            .and_then(|(k, v)| truncated(k).or_else(|| truncated(v)))
        {
            return Err(e);
        }
        if map.index < entries.len() {
            return Err(de::Error::invalid_length(
                entries.len(),
                &ExpectedLen(map.index, "map containing", "entry", "entries"),
            ));
        }
        Ok(value)
    }

    /// The local tag of the node, unless it already selected the enum variant being read.
    fn enum_tag(&self) -> Option<&'de str> {
        local_tag(self.own_tag())
    }

    /// The node's tag, unless it already selected the enum variant being read.
    fn own_tag(&self) -> Option<&'de str> {
        self.node.tag.as_deref().filter(|_| !self.tagged_already())
    }

    /// An empty plain scalar, or no document at all: read as an empty sequence or mapping.
    fn is_empty_value(&self) -> bool {
        match &self.node.value {
            Value::Void => true,
            Value::Scalar(s) => s.text.is_empty() && s.style == Style::Plain,
            _ => false,
        }
    }
}

/// The error of a node that ends where the text stopped being YAML (unread items are skipped up
/// to there, and fail there).
fn truncated(node: &Node) -> Option<Error> {
    match &node.value {
        Value::Missing(e) => Some((**e).clone()),
        Value::Seq(items) => items.last().and_then(truncated),
        Value::Map(entries) => entries
            .last()
            .and_then(|(k, v)| truncated(k).or_else(|| truncated(v))),
        _ => None,
    }
}

/// `!name` → `name` (`!` alone stays `!`); core schema tags are not local.
fn local_tag(tag: Option<&str>) -> Option<&str> {
    let rest = tag?.strip_prefix('!')?;
    Some(if rest.is_empty() { "!" } else { rest })
}

/// `sequence of 3 elements`, `map containing 1 entry`.
struct ExpectedLen(usize, &'static str, &'static str, &'static str);

impl Expected for ExpectedLen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let unit = if self.0 == 1 { self.2 } else { self.3 };
        write!(f, "{} {} {unit}", self.1, self.0)
    }
}

// ---------------------------------------------------------------------------------------------
// Scalars.

fn parse_null(v: &str) -> bool {
    matches!(v, "null" | "Null" | "NULL" | "~")
}

fn parse_bool(v: &str) -> Option<bool> {
    match v {
        "true" | "True" | "TRUE" => Some(true),
        "false" | "False" | "FALSE" => Some(false),
        _ => None,
    }
}

/// A leading zero followed by digits (`012`): a string in YAML 1.2, not an octal number.
fn leading_zero_digits(v: &str) -> bool {
    let v = v.strip_prefix(['-', '+']).unwrap_or(v);
    v.len() > 1 && v.starts_with('0') && v[1..].bytes().all(|b| b.is_ascii_digit())
}

/// `0x`, `0o`, `0b` with a radix; `None` when the digits carry a sign of their own.
fn radix_digits(v: &str) -> Option<(&str, u32)> {
    let (digits, radix) = [("0x", 16), ("0o", 8), ("0b", 2)]
        .into_iter()
        .find_map(|(prefix, radix)| Some((v.strip_prefix(prefix)?, radix)))?;
    (!digits.starts_with(['+', '-'])).then_some((digits, radix))
}

/// An integer of 0 or more: `12`, `+12`, `0x1F`, `0o17`, `0b101`.
fn parse_unsigned<T>(v: &str, from_radix: fn(&str, u32) -> Result<T, ParseIntError>) -> Option<T> {
    let unsigned = v.strip_prefix('+').unwrap_or(v);
    if let Some((digits, radix)) = radix_digits(unsigned) {
        if let Ok(n) = from_radix(digits, radix) {
            return Some(n);
        }
    }
    if unsigned.starts_with(['+', '-']) || leading_zero_digits(v) {
        return None;
    }
    from_radix(unsigned, 10).ok()
}

/// A negative integer: `-12`, `-0x1F`, `-0o17`, `-0b101`.
fn parse_negative<T>(v: &str, from_radix: fn(&str, u32) -> Result<T, ParseIntError>) -> Option<T> {
    if let Some((digits, radix)) = v.strip_prefix('-').and_then(radix_digits) {
        if let Ok(n) = from_radix(&format!("-{digits}"), radix) {
            return Some(n);
        }
    }
    if leading_zero_digits(v) {
        return None;
    }
    from_radix(v, 10).ok()
}

/// Any integer, for a signed target.
fn parse_signed<T>(v: &str, from_radix: fn(&str, u32) -> Result<T, ParseIntError>) -> Option<T> {
    let unsigned = match v.strip_prefix('+') {
        Some(rest) if rest.starts_with(['+', '-']) => return None,
        Some(rest) => rest,
        None => v,
    };
    if let Some((digits, radix)) = radix_digits(unsigned) {
        if let Ok(n) = from_radix(digits, radix) {
            return Some(n);
        }
    }
    if let Some((digits, radix)) = v.strip_prefix('-').and_then(radix_digits) {
        if let Ok(n) = from_radix(&format!("-{digits}"), radix) {
            return Some(n);
        }
    }
    if leading_zero_digits(v) {
        return None;
    }
    from_radix(unsigned, 10).ok()
}

/// A float: `1.5`, `1e3`, `.inf`, `-.inf`, `.nan` (and their capitalized forms).
fn parse_float(v: &str) -> Option<f64> {
    let unsigned = match v.strip_prefix('+') {
        Some(rest) if rest.starts_with(['+', '-']) => return None,
        Some(rest) => rest,
        None => v,
    };
    if matches!(unsigned, ".inf" | ".Inf" | ".INF") {
        return Some(f64::INFINITY);
    }
    if matches!(v, "-.inf" | "-.Inf" | "-.INF") {
        return Some(f64::NEG_INFINITY);
    }
    if matches!(v, ".nan" | ".NaN" | ".NAN") {
        return Some(f64::NAN);
    }
    // Rust also reads `inf`, `nan`, `infinity`: YAML does not.
    unsigned.parse::<f64>().ok().filter(|f| f.is_finite())
}

/// Visit an integer of any size, or give the visitor back.
fn visit_int<'de, V: Visitor<'de>>(visitor: V, v: &str) -> Result<Result<V::Value, Error>, V> {
    if let Some(n) = parse_unsigned(v, u64::from_str_radix) {
        return Ok(visitor.visit_u64(n));
    }
    if let Some(n) = parse_negative(v, i64::from_str_radix) {
        return Ok(visitor.visit_i64(n));
    }
    if let Some(n) = parse_unsigned(v, u128::from_str_radix) {
        return Ok(visitor.visit_u128(n));
    }
    if let Some(n) = parse_negative(v, i128::from_str_radix) {
        return Ok(visitor.visit_i128(n));
    }
    Err(visitor)
}

/// A plain scalar by the core schema: null, boolean, integer, float, else string.
fn visit_plain<'de, V: Visitor<'de>>(visitor: V, v: &'de str) -> Result<V::Value, Error> {
    if v.is_empty() || parse_null(v) {
        return visitor.visit_unit();
    }
    if let Some(b) = parse_bool(v) {
        return visitor.visit_bool(b);
    }
    let visitor = match visit_int(visitor, v) {
        Ok(result) => return result,
        Err(visitor) => visitor,
    };
    if !leading_zero_digits(v) {
        if let Some(f) = parse_float(v) {
            return visitor.visit_f64(f);
        }
    }
    visitor.visit_borrowed_str(v)
}

/// A scalar for a target without a type of its own: by its core schema tag, if it has one, else
/// by the core schema when plain, else a string.
fn visit_scalar<'de, V: Visitor<'de>>(
    visitor: V,
    s: &'de Scalar,
    tag: Option<&str>,
) -> Result<V::Value, Error> {
    let v = s.text.as_str();
    match tag {
        Some(tag) => {
            if tag == TAG_BOOL {
                return match parse_bool(v) {
                    Some(b) => visitor.visit_bool(b),
                    None => Err(de::Error::invalid_value(Unexpected::Str(v), &"a boolean")),
                };
            } else if tag == TAG_INT {
                return match visit_int(visitor, v) {
                    Ok(result) => result,
                    Err(_) => Err(de::Error::invalid_value(Unexpected::Str(v), &"an integer")),
                };
            } else if tag == TAG_FLOAT {
                return match parse_float(v) {
                    Some(f) => visitor.visit_f64(f),
                    None => Err(de::Error::invalid_value(Unexpected::Str(v), &"a float")),
                };
            } else if tag == TAG_NULL {
                return if parse_null(v) {
                    visitor.visit_unit()
                } else {
                    Err(de::Error::invalid_value(Unexpected::Str(v), &"null"))
                };
            } else if tag.starts_with('!') && s.style == Style::Plain {
                return visit_plain(visitor, v);
            }
        }
        _ if s.style == Style::Plain => return visit_plain(visitor, v),
        _ => {}
    }
    visitor.visit_borrowed_str(v)
}

/// A plain scalar, or a literal block tagged with `expected` (`!!int |`), for a typed target.
fn is_plain_or_tagged_literal(expected: &str, s: &Scalar, tag: Option<&str>) -> bool {
    match (s.style, tag) {
        (Style::Plain, _) => true,
        (Style::Literal, Some(tag)) => tag == expected,
        _ => false,
    }
}

/// The error for a node that is not what `exp` expects, naming what it is.
fn invalid_type(node: &Node, exp: &dyn Expected) -> Error {
    enum Never {}
    struct InvalidType<'a>(&'a dyn Expected);
    impl Visitor<'_> for InvalidType<'_> {
        type Value = Never;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.fmt(f)
        }
    }
    match &node.value {
        Value::Scalar(s) => match visit_scalar(InvalidType(exp), s, node.tag.as_deref()) {
            Ok(never) => match never {},
            Err(e) => e,
        },
        Value::Seq(_) => de::Error::invalid_type(Unexpected::Seq, exp),
        Value::Map(_) => de::Error::invalid_type(Unexpected::Map, exp),
        Value::Void => Error::new(ErrorKind::EndOfStream),
        Value::Missing(e) => (**e).clone(),
    }
}

// ---------------------------------------------------------------------------------------------
// The deserializer.

/// Reading a node where the text stopped being YAML gives that error.
macro_rules! missing {
    ($de:expr) => {
        if let Value::Missing(e) = &$de.node.value {
            return Err((**e).clone());
        }
    };
}

macro_rules! typed_scalar {
    ($method:ident, $tag:expr, $parse:expr, $visit:ident) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
            missing!(self);
            let result = match &self.node.value {
                Value::Scalar(s) if is_plain_or_tagged_literal($tag, s, self.own_tag()) => {
                    match ($parse)(s.text.as_str()) {
                        Some(v) => visitor.$visit(v),
                        None => Err(invalid_type(self.node, &visitor)),
                    }
                }
                _ => Err(invalid_type(self.node, &visitor)),
            };
            result.map_err(|e| self.fix(e))
        }
    };
}

impl<'de> de::Deserializer<'de> for De<'de, '_> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        missing!(self);
        let (mark, path) = (self.mark(), self.path);
        let result = if let Some(tag) = self.enum_tag() {
            visitor.visit_enum(EnumAccess {
                de: self,
                name: None,
                tag,
            })
        } else {
            match &self.node.value {
                Value::Void => visitor.visit_none(),
                Value::Missing(e) => Err((**e).clone()),
                Value::Scalar(s) => visit_scalar(visitor, s, self.own_tag()),
                Value::Seq(items) => self.visit_seq(items, visitor),
                Value::Map(entries) => self.visit_map(entries, visitor),
            }
        };
        result.map_err(|e| e.fix_mark(mark, path))
    }

    typed_scalar!(deserialize_bool, TAG_BOOL, parse_bool, visit_bool);
    typed_scalar!(
        deserialize_i64,
        TAG_INT,
        |v| parse_signed(v, i64::from_str_radix),
        visit_i64
    );
    typed_scalar!(
        deserialize_i128,
        TAG_INT,
        |v| parse_signed(v, i128::from_str_radix),
        visit_i128
    );
    typed_scalar!(
        deserialize_u64,
        TAG_INT,
        |v| parse_unsigned(v, u64::from_str_radix),
        visit_u64
    );
    typed_scalar!(
        deserialize_u128,
        TAG_INT,
        |v| parse_unsigned(v, u128::from_str_radix),
        visit_u128
    );
    typed_scalar!(deserialize_f64, TAG_FLOAT, parse_float, visit_f64);

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_i64(visitor)
    }
    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_i64(visitor)
    }
    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_i64(visitor)
    }
    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_u64(visitor)
    }
    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_u64(visitor)
    }
    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_u64(visitor)
    }
    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_f64(visitor)
    }
    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    /// Any scalar's text, whatever its tag.
    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        missing!(self);
        let result = match &self.node.value {
            Value::Scalar(s) => visitor.visit_borrowed_str(&s.text),
            _ => Err(invalid_type(self.node, &visitor)),
        };
        result.map_err(|e| self.fix(e))
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(Error::new(ErrorKind::BytesUnsupported))
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Error> {
        Err(Error::new(ErrorKind::BytesUnsupported))
    }

    /// Null (plain `null`, `~`, nothing, or `!!null`) is `None`; anything else `Some`.
    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        missing!(self);
        let is_some = match &self.node.value {
            Value::Void => false,
            Value::Scalar(s) => {
                if s.style != Style::Plain {
                    true
                } else if let Some(tag) = self.own_tag() {
                    if tag != TAG_NULL {
                        true
                    } else if parse_null(&s.text) {
                        false
                    } else {
                        return Err(de::Error::invalid_value(Unexpected::Str(&s.text), &"null"));
                    }
                } else {
                    !s.text.is_empty() && !parse_null(&s.text)
                }
            }
            Value::Seq(_) | Value::Map(_) | Value::Missing(_) => true,
        };
        if is_some {
            visitor.visit_some(self)
        } else {
            visitor.visit_none()
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        missing!(self);
        let result = match &self.node.value {
            Value::Void => visitor.visit_unit(),
            Value::Scalar(s) => {
                let is_null = if s.style != Style::Plain {
                    false
                } else if let Some(tag) = self.own_tag() {
                    tag == TAG_NULL && parse_null(&s.text)
                } else {
                    s.text.is_empty() || parse_null(&s.text)
                };
                if is_null {
                    visitor.visit_unit()
                } else {
                    Err(de::Error::invalid_value(Unexpected::Str(&s.text), &"null"))
                }
            }
            _ => Err(invalid_type(self.node, &visitor)),
        };
        result.map_err(|e| self.fix(e))
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        missing!(self);
        let remaining_depth = self.nested()?;
        visitor.visit_newtype_struct(De {
            remaining_depth,
            ..self
        })
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        missing!(self);
        let result = match &self.node.value {
            Value::Seq(items) => self.visit_seq(items, visitor),
            _ if self.is_empty_value() => visitor.visit_seq(SeqAccess {
                items: &[],
                index: 0,
                path: self.path,
                remaining_depth: self.remaining_depth,
            }),
            _ => Err(invalid_type(self.node, &visitor)),
        };
        result.map_err(|e| self.fix(e))
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        missing!(self);
        let result = match &self.node.value {
            Value::Map(entries) => self.visit_map(entries, visitor),
            _ if self.is_empty_value() => visitor.visit_map(MapAccess {
                entries: &[],
                index: 0,
                path: self.path,
                remaining_depth: self.remaining_depth,
            }),
            _ => Err(invalid_type(self.node, &visitor)),
        };
        result.map_err(|e| self.fix(e))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    /// A variant is a plain scalar (a unit variant) or a node with a local tag (`!Variant
    /// value`).
    fn deserialize_enum<V: Visitor<'de>>(
        self,
        name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        missing!(self);
        let (mark, path) = (self.mark(), self.path);
        let result = if let Some(current) = self.current_enum {
            match &self.node.value {
                Value::Scalar(s) if !s.text.is_empty() => {
                    visitor.visit_enum(UnitVariantAccess { de: self })
                }
                _ => {
                    let outer = match current.name {
                        Some(name) => format!("{name}::{}", current.tag),
                        None => format!("!{}", current.tag),
                    };
                    Err(de::Error::custom(format!(
                        "deserializing nested enum in {outer} from YAML is not supported yet"
                    )))
                }
            }
        } else {
            match (&self.node.value, local_tag(self.node.tag.as_deref())) {
                (Value::Void, _) => Err(Error::new(ErrorKind::EndOfStream)),
                (Value::Missing(e), _) => Err((**e).clone()),
                (_, Some(tag)) => visitor.visit_enum(EnumAccess {
                    de: self,
                    name: Some(name),
                    tag,
                }),
                (Value::Scalar(_), None) => visitor.visit_enum(UnitVariantAccess { de: self }),
                (Value::Map(_), None) => Err(de::Error::invalid_type(
                    Unexpected::Map,
                    &"a YAML tag starting with '!'",
                )),
                (Value::Seq(_), None) => Err(de::Error::invalid_type(
                    Unexpected::Seq,
                    &"a YAML tag starting with '!'",
                )),
            }
        };
        result.map_err(|e| e.fix_mark(mark, path))
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        missing!(self);
        visitor.visit_unit()
    }
}

struct SeqAccess<'de, 'p> {
    items: &'de [Node],
    index: usize,
    path: &'p Path<'p>,
    remaining_depth: u8,
}

impl<'de> de::SeqAccess<'de> for SeqAccess<'de, '_> {
    type Error = Error;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Error> {
        let Some(node) = self.items.get(self.index) else {
            return Ok(None);
        };
        let path = Path::Seq {
            parent: self.path,
            index: self.index,
        };
        self.index += 1;
        seed.deserialize(De::new(node, &path, self.remaining_depth))
            .map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.items.len() - self.index)
    }
}

struct MapAccess<'de, 'p> {
    entries: &'de [(Node, Node)],
    /// Entries whose key has been read.
    index: usize,
    path: &'p Path<'p>,
    remaining_depth: u8,
}

impl<'de> de::MapAccess<'de> for MapAccess<'de, '_> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        let Some((key, _)) = self.entries.get(self.index) else {
            return Ok(None);
        };
        self.index += 1;
        // A key is read at the path of its mapping.
        seed.deserialize(De::new(key, self.path, self.remaining_depth))
            .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let Some((key, value)) = self.index.checked_sub(1).and_then(|i| self.entries.get(i)) else {
            return Err(de::Error::custom("value requested before its key"));
        };
        let path = match &key.value {
            Value::Scalar(s) => Path::Map {
                parent: self.path,
                key: &s.text,
            },
            _ => Path::Unknown { parent: self.path },
        };
        seed.deserialize(De::new(value, &path, self.remaining_depth))
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.entries.len() - self.index)
    }
}

/// The variant named by a local tag; the node is its value.
struct EnumAccess<'de, 'p> {
    de: De<'de, 'p>,
    name: Option<&'static str>,
    tag: &'de str,
}

impl<'de, 'p> de::EnumAccess<'de> for EnumAccess<'de, 'p> {
    type Error = Error;
    type Variant = De<'de, 'p>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let tag: StrDeserializer<'_, Error> = self.tag.into_deserializer();
        let variant = seed.deserialize(tag)?;
        let de = De {
            current_enum: Some(CurrentEnum {
                name: self.name,
                tag: self.tag,
            }),
            ..self.de
        };
        Ok((variant, de))
    }
}

impl<'de> de::VariantAccess<'de> for De<'de, '_> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        <()>::deserialize(self)
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        seed.deserialize(self)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, visitor: V) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_seq(self, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        de::Deserializer::deserialize_map(self, visitor)
    }
}

/// A unit variant named by the scalar itself.
struct UnitVariantAccess<'de, 'p> {
    de: De<'de, 'p>,
}

impl<'de, 'p> de::EnumAccess<'de> for UnitVariantAccess<'de, 'p> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), Error> {
        let de = De { ..self.de };
        Ok((seed.deserialize(de)?, self))
    }
}

impl<'de> de::VariantAccess<'de> for UnitVariantAccess<'de, '_> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value, Error> {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"newtype variant",
        ))
    }

    fn tuple_variant<V: Visitor<'de>>(self, _len: usize, _visitor: V) -> Result<V::Value, Error> {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"tuple variant",
        ))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Error> {
        Err(de::Error::invalid_type(
            Unexpected::UnitVariant,
            &"struct variant",
        ))
    }
}

// ---------------------------------------------------------------------------------------------
// Generic values.

/// Check `node` as a generic YAML value, as the configuration's lenient pass always has: a
/// mapping with the same key twice (`1` and `0x1` are the same key, `1` and `"1"` are not) or
/// an integer beyond 64 bits is an error.
pub(crate) fn check_generic(node: &Node) -> Result<(), Error> {
    from_node::<Generic>(node).map(|_| ())
}

/// A generic value, compared by what it means.
#[derive(Clone, PartialEq, Eq, Hash)]
enum Generic {
    Null,
    Bool(bool),
    PosInt(u64),
    NegInt(i64),
    /// Bits of a float; every NaN and both zeros as one.
    Float(u64),
    String(String),
    Seq(Vec<Generic>),
    Map(Vec<(Generic, Generic)>),
    Tagged(String, Box<Generic>),
}

impl Generic {
    fn float(f: f64) -> Generic {
        let f = if f.is_nan() {
            f64::NAN
        } else if f == 0.0 {
            0.0
        } else {
            f
        };
        Generic::Float(f.to_bits())
    }
}

/// `with key "a/b"`, `with key 1`, `with null key`, `in YAML map`.
impl fmt::Display for Generic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Generic::Null => f.write_str("with null key"),
            Generic::Bool(b) => write!(f, "with key `{b}`"),
            Generic::PosInt(n) => write!(f, "with key {n}"),
            Generic::NegInt(n) => write!(f, "with key {n}"),
            Generic::Float(bits) => write!(f, "with key {}", float_text(f64::from_bits(*bits))),
            Generic::String(s) => write!(f, "with key {s:?}"),
            Generic::Seq(_) | Generic::Map(_) | Generic::Tagged(..) => f.write_str("in YAML map"),
        }
    }
}

/// A float as YAML writes it: `.nan`, `-.inf`, `1.5`, `1000.0`.
fn float_text(f: f64) -> String {
    if f.is_nan() {
        ".nan".into()
    } else if f.is_infinite() {
        if f < 0.0 { "-.inf" } else { ".inf" }.into()
    } else {
        format!("{f:?}")
    }
}

impl<'de> Deserialize<'de> for Generic {
    fn deserialize<D: de::Deserializer<'de>>(d: D) -> Result<Generic, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Generic;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("any YAML value")
            }
            fn visit_bool<E: de::Error>(self, b: bool) -> Result<Generic, E> {
                Ok(Generic::Bool(b))
            }
            fn visit_i64<E: de::Error>(self, n: i64) -> Result<Generic, E> {
                Ok(if n < 0 {
                    Generic::NegInt(n)
                } else {
                    Generic::PosInt(n as u64)
                })
            }
            fn visit_u64<E: de::Error>(self, n: u64) -> Result<Generic, E> {
                Ok(Generic::PosInt(n))
            }
            fn visit_f64<E: de::Error>(self, f: f64) -> Result<Generic, E> {
                Ok(Generic::float(f))
            }
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Generic, E> {
                Ok(Generic::String(s.to_string()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Generic, E> {
                Ok(Generic::Null)
            }
            fn visit_none<E: de::Error>(self) -> Result<Generic, E> {
                Ok(Generic::Null)
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Generic, A::Error> {
                let mut items = Vec::new();
                while let Some(item) = seq.next_element()? {
                    items.push(item);
                }
                Ok(Generic::Seq(items))
            }
            fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<Generic, A::Error> {
                let mut entries = Vec::new();
                let mut seen = HashSet::new();
                while let Some(key) = map.next_key::<Generic>()? {
                    if !seen.insert(key.clone()) {
                        return Err(de::Error::custom(format_args!("duplicate entry {key}")));
                    }
                    let value = map.next_value()?;
                    entries.push((key, value));
                }
                Ok(Generic::Map(entries))
            }
            fn visit_enum<A: de::EnumAccess<'de>>(self, data: A) -> Result<Generic, A::Error> {
                use de::VariantAccess;
                let (tag, variant) = data.variant::<String>()?;
                Ok(Generic::Tagged(tag, Box::new(variant.newtype_variant()?)))
            }
        }
        d.deserialize_any(V)
    }
}

/// What a node means where the target type leaves it open, as `deserialize_any` reads it.
#[derive(Debug, PartialEq)]
pub(crate) enum Meaning<'a> {
    Null,
    Bool(bool),
    U64(u64),
    I64(i64),
    /// An integer beyond 64 bits, as written.
    Big(&'a str),
    Float(f64),
    Str(&'a str),
    /// A scalar with a local tag (an enum variant) or a core tag it does not match.
    Tagged,
    Seq(&'a [Node]),
    Map(&'a [(Node, Node)]),
}

/// What `node` means; see [`Meaning`].
pub(crate) fn meaning(node: &Node) -> Meaning<'_> {
    struct M<'a>(&'a str);
    impl<'a> Visitor<'a> for M<'a> {
        type Value = Meaning<'a>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a scalar")
        }
        fn visit_bool<E: de::Error>(self, b: bool) -> Result<Meaning<'a>, E> {
            Ok(Meaning::Bool(b))
        }
        fn visit_i64<E: de::Error>(self, n: i64) -> Result<Meaning<'a>, E> {
            Ok(Meaning::I64(n))
        }
        fn visit_u64<E: de::Error>(self, n: u64) -> Result<Meaning<'a>, E> {
            Ok(Meaning::U64(n))
        }
        fn visit_i128<E: de::Error>(self, _: i128) -> Result<Meaning<'a>, E> {
            Ok(Meaning::Big(self.0))
        }
        fn visit_u128<E: de::Error>(self, _: u128) -> Result<Meaning<'a>, E> {
            Ok(Meaning::Big(self.0))
        }
        fn visit_f64<E: de::Error>(self, f: f64) -> Result<Meaning<'a>, E> {
            Ok(Meaning::Float(f))
        }
        fn visit_borrowed_str<E: de::Error>(self, s: &'a str) -> Result<Meaning<'a>, E> {
            Ok(Meaning::Str(s))
        }
        fn visit_unit<E: de::Error>(self) -> Result<Meaning<'a>, E> {
            Ok(Meaning::Null)
        }
    }
    match &node.value {
        Value::Void => Meaning::Null,
        Value::Missing(_) => Meaning::Tagged,
        Value::Seq(items) => Meaning::Seq(items),
        Value::Map(entries) => Meaning::Map(entries),
        Value::Scalar(_) if local_tag(node.tag.as_deref()).is_some() => Meaning::Tagged,
        Value::Scalar(s) => {
            visit_scalar(M(&s.text), s, node.tag.as_deref()).unwrap_or(Meaning::Tagged)
        }
    }
}

/// A mapping key as text, as the configuration names it: a plain scalar by the core schema
/// (`~` is `null`, `0x1F` is `31`, `True` is `true`), any other scalar as written, a sequence or
/// mapping in flow style.
pub(crate) fn key_text(node: &Node) -> String {
    match meaning(node) {
        Meaning::Null => "null".to_string(),
        Meaning::Bool(b) => b.to_string(),
        Meaning::U64(n) => n.to_string(),
        Meaning::I64(n) => n.to_string(),
        Meaning::Float(f) => float_text(f),
        Meaning::Big(s) | Meaning::Str(s) => s.to_string(),
        // A local tag, as the old parser wrote it: `!x y`.
        Meaning::Tagged => match (&node.value, node.tag.as_deref()) {
            (Value::Scalar(s), Some(tag)) if tag.starts_with('!') => format!("{tag} {}", s.text),
            (Value::Scalar(s), _) => s.text.clone(),
            _ => String::new(),
        },
        Meaning::Seq(items) => format!(
            "[{}]",
            items.iter().map(key_text).collect::<Vec<_>>().join(", ")
        ),
        Meaning::Map(entries) => format!(
            "{{{}}}",
            entries
                .iter()
                .map(|(k, v)| format!("{}: {}", key_text(k), key_text(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

/// `node` as the configuration's lenient pass reads it: each scalar as the generic value it
/// means, written back the way that value is written (`~` as `null`, `0x1F` as `31`, `True` as
/// `true`, a string quoted, an empty value as `null`), as the YAML round trip of the old parser
/// did.
pub(crate) fn canonical(node: &Node) -> Node {
    struct Canon;
    impl Visitor<'_> for Canon {
        type Value = Scalar;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a scalar")
        }
        fn visit_bool<E: de::Error>(self, b: bool) -> Result<Scalar, E> {
            Ok(plain(b.to_string()))
        }
        fn visit_i64<E: de::Error>(self, n: i64) -> Result<Scalar, E> {
            Ok(plain(n.to_string()))
        }
        fn visit_u64<E: de::Error>(self, n: u64) -> Result<Scalar, E> {
            Ok(plain(n.to_string()))
        }
        fn visit_i128<E: de::Error>(self, n: i128) -> Result<Scalar, E> {
            Ok(plain(n.to_string()))
        }
        fn visit_u128<E: de::Error>(self, n: u128) -> Result<Scalar, E> {
            Ok(plain(n.to_string()))
        }
        fn visit_f64<E: de::Error>(self, f: f64) -> Result<Scalar, E> {
            Ok(plain(float_text(f)))
        }
        fn visit_str<E: de::Error>(self, s: &str) -> Result<Scalar, E> {
            Ok(Scalar {
                text: s.to_string(),
                style: Style::DoubleQuoted,
            })
        }
        fn visit_unit<E: de::Error>(self) -> Result<Scalar, E> {
            Ok(plain("null".to_string()))
        }
    }
    fn plain(text: String) -> Scalar {
        Scalar {
            text,
            style: Style::Plain,
        }
    }
    // Local tags stay (an enum variant); core schema tags have done their work.
    let local = local_tag(node.tag.as_deref()).is_some();
    let value = match &node.value {
        Value::Seq(items) => Value::Seq(items.iter().map(canonical).collect()),
        Value::Map(entries) => Value::Map(
            entries
                .iter()
                .map(|(k, v)| (canonical(k), canonical(v)))
                .collect(),
        ),
        Value::Scalar(s) => {
            let tag = if local { None } else { node.tag.as_deref() };
            Value::Scalar(visit_scalar(Canon, s, tag).unwrap_or_else(|_| s.clone()))
        }
        Value::Void | Value::Missing(_) => return node.clone(),
    };
    Node {
        value,
        tag: node.tag.clone().filter(|_| local),
        mark: node.mark,
    }
}
