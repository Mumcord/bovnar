//! Stress tests: hostile nesting depth and inputs at the size cap. The
//! reader, the tree and the writer must neither overflow the stack nor
//! slow down more than linearly, and every limit must hold.

use std::time::{Duration, Instant};

use bovnar::{parse_with, read_all, to_vec, ErrorKind, Limits};

const MIB: usize = 1024 * 1024;

fn reader_error(input: &[u8], limits: &Limits) -> Option<ErrorKind> {
    read_all(input, limits, |_| {}).err().map(|e| e.kind())
}

fn tree_error(input: &[u8], limits: &Limits) -> Option<ErrorKind> {
    parse_with(input, limits).err().map(|e| e.kind())
}

/// `.a = {.a = {... ;};};` nested `depth` deep.
fn nested_structs(depth: usize) -> Vec<u8> {
    let mut doc = b".a = ".to_vec();
    doc.extend(b"{.a = ".repeat(depth));
    doc.extend(b"1;");
    doc.extend(b"};".repeat(depth));
    doc
}

/// `.a = [[[... 1 ...]]];` nested `depth` deep.
fn nested_arrays(depth: usize) -> Vec<u8> {
    let mut doc = b".a = ".to_vec();
    doc.extend(b"[".repeat(depth));
    doc.push(b'1');
    doc.extend(b"]".repeat(depth));
    doc.push(b';');
    doc
}

#[test]
fn a_million_open_structs_hit_the_nesting_limit() {
    let mut doc = b".a = ".to_vec();
    doc.extend(b"{.a = ".repeat(MIB));
    let limits = Limits::default();
    assert_eq!(
        reader_error(&doc, &limits),
        Some(ErrorKind::StructNestingTooHigh)
    );
    assert_eq!(
        tree_error(&doc, &limits),
        Some(ErrorKind::StructNestingTooHigh)
    );
}

#[test]
fn a_million_open_arrays_hit_the_nesting_limit() {
    let doc = nested_arrays(MIB);
    let limits = Limits::default();
    assert_eq!(
        reader_error(&doc, &limits),
        Some(ErrorKind::ArrayNestingTooHigh)
    );
    assert_eq!(
        tree_error(&doc, &limits),
        Some(ErrorKind::ArrayNestingTooHigh)
    );
}

#[test]
fn nesting_limits_are_exact() {
    let mut limits = Limits::default();
    limits.max_struct_nesting = 10;
    limits.max_array_nesting = 10;
    assert_eq!(reader_error(&nested_structs(10), &limits), None);
    assert_eq!(
        reader_error(&nested_structs(11), &limits),
        Some(ErrorKind::StructNestingTooHigh)
    );
    assert_eq!(reader_error(&nested_arrays(10), &limits), None);
    assert_eq!(
        reader_error(&nested_arrays(11), &limits),
        Some(ErrorKind::ArrayNestingTooHigh)
    );
}

#[test]
fn the_deepest_allowed_tree_reads_writes_and_drops() {
    let mut limits = Limits::default();
    limits.max_struct_nesting = u8::MAX;
    limits.max_array_nesting = u8::MAX;
    for doc in [nested_structs(255), nested_arrays(255)] {
        let tree = parse_with(&doc, &limits).expect("deepest allowed tree");
        let written = to_vec(&tree).expect("writing the deepest tree");
        let again = parse_with(&written, &limits).expect("reading it back");
        assert!(tree == again);
        drop((tree, again));
    }
    // Structs inside arrays inside structs: both counters at the limit.
    let mut doc = b".a = ".to_vec();
    for _ in 0..255 {
        doc.extend(b"[{.a = ");
    }
    doc.push(b'1');
    for _ in 0..255 {
        doc.extend(b";}]");
    }
    doc.push(b';');
    assert_eq!(tree_error(&doc, &limits), None);
}

#[test]
fn deep_unit_groups_in_an_annotation_are_bounded() {
    for open in [b"(", b"[", b"{"] {
        let mut doc = b".a = <float:64,".to_vec();
        doc.extend(open.repeat(100_000));
        doc.extend(b"m> 1;");
        assert!(reader_error(&doc, &Limits::default()).is_some());
    }
}

