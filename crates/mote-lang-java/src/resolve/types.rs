//! Java types as seen by the resolver.

use std::borrow::Cow;
use std::fmt;

/// A primitive type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Prim {
    Boolean,
    Byte,
    Short,
    Char,
    Int,
    Long,
    Float,
    Double,
}

impl Prim {
    pub fn from_name(s: &str) -> Option<Prim> {
        Some(match s {
            "boolean" => Prim::Boolean,
            "byte" => Prim::Byte,
            "short" => Prim::Short,
            "char" => Prim::Char,
            "int" => Prim::Int,
            "long" => Prim::Long,
            "float" => Prim::Float,
            "double" => Prim::Double,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Prim::Boolean => "boolean",
            Prim::Byte => "byte",
            Prim::Short => "short",
            Prim::Char => "char",
            Prim::Int => "int",
            Prim::Long => "long",
            Prim::Float => "float",
            Prim::Double => "double",
        }
    }
    /// Everything except `boolean`.
    pub fn is_numeric(self) -> bool {
        self != Prim::Boolean
    }
    /// `byte`, `short`, `char`, `int`, `long`.
    pub fn is_integral(self) -> bool {
        matches!(
            self,
            Prim::Byte | Prim::Short | Prim::Char | Prim::Int | Prim::Long
        )
    }
    /// Fully-qualified name of the wrapper class.
    pub fn boxed_fqn(self) -> &'static str {
        match self {
            Prim::Boolean => "java.lang.Boolean",
            Prim::Byte => "java.lang.Byte",
            Prim::Short => "java.lang.Short",
            Prim::Char => "java.lang.Character",
            Prim::Int => "java.lang.Integer",
            Prim::Long => "java.lang.Long",
            Prim::Float => "java.lang.Float",
            Prim::Double => "java.lang.Double",
        }
    }
    /// The primitive a wrapper class unboxes to.
    pub fn from_boxed(fqn: &str) -> Option<Prim> {
        Some(match fqn {
            "java.lang.Boolean" => Prim::Boolean,
            "java.lang.Byte" => Prim::Byte,
            "java.lang.Short" => Prim::Short,
            "java.lang.Character" => Prim::Char,
            "java.lang.Integer" => Prim::Int,
            "java.lang.Long" => Prim::Long,
            "java.lang.Float" => Prim::Float,
            "java.lang.Double" => Prim::Double,
            _ => return None,
        })
    }
    fn rank(self) -> u8 {
        match self {
            Prim::Boolean => 0,
            Prim::Byte => 1,
            Prim::Short | Prim::Char => 2,
            Prim::Int => 3,
            Prim::Long => 4,
            Prim::Float => 5,
            Prim::Double => 6,
        }
    }
    /// Whether a value of `self` converts to `to` by identity or primitive widening.
    pub fn widens_to(self, to: Prim) -> bool {
        if self == to {
            return true;
        }
        if self == Prim::Boolean || to == Prim::Boolean || to == Prim::Char {
            return false;
        }
        if self == Prim::Char {
            return to.rank() >= 3;
        }
        if to == Prim::Short {
            return self == Prim::Byte;
        }
        to.rank() > self.rank()
    }
    /// Binary numeric promotion.
    pub fn promote(a: Prim, b: Prim) -> Prim {
        if a == Prim::Double || b == Prim::Double {
            Prim::Double
        } else if a == Prim::Float || b == Prim::Float {
            Prim::Float
        } else if a == Prim::Long || b == Prim::Long {
            Prim::Long
        } else {
            Prim::Int
        }
    }
}

/// A class or interface type with its type arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClassType {
    /// Fully-qualified (canonical) name: `java.util.Map.Entry`, `com.acme.Order.Line`.
    pub name: Cow<'static, str>,
    /// Type arguments; empty for non-generic or raw types. Unresolvable arguments are
    /// [`JType::Unknown`].
    pub args: Vec<JType>,
}

/// A resolved Java type. [`crate::Ctx::type_of`] never returns [`JType::Unknown`] at the top
/// level (it returns `None` instead); `Unknown` only appears inside type arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum JType {
    Prim(Prim),
    /// Result of a `void` method.
    Void,
    /// The type of the `null` literal.
    Null,
    Class(ClassType),
    Array(Box<JType>),
    /// A type we could not resolve (type variables, repo or third-party types) — only ever
    /// nested inside a known type.
    Unknown,
}

