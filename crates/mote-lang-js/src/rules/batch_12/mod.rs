//! Rule batch 12: the opt-in `conventions` set for JavaScript and TypeScript (naming, braces,
//! declarations, exports, type assertions and other style-guide checks). Every rule here is
//! incubating, off by default and tagged `conventions`. See `RULE_AUTHORING.md`.

mod array_type;
mod consistent_type_assertions;
mod const_enum;
mod curly;
mod default_case_last;
mod default_export;
mod double_assertion;
mod mutable_export;
mod naming_convention;
mod no_extraneous_class;
mod no_multi_str;
mod no_object_constructor;
mod one_var;
mod prefer_arrow_callback;
mod prefer_for_of;
mod triple_slash_reference;

use crate::JsRule;

/// The rules of batch 12, in a stable order.
pub fn rules() -> Vec<Box<dyn JsRule>> {
    vec![
        Box::new(naming_convention::NamingConvention),
        Box::new(curly::Curly),
        Box::new(one_var::OneVar),
        Box::new(mutable_export::MutableExport),
        Box::new(no_object_constructor::NoObjectConstructor),
        Box::new(consistent_type_assertions::ConsistentTypeAssertions),
        Box::new(double_assertion::DoubleAssertion),
        Box::new(array_type::ArrayType),
        Box::new(const_enum::ConstEnum),
        Box::new(triple_slash_reference::TripleSlashReference),
        Box::new(no_multi_str::NoMultiStr),
        Box::new(default_case_last::DefaultCaseLast),
        Box::new(prefer_for_of::PreferForOf),
        Box::new(no_extraneous_class::NoExtraneousClass),
        Box::new(prefer_arrow_callback::PreferArrowCallback),
        Box::new(default_export::DefaultExport),
    ]
}
