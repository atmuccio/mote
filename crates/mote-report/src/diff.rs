//! Unified diffs of a file before and after `mote fix` (the format `git apply` and `patch -p1`
//! read).

use std::fmt::Write as _;

/// Lines of context around each change.
pub const CONTEXT: usize = 3;

/// One line of a text, terminator included (the last line may have none).
fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Equal,
    Delete,
    Insert,
}

/// Edits (deleted plus inserted lines) beyond which [`script`] gives up looking for the
/// shortest script and replaces the differing middle whole (its trace grows with the square).
const MAX_EDITS: usize = 2000;

/// An edit script turning `a` into `b`: (op, index into `a` for `Equal` and `Delete`, into `b`
/// for `Insert`). The shortest one (Myers' algorithm) up to [`MAX_EDITS`] edits.
fn script(a: &[&str], b: &[&str]) -> Vec<(Op, usize)> {
    // Common prefix and suffix first: fixes touch few lines of large files.
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let (am, bm) = (&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix]);
    let mut ops: Vec<(Op, usize)> = (0..prefix).map(|i| (Op::Equal, i)).collect();
    let middle = myers(am, bm).unwrap_or_else(|| {
        let mut m: Vec<(Op, usize)> = (0..am.len()).map(|i| (Op::Delete, i)).collect();
        m.extend((0..bm.len()).map(|i| (Op::Insert, i)));
        m
    });
    ops.extend(middle.into_iter().map(|(op, i)| (op, i + prefix)));
    let sa = a.len() - suffix;
    ops.extend((0..suffix).map(|i| (Op::Equal, sa + i)));
    ops
}

/// Myers' shortest edit script, or `None` past [`MAX_EDITS`] edits.
fn myers(a: &[&str], b: &[&str]) -> Option<Vec<(Op, usize)>> {
    let (n, m) = (a.len() as isize, b.len() as isize);
    let max = (n + m) as usize;
    if max == 0 {
        return Some(Vec::new());
    }
    // `v[k + max]`: the furthest x reached on diagonal k; `trace[d]`: v[-d..=d] after round d.
    let mut v = vec![0isize; 2 * max + 2];
    let at = |k: isize| (k + max as isize) as usize;
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let mut done = false;
    for d in 0..=(max.min(MAX_EDITS) as isize) {
        let mut k = -d;
        while k <= d {
            let mut x = if d == 0 {
                0
            } else if k == -d || (k != d && v[at(k - 1)] < v[at(k + 1)]) {
                v[at(k + 1)]
            } else {
                v[at(k - 1)] + 1
            };
            let mut y = x - k;
            while x < n && y < m && a[x as usize] == b[y as usize] {
                x += 1;
                y += 1;
            }
            v[at(k)] = x;
            if x >= n && y >= m {
                done = true;
            }
            k += 2;
        }
        trace.push(v[at(-d)..=at(d)].to_vec());
        if done {
            break;
        }
    }
    if !done {
        return None;
    }
    // Back from (n, m): each round d ends with a snake from one edit after round d - 1.
    let mut out: Vec<(Op, usize)> = Vec::new();
    let (mut x, mut y) = (n, m);
    for d in (1..trace.len() as isize).rev() {
        let prev = &trace[(d - 1) as usize];
        let pv = |k: isize| prev[(k + d - 1) as usize];
        let k = x - y;
        let down = k == -d || (k != d && pv(k - 1) < pv(k + 1));
        let prev_k = if down { k + 1 } else { k - 1 };
        let prev_x = pv(prev_k);
        let prev_y = prev_x - prev_k;
        let (step_x, step_y) = if down {
            (prev_x, prev_y + 1)
        } else {
            (prev_x + 1, prev_y)
        };
        while x > step_x && y > step_y {
            x -= 1;
            y -= 1;
            out.push((Op::Equal, x as usize));
        }
        if down {
            out.push((Op::Insert, prev_y as usize));
        } else {
            out.push((Op::Delete, prev_x as usize));
        }
        (x, y) = (prev_x, prev_y);
    }
    while x > 0 && y > 0 {
        x -= 1;
        y -= 1;
        out.push((Op::Equal, x as usize));
    }
    out.reverse();
    Some(out)
}

/// Write one line of a hunk: `prefix` and the line, with a `\ No newline at end of file` marker
/// when it has no terminator.
fn push_line(out: &mut String, prefix: char, line: &str) {
    out.push(prefix);
    out.push_str(line);
    if !line.ends_with('\n') {
        out.push_str("\n\\ No newline at end of file\n");
    }
}