#[test]
fn the_input_cap_is_exact() {
    let mut limits = Limits::default();
    limits.max_input_bytes = 4096;
    let fill = |len: usize| {
        let mut doc = b".a = 1;".to_vec();
        doc.resize(len, b' ');
        doc
    };
    assert_eq!(reader_error(&fill(4096), &limits), None);
    assert_eq!(
        reader_error(&fill(4097), &limits),
        Some(ErrorKind::FileTooLong)
    );
    assert_eq!(
        tree_error(&fill(4097), &limits),
        Some(ErrorKind::FileTooLong)
    );
}

/// Builds a document of exactly `len` bytes: `head`, then `item(i)` for
/// i = 0, 1, ... while it fits, then `tail`, padded with spaces.
fn build(head: &[u8], item: impl Fn(usize) -> Vec<u8>, tail: &[u8], len: usize) -> Vec<u8> {
    let mut doc = head.to_vec();
    for i in 0.. {
        let next = item(i);
        if doc.len() + next.len() + tail.len() > len {
            break;
        }
        doc.extend(next);
    }
    doc.extend_from_slice(tail);
    doc.resize(len, b' ');
    doc
}

/// The shapes a large document can take, each valid for the tree too:
/// many assignments, one wide array, long strings with escapes, comments,
/// many octet chunks, many small structs.
fn shapes(len: usize) -> Vec<(&'static str, Vec<u8>)> {
    let long_string = format!("\"{}\"", "ab\\\"c\\\\d\\n ".repeat(400));
    vec![
        (
            "assignments",
            build(b"", |i| format!(".k{i} = {i};\n").into_bytes(), b"", len),
        ),
        (
            "array",
            build(
                b".a = [0",
                |i| format!(", {}", i % 1000).into_bytes(),
                b"];",
                len,
            ),
        ),
        (
            "strings",
            build(
                b"",
                |i| format!(".s{i} = {long_string};\n").into_bytes(),
                b"",
                len,
            ),
        ),
        (
            "comments",
            build(b"", |_| b"# a comment line\n".to_vec(), b".a = 1;", len),
        ),
        (
            "octets",
            build(
                b".o = \0",
                |_| vec![1, 8, 0, 1, 2, 3, 4, 5, 6, 7, 8],
                b"\0;",
                len,
            ),
        ),
        (
            "structs",
            build(
                b"",
                |i| format!(".s{i} = {{.x = 1; .y = \"z\";}};\n").into_bytes(),
                b"",
                len,
            ),
        ),
    ]
}

/// Generous limits so the stress documents measure time, not limits.
fn roomy() -> Limits {
    let mut limits = Limits::default();
    limits.max_dom_nodes = u64::MAX;
    limits
}

fn time_reader(doc: &[u8]) -> Duration {
    let start = Instant::now();
    let mut events = 0u64;
    read_all(doc, &roomy(), |_| events += 1).expect("stress document");
    assert!(events > 0);
    start.elapsed()
}

fn time_tree(doc: &[u8]) -> Duration {
    let start = Instant::now();
    let tree = parse_with(doc, &roomy()).expect("stress document");
    drop(tree);
    start.elapsed()
}

/// The best of three runs, to keep scheduler noise out of the baseline.
fn best(time: fn(&[u8]) -> Duration, doc: &[u8]) -> Duration {
    time(doc).min(time(doc)).min(time(doc))
}

#[test]
fn sixteen_mebibytes_read_in_linear_time() {
    let small = shapes(MIB);
    let large = shapes(16 * MIB);
    for ((name, small), (_, large)) in small.iter().zip(&large) {
        assert_eq!(large.len(), 16 * MIB, "{name}");
        for (api, time) in [
            ("reader", time_reader as fn(&[u8]) -> Duration),
            ("tree", time_tree),
        ] {
            let (t_small, t_large) = (best(time, small), time(large));
            // 16 times the input; allow up to 48 times the time (plus a
            // fixed margin for tiny timings) before calling it superlinear.
            let bound = t_small * 48 + Duration::from_millis(50);
            assert!(
                t_large <= bound,
                "{name}/{api}: 1 MiB in {t_small:?}, 16 MiB in {t_large:?}"
            );
        }
    }
    // The default limits take a 16 MiB document and refuse one byte more.
    let mut doc = large[3].1.clone();
    assert_eq!(reader_error(&doc, &Limits::default()), None);
    doc.push(b' ');
    assert_eq!(
        reader_error(&doc, &Limits::default()),
        Some(ErrorKind::FileTooLong)
    );
}
