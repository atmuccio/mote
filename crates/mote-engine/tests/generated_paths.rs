//! Files known to be tool-written by their path (Yarn Plug'n'Play) are skipped as generated.

mod common;

use common::{opts, scanner, write};
use mote_engine::analyze::SkipReason;
use mote_engine::scan::SourceSpec;

#[test]
fn yarn_plug_n_play_files_are_skipped_as_generated() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    write(root, ".yarn/sdks/typescript/lib/tsserver.fk", "LOUD\n");
    write(root, "web/.yarn/releases/yarn.fk", "LOUD\n");
    write(root, "src/a.fk", "LOUD\n");
    let (s, _) = scanner(root, "");
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    let skipped: Vec<(&str, Option<SkipReason>)> = r
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.skip_reason))
        .collect();
    assert_eq!(
        skipped,
        vec![
            (
                ".yarn/sdks/typescript/lib/tsserver.fk",
                Some(SkipReason::Generated)
            ),
            ("src/a.fk", None),
            ("web/.yarn/releases/yarn.fk", Some(SkipReason::Generated)),
        ]
    );
    assert_eq!(r.findings.len(), 1);

    // `skip_generated: false` analyzes them like any file.
    let (s, _) = scanner(root, "skip_generated: false\n");
    let r = s.scan(&opts(SourceSpec::Directory, None)).unwrap();
    assert_eq!(r.findings.len(), 3);
}