impl JType {
    /// A class type with no type arguments.
    pub fn class(fqn: impl Into<Cow<'static, str>>) -> JType {
        JType::Class(ClassType {
            name: fqn.into(),
            args: Vec::new(),
        })
    }
    /// A class type with type arguments.
    pub fn generic(fqn: impl Into<Cow<'static, str>>, args: Vec<JType>) -> JType {
        JType::Class(ClassType {
            name: fqn.into(),
            args,
        })
    }
    pub fn string() -> JType {
        JType::class("java.lang.String")
    }
    /// Fully-qualified name for class types.
    pub fn fqn(&self) -> Option<&str> {
        match self {
            JType::Class(c) => Some(&c.name),
            _ => None,
        }
    }
    /// Simple name for class types (`List` for `java.util.List`).
    pub fn simple_name(&self) -> Option<&str> {
        self.fqn().map(|n| n.rsplit('.').next().unwrap_or(n))
    }
    /// Type argument `i` if present and known.
    pub fn type_arg(&self, i: usize) -> Option<&JType> {
        match self {
            JType::Class(c) => c.args.get(i).filter(|t| !t.is_unknown()),
            _ => None,
        }
    }
    /// Element type of an array.
    pub fn element_type(&self) -> Option<&JType> {
        match self {
            JType::Array(e) if !e.is_unknown() => Some(e),
            _ => None,
        }
    }
    pub fn is_unknown(&self) -> bool {
        matches!(self, JType::Unknown)
    }
    pub fn is_null(&self) -> bool {
        matches!(self, JType::Null)
    }
    pub fn is_void(&self) -> bool {
        matches!(self, JType::Void)
    }
    pub fn is_array(&self) -> bool {
        matches!(self, JType::Array(_))
    }
    pub fn is_primitive(&self) -> bool {
        matches!(self, JType::Prim(_))
    }
    /// Class, array, or null types.
    pub fn is_reference(&self) -> bool {
        matches!(self, JType::Class(_) | JType::Array(_) | JType::Null)
    }
    pub fn prim(&self) -> Option<Prim> {
        match self {
            JType::Prim(p) => Some(*p),
            _ => None,
        }
    }
    /// `java.lang.String`.
    pub fn is_string(&self) -> bool {
        self.fqn() == Some("java.lang.String")
    }
    /// Whether this is the class `fqn` itself (ignoring type arguments).
    pub fn is_class(&self, fqn: &str) -> bool {
        self.fqn() == Some(fqn)
    }
    /// A wrapper class (`Integer`, `Boolean`, …).
    pub fn is_boxed(&self) -> bool {
        self.fqn().and_then(Prim::from_boxed).is_some()
    }
    /// The primitive for a primitive or wrapper type.
    pub fn unboxed(&self) -> Option<Prim> {
        match self {
            JType::Prim(p) => Some(*p),
            JType::Class(c) => Prim::from_boxed(&c.name),
            _ => None,
        }
    }
    /// A numeric primitive (`byte` … `double`, including `char`).
    pub fn is_numeric_primitive(&self) -> bool {
        self.prim().is_some_and(Prim::is_numeric)
    }
    /// A numeric primitive or its wrapper.
    pub fn is_numeric(&self) -> bool {
        self.unboxed().is_some_and(Prim::is_numeric)
    }
    /// `boolean`.
    pub fn is_boolean(&self) -> bool {
        self.prim() == Some(Prim::Boolean)
    }
    /// `boolean` or `Boolean`.
    pub fn is_boolean_like(&self) -> bool {
        self.unboxed() == Some(Prim::Boolean)
    }
    /// Whether the type (at any depth) contains no `Unknown`.
    pub fn is_fully_known(&self) -> bool {
        match self {
            JType::Unknown => false,
            JType::Class(c) => c.args.iter().all(JType::is_fully_known),
            JType::Array(e) => e.is_fully_known(),
            _ => true,
        }
    }
}

impl fmt::Display for JType {
    /// Short display form for messages: `List<String>`, `int[]`, `Map.Entry<K, ?>`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JType::Prim(p) => f.write_str(p.name()),
            JType::Void => f.write_str("void"),
            JType::Null => f.write_str("null"),
            JType::Unknown => f.write_str("?"),
            JType::Array(e) => write!(f, "{e}[]"),
            JType::Class(c) => {
                // Drop the package (lower-case leading segments).
                let mut simple = c.name.as_ref();
                let mut rest = c.name.as_ref();
                while let Some((head, tail)) = rest.split_once('.') {
                    if head.chars().next().is_some_and(char::is_lowercase) {
                        simple = tail;
                        rest = tail;
                    } else {
                        break;
                    }
                }
                f.write_str(simple)?;
                if !c.args.is_empty() {
                    f.write_str("<")?;
                    for (i, a) in c.args.iter().enumerate() {
                        if i > 0 {
                            f.write_str(", ")?;
                        }
                        write!(f, "{a}")?;
                    }
                    f.write_str(">")?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widening_and_display() {
        assert!(Prim::Int.widens_to(Prim::Long));
        assert!(Prim::Char.widens_to(Prim::Int));
        assert!(!Prim::Byte.widens_to(Prim::Char));
        assert!(!Prim::Long.widens_to(Prim::Int));
        assert!(Prim::Long.widens_to(Prim::Float));
        let t = JType::generic("java.util.Map.Entry", vec![JType::string(), JType::Unknown]);
        assert_eq!(t.to_string(), "Map.Entry<String, ?>");
        assert_eq!(
            JType::Array(Box::new(JType::Prim(Prim::Int))).to_string(),
            "int[]"
        );
    }
}
