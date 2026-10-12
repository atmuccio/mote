//! Modules shared by several rule batches, compiled once: the `java.util.regex` pattern parser,
//! the regex call-site helper, and the call / string-constant helpers they rely on (written for
//! batch 9; used by batches 9, 13 and 14). A batch imports what it needs into its own module
//! (`use super::shared::{regex_syntax, util};`), so its rules keep writing `super::util::…`.

pub(crate) mod regex_call;
pub(crate) mod regex_syntax;
pub(crate) mod util;