/// A unified diff of `old` and `new`, the file `path` (`--- a/path`, `+++ b/path`), with
/// [`CONTEXT`] lines of context; empty when the texts are equal. Lines keep their terminators
/// (`\r\n` included), as `git apply` expects.
pub fn unified(path: &str, old: &str, new: &str) -> String {
    if old == new {
        return String::new();
    }
    let (a, b) = (lines(old), lines(new));
    let ops = script(&a, &b);
    // Position in `a` and `b` before each op.
    let mut pos = Vec::with_capacity(ops.len());
    let (mut ia, mut ib) = (0usize, 0usize);
    for (op, _) in &ops {
        pos.push((ia, ib));
        match op {
            Op::Equal => {
                ia += 1;
                ib += 1;
            }
            Op::Delete => ia += 1,
            Op::Insert => ib += 1,
        }
    }
    let changed: Vec<usize> = (0..ops.len()).filter(|&i| ops[i].0 != Op::Equal).collect();
    let mut out = String::new();
    let _ = writeln!(out, "--- a/{path}");
    let _ = writeln!(out, "+++ b/{path}");
    let mut i = 0;
    while i < changed.len() {
        // A hunk: changes no more than 2 * CONTEXT equal lines apart.
        let first = changed[i];
        let mut last = first;
        while i + 1 < changed.len() && changed[i + 1] - last <= 2 * CONTEXT + 1 {
            i += 1;
            last = changed[i];
        }
        i += 1;
        let start = first.saturating_sub(CONTEXT);
        let end = (last + CONTEXT + 1).min(ops.len());
        let (sa, sb) = pos[start];
        let (mut na, mut nb) = (0usize, 0usize);
        let mut body = String::new();
        for &(op, idx) in &ops[start..end] {
            match op {
                Op::Equal => {
                    push_line(&mut body, ' ', a[idx]);
                    na += 1;
                    nb += 1;
                }
                Op::Delete => {
                    push_line(&mut body, '-', a[idx]);
                    na += 1;
                }
                Op::Insert => {
                    push_line(&mut body, '+', b[idx]);
                    nb += 1;
                }
            }
        }
        // An empty side starts at the line before (as diff and git write it).
        let range = |s: usize, n: usize| {
            let s = if n == 0 { s } else { s + 1 };
            if n == 1 {
                format!("{s}")
            } else {
                format!("{s},{n}")
            }
        };
        let _ = writeln!(out, "@@ -{} +{} @@", range(sa, na), range(sb, nb));
        out.push_str(&body);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Apply a unified diff produced by [`unified`] to `old` (test helper).
    fn patch(old: &str, diff: &str) -> String {
        let a = lines(old);
        let mut out = String::new();
        let mut ia = 0usize;
        let mut it = diff.split_inclusive('\n').skip(2).peekable();
        while let Some(h) = it.next() {
            let h = h.trim_end();
            assert!(h.starts_with("@@ -"), "{h}");
            let old_range = h[4..].split(' ').next().unwrap();
            let (s, n) = match old_range.split_once(',') {
                Some((s, n)) => (s.parse::<usize>().unwrap(), n.parse::<usize>().unwrap()),
                None => (old_range.parse::<usize>().unwrap(), 1),
            };
            let s0 = if n == 0 { s } else { s - 1 };
            while ia < s0 {
                out.push_str(a[ia]);
                ia += 1;
            }
            while let Some(l) = it.peek() {
                if l.starts_with("@@") {
                    break;
                }
                let l = it.next().unwrap();
                let (tag, rest) = l.split_at(1);
                let mut text = rest.to_string();
                if it.peek().is_some_and(|n| n.starts_with('\\')) {
                    it.next();
                    text.pop();
                }
                match tag {
                    " " => {
                        out.push_str(&text);
                        ia += 1;
                    }
                    "-" => ia += 1,
                    "+" => out.push_str(&text),
                    _ => panic!("bad line {l:?}"),
                }
            }
        }
        while ia < a.len() {
            out.push_str(a[ia]);
            ia += 1;
        }
        out
    }

    #[test]
    fn diffs_apply_back() {
        let cases = [
            ("a\nb\nc\n", "a\nc\n"),
            ("a\nb\nc\n", "a\nb\nc\nd\n"),
            ("x\n", ""),
            ("", "x\n"),
            ("a\nb", "a\nc"),
            ("a\r\nb\r\nc\r\n", "a\r\nc\r\n"),
            (
                "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n",
                "1\n2x\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14x\n15\n",
            ),
            (
                "import a;\nimport b;\nclass A {}\n",
                "import b;\nclass A {}\n",
            ),
            ("p\nq\nr\ns\n", "q\np\ns\nr\n"),
        ];
        for (old, new) in cases {
            let d = unified("f.java", old, new);
            assert_eq!(patch(old, &d), new, "{old:?} -> {new:?}\n{d}");
        }
        assert_eq!(unified("f", "same\n", "same\n"), "");
    }

    #[test]
    fn random_texts_round_trip() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        for _ in 0..500 {
            let (la, lb) = (next(12), next(12));
            let mut text = |len: u64| -> String {
                (0..len)
                    .map(|_| ["a\n", "b\n", "c\n", "d\r\n", "e"][next(5) as usize])
                    .collect()
            };
            let (old, new) = (text(la), text(lb));
            let d = unified("x", &old, &new);
            assert_eq!(patch(&old, &d), new, "{old:?} -> {new:?}\n{d}");
        }
    }

    #[test]
    fn hunk_headers_and_context() {
        let old: String = (1..=20).map(|i| format!("l{i}\n")).collect();
        let new = old.replace("l10\n", "");
        let d = unified("src/A.java", &old, &new);
        assert_eq!(
            d,
            "--- a/src/A.java\n+++ b/src/A.java\n@@ -7,7 +7,6 @@\n l7\n l8\n l9\n-l10\n l11\n l12\n l13\n"
        );
        let new = old.replace("l2\n", "l2x\n").replace("l19\n", "");
        let d = unified("A", &old, &new);
        assert_eq!(d.matches("@@ -").count(), 2, "{d}");
    }
}
