//! Reader options against the C reference. Every expectation below was
//! captured from `libbvnr` reading the same bytes from memory with
//! `bvnr_read_flags_t.strict_version` (s), `text_only` (t) and
//! `continue_on_error` (c) set as listed: error name, line, column and
//! `bvnr_reader_get_recovery_count`.

use bovnar::{ErrorKind, ReadOptions, Reader, SpecVersion};

/// Input, flags, error name, line, column, recovery count.
type Case = (&'static [u8], &'static str, &'static str, u64, u64, u64);

#[rustfmt::skip]
const CASES: &[Case] = &[
    (b"#!bovnar 1.3\x0a.a = 1;", "", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0a.a = 1;", "s", "unsupported_spec_version", 1, 13, 0),
    (b"#!bovnar 1.3\x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0a.a = 1;", "sc", "none", 0, 0, 1),
    (b"#!bovnar 1.3\x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0a.a = 1;", "stc", "none", 0, 0, 1),
    (b"#!bovnar 2.0\x0a.a = 1;", "", "none", 0, 0, 0),
    (b"#!bovnar 2.0\x0a.a = 1;", "s", "unsupported_spec_version", 1, 13, 0),
    (b"#!bovnar 2.0\x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"#!bovnar 2.0\x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"#!bovnar 2.0\x0a.a = 1;", "sc", "none", 0, 0, 1),
    (b"#!bovnar 2.0\x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"#!bovnar 2.0\x0a.a = 1;", "stc", "none", 0, 0, 1),
    (b"#!bovnar 9.9", "", "none", 0, 0, 0),
    (b"#!bovnar 9.9", "s", "unsupported_spec_version", 1, 12, 0),
    (b"#!bovnar 9.9", "t", "none", 0, 0, 0),
    (b"#!bovnar 9.9", "c", "none", 0, 0, 0),
    (b"#!bovnar 9.9", "sc", "unsupported_spec_version", 1, 12, 0),
    (b"#!bovnar 9.9", "tc", "none", 0, 0, 0),
    (b"#!bovnar 9.9", "stc", "unsupported_spec_version", 1, 12, 0),
    (b"#!bovnar 1.2\x0a.a = 1;", "", "none", 0, 0, 0),
    (b"#!bovnar 1.2\x0a.a = 1;", "s", "none", 0, 0, 0),
    (b"#!bovnar 1.2\x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"#!bovnar 1.2\x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"#!bovnar 1.2\x0a.a = 1;", "sc", "none", 0, 0, 0),
    (b"#!bovnar 1.2\x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"#!bovnar 1.2\x0a.a = 1;", "stc", "none", 0, 0, 0),
    (b"#!bovnar 0.9\x0a.a = 1;", "", "none", 0, 0, 0),
    (b"#!bovnar 0.9\x0a.a = 1;", "s", "unsupported_spec_version", 1, 13, 0),
    (b"#!bovnar 0.9\x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"#!bovnar 0.9\x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"#!bovnar 0.9\x0a.a = 1;", "sc", "none", 0, 0, 1),
    (b"#!bovnar 0.9\x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"#!bovnar 0.9\x0a.a = 1;", "stc", "none", 0, 0, 1),
    (b"#!bovnar 1.x\x0a.a = 1;", "", "invalid_spec_version", 1, 13, 0),
    (b"#!bovnar 1.x\x0a.a = 1;", "s", "invalid_spec_version", 1, 13, 0),
    (b"#!bovnar 1.x\x0a.a = 1;", "t", "invalid_spec_version", 1, 13, 0),
    (b"#!bovnar 1.x\x0a.a = 1;", "c", "none", 0, 0, 1),
    (b"#!bovnar 1.x\x0a.a = 1;", "sc", "none", 0, 0, 1),
    (b"#!bovnar 1.x\x0a.a = 1;", "tc", "none", 0, 0, 1),
    (b"#!bovnar 1.x\x0a.a = 1;", "stc", "none", 0, 0, 1),
    (b".a = \x00\x01\x02\x00hi\x00;", "", "none", 0, 0, 0),
    (b".a = \x00\x01\x02\x00hi\x00;", "s", "none", 0, 0, 0),
    (b".a = \x00\x01\x02\x00hi\x00;", "t", "octet_stream_forbidden", 1, 6, 0),
    (b".a = \x00\x01\x02\x00hi\x00;", "c", "none", 0, 0, 0),
    (b".a = \x00\x01\x02\x00hi\x00;", "sc", "none", 0, 0, 0),
    (b".a = \x00\x01\x02\x00hi\x00;", "tc", "none", 0, 0, 1),
    (b".a = \x00\x01\x02\x00hi\x00;", "stc", "none", 0, 0, 1),
    (b".x = 1;\x0a  .a = \x00\x01\x02\x00hi\x00; .b = 2;", "", "none", 0, 0, 0),
    (b".x = 1;\x0a  .a = \x00\x01\x02\x00hi\x00; .b = 2;", "s", "none", 0, 0, 0),
    (b".x = 1;\x0a  .a = \x00\x01\x02\x00hi\x00; .b = 2;", "t", "octet_stream_forbidden", 2, 8, 0),
    (b".x = 1;\x0a  .a = \x00\x01\x02\x00hi\x00; .b = 2;", "c", "none", 0, 0, 0),
    (b".x = 1;\x0a  .a = \x00\x01\x02\x00hi\x00; .b = 2;", "sc", "none", 0, 0, 0),
    (b".x = 1;\x0a  .a = \x00\x01\x02\x00hi\x00; .b = 2;", "tc", "none", 0, 0, 1),
    (b".x = 1;\x0a  .a = \x00\x01\x02\x00hi\x00; .b = 2;", "stc", "none", 0, 0, 1),
    (b".a = [1, \x00\x01\x01\x00z\x00];", "", "none", 0, 0, 0),
    (b".a = [1, \x00\x01\x01\x00z\x00];", "s", "none", 0, 0, 0),
    (b".a = [1, \x00\x01\x01\x00z\x00];", "t", "octet_stream_forbidden", 1, 10, 0),
    (b".a = [1, \x00\x01\x01\x00z\x00];", "c", "none", 0, 0, 0),
    (b".a = [1, \x00\x01\x01\x00z\x00];", "sc", "none", 0, 0, 0),
    (b".a = [1, \x00\x01\x01\x00z\x00];", "tc", "none", 0, 0, 1),
    (b".a = [1, \x00\x01\x01\x00z\x00];", "stc", "none", 0, 0, 1),
    (b".a = \x22\x00\x22;", "", "unexpected_input_byte", 1, 7, 0),
    (b".a = \x22\x00\x22;", "s", "unexpected_input_byte", 1, 7, 0),
    (b".a = \x22\x00\x22;", "t", "unexpected_input_byte", 1, 7, 0),
    (b".a = \x22\x00\x22;", "c", "none", 0, 0, 1),
    (b".a = \x22\x00\x22;", "sc", "none", 0, 0, 1),
    (b".a = \x22\x00\x22;", "tc", "none", 0, 0, 1),
    (b".a = \x22\x00\x22;", "stc", "none", 0, 0, 1),
    (b"#!bovnar 1.3\x0a.a = \x00\x00;", "", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0a.a = \x00\x00;", "s", "unsupported_spec_version", 1, 13, 0),
    (b"#!bovnar 1.3\x0a.a = \x00\x00;", "t", "octet_stream_forbidden", 2, 6, 0),
    (b"#!bovnar 1.3\x0a.a = \x00\x00;", "c", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0a.a = \x00\x00;", "sc", "none", 0, 0, 1),
    (b"#!bovnar 1.3\x0a.a = \x00\x00;", "tc", "none", 0, 0, 1),
    (b"#!bovnar 1.3\x0a.a = \x00\x00;", "stc", "none", 0, 0, 2),
    (b"\xef\xbb\xbf#!bovnar 1.4\x0a.a = 1;", "", "none", 0, 0, 0),
    (b"\xef\xbb\xbf#!bovnar 1.4\x0a.a = 1;", "s", "unsupported_spec_version", 1, 14, 0),
    (b"\xef\xbb\xbf#!bovnar 1.4\x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"\xef\xbb\xbf#!bovnar 1.4\x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"\xef\xbb\xbf#!bovnar 1.4\x0a.a = 1;", "sc", "none", 0, 0, 1),
    (b"\xef\xbb\xbf#!bovnar 1.4\x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"\xef\xbb\xbf#!bovnar 1.4\x0a.a = 1;", "stc", "none", 0, 0, 1),
    (b"#!bovnar 1.3\x0d\x0a.a = 1;", "", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0d\x0a.a = 1;", "s", "unsupported_spec_version", 1, 13, 0),
    (b"#!bovnar 1.3\x0d\x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0d\x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0d\x0a.a = 1;", "sc", "none", 0, 0, 1),
    (b"#!bovnar 1.3\x0d\x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"#!bovnar 1.3\x0d\x0a.a = 1;", "stc", "none", 0, 0, 1),
    (b"#!bovnar 1.3 \x0a.a = 1;", "", "none", 0, 0, 0),
    (b"#!bovnar 1.3 \x0a.a = 1;", "s", "unsupported_spec_version", 1, 14, 0),
    (b"#!bovnar 1.3 \x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"#!bovnar 1.3 \x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"#!bovnar 1.3 \x0a.a = 1;", "sc", "none", 0, 0, 1),
    (b"#!bovnar 1.3 \x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"#!bovnar 1.3 \x0a.a = 1;", "stc", "none", 0, 0, 1),
    (b"# comment\x0a#!bovnar 1.9\x0a.a = 1;", "", "none", 0, 0, 0),
    (b"# comment\x0a#!bovnar 1.9\x0a.a = 1;", "s", "none", 0, 0, 0),
    (b"# comment\x0a#!bovnar 1.9\x0a.a = 1;", "t", "none", 0, 0, 0),
    (b"# comment\x0a#!bovnar 1.9\x0a.a = 1;", "c", "none", 0, 0, 0),
    (b"# comment\x0a#!bovnar 1.9\x0a.a = 1;", "sc", "none", 0, 0, 0),
    (b"# comment\x0a#!bovnar 1.9\x0a.a = 1;", "tc", "none", 0, 0, 0),
    (b"# comment\x0a#!bovnar 1.9\x0a.a = 1;", "stc", "none", 0, 0, 0),
    (b".a = { .b = \x00\x00; };", "", "none", 0, 0, 0),
    (b".a = { .b = \x00\x00; };", "s", "none", 0, 0, 0),
    (b".a = { .b = \x00\x00; };", "t", "octet_stream_forbidden", 1, 13, 0),
    (b".a = { .b = \x00\x00; };", "c", "none", 0, 0, 0),
    (b".a = { .b = \x00\x00; };", "sc", "none", 0, 0, 0),
    (b".a = { .b = \x00\x00; };", "tc", "none", 0, 0, 1),
    (b".a = { .b = \x00\x00; };", "stc", "none", 0, 0, 1),
    (b".a = <utf8> \x00\x00;", "", "none", 0, 0, 0),
    (b".a = <utf8> \x00\x00;", "s", "none", 0, 0, 0),
    (b".a = <utf8> \x00\x00;", "t", "octet_stream_forbidden", 1, 13, 0),
    (b".a = <utf8> \x00\x00;", "c", "none", 0, 0, 0),
    (b".a = <utf8> \x00\x00;", "sc", "none", 0, 0, 0),
    (b".a = <utf8> \x00\x00;", "tc", "none", 0, 0, 1),
    (b".a = <utf8> \x00\x00;", "stc", "none", 0, 0, 1),
    (b"#!bovnar 1.1\x0a.a = \x00\x01\x01\x00q\x00; .b = \x00\x00;", "", "none", 0, 0, 0),
    (b"#!bovnar 1.1\x0a.a = \x00\x01\x01\x00q\x00; .b = \x00\x00;", "s", "none", 0, 0, 0),
    (b"#!bovnar 1.1\x0a.a = \x00\x01\x01\x00q\x00; .b = \x00\x00;", "t", "octet_stream_forbidden", 2, 6, 0),
    (b"#!bovnar 1.1\x0a.a = \x00\x01\x01\x00q\x00; .b = \x00\x00;", "c", "none", 0, 0, 0),
    (b"#!bovnar 1.1\x0a.a = \x00\x01\x01\x00q\x00; .b = \x00\x00;", "sc", "none", 0, 0, 0),
    (b"#!bovnar 1.1\x0a.a = \x00\x01\x01\x00q\x00; .b = \x00\x00;", "tc", "none", 0, 0, 2),
    (b"#!bovnar 1.1\x0a.a = \x00\x01\x01\x00q\x00; .b = \x00\x00;", "stc", "none", 0, 0, 2),
];

/// Reads `input` in `chunk`-byte pieces; returns error name, line, column
/// and recovery count in the C harness's format.
fn run(input: &[u8], options: &ReadOptions, chunk: usize) -> (String, u64, u64, u64) {
    let mut reader = Reader::with_options(options);
    let mut result = Ok(());
    for part in input.chunks(chunk) {
        result = reader.feed(part);
        while reader.next_event().is_some() {}
        if result.is_err() {
            break;
        }
    }
    if result.is_ok() {
        result = reader.finish();
        while reader.next_event().is_some() {}
    }
    match result {
        Ok(()) => ("none".into(), 0, 0, reader.recovery_count()),
        Err(e) => (
            e.kind().as_str().into(),
            e.line(),
            e.column(),
            reader.recovery_count(),
        ),
    }
}

#[test]
fn flags_match_the_c_reference() {
    for &(input, flags, err, line, column, recovered) in CASES {
        let options = ReadOptions::new()
            .strict_version(flags.contains('s'))
            .text_only(flags.contains('t'))
            .recovery(flags.contains('c'));
        let want = (err.to_string(), line, column, recovered);
        for chunk in [input.len().max(1), 1, 3] {
            assert_eq!(
                run(input, &options, chunk),
                want,
                "{:?} flags {flags:?} chunk {chunk}",
                String::from_utf8_lossy(input)
            );
        }
    }
}

#[test]
fn unsupported_version_is_recorded_when_not_strict() {
    let mut reader = Reader::with_options(&ReadOptions::new());
    reader.feed(b"#!bovnar 1.3\n.a = 1;").expect("accepted");
    reader.finish().expect("complete");
    assert_eq!(
        reader.declared_version(),
        Some(SpecVersion { major: 1, minor: 3 })
    );
}

#[test]
fn dom_honours_the_policies() {
    let strict = ReadOptions::new().strict_version(true);
    let err = bovnar::parse_with_options(b"#!bovnar 2.0\n.a = 1;", &strict).unwrap_err();
    assert_eq!(
        (err.kind(), err.line(), err.column()),
        (ErrorKind::UnsupportedSpecVersion, 1, 13)
    );
    assert!(bovnar::parse_with_options(b"#!bovnar 1.2\n.a = 1;", &strict).is_ok());

    let text = ReadOptions::new().text_only(true);
    let err = bovnar::parse_with_options(b".x = 1;\n.a = \x00\x00;", &text).unwrap_err();
    assert_eq!(
        (err.kind(), err.line(), err.column()),
        (ErrorKind::OctetStreamForbidden, 2, 6)
    );
    let doc = bovnar::parse_with_options(b".a = \"text\";", &text).expect("no octets");
    assert_eq!(doc.get("a").and_then(|v| v.as_str()), Some("text"));
}

#[test]
fn dom_refuses_recovery() {
    let err =
        bovnar::parse_with_options(b".a = 1;", &ReadOptions::new().recovery(true)).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidArgument);
}
