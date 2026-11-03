//! Lexer tests. Expected codes and positions were taken from the C
//! reference (`libbvnr`, reading the same bytes from memory).

use super::*;

/// Result of a whole run plus the recovery counters.
type Outcome = (Result<(), Error>, u64, u64);

/// Lexes `input` in pieces of `chunk` bytes; returns the outcome together
/// with the recovery counters.
fn run(input: &[u8], chunk: usize, recover: bool, limits: &Limits) -> Outcome {
    let mut val = Validator::new();
    let mut lex = Lexer::new(limits, recover);
    let mut res = Ok(());
    for part in input.chunks(chunk.max(1)) {
        res = lex.feed(&mut val, part);
        if res.is_err() {
            break;
        }
    }
    if res.is_ok() {
        res = lex.finish(&mut val);
    }
    (res, lex.recovery_count(), lex.skipped_bytes())
}

/// Runs `input` whole and in several splits, checks that every split gives
/// the same outcome, and returns it.
fn check_splits(input: &[u8], recover: bool, limits: &Limits) -> Outcome {
    let whole = run(input, usize::MAX, recover, limits);
    for chunk in [1, 2, 3, 5, 7] {
        assert_eq!(run(input, chunk, recover, limits), whole, "chunk {chunk}");
    }
    whole
}

fn err(kind: ErrorKind, line: u64, column: u64, offset: u64) -> Result<(), Error> {
    Err(Error::new(kind, line, column, offset))
}

fn octet_doc(payload_len: usize) -> Vec<u8> {
    let mut d = b".a = \x00".to_vec();
    let len = u16::try_from(payload_len % 65_536)
        .unwrap_or(0)
        .to_le_bytes();
    d.extend_from_slice(&[0x01, len[0], len[1]]);
    d.resize(d.len() + payload_len, 0xab);
    d.extend_from_slice(b"\x00;\n.b = @;");
    d
}

#[test]
fn errors_match_reference_in_every_split() {
    use ErrorKind::*;
    let limits = Limits::default();
    let cases: &[(&[u8], Result<(), Error>)] = &[
        (b".a = 1;\n.b = @;", err(UnexpectedInputByte, 2, 6, 13)),
        (b".a = \"x\xff\";", err(InvalidUtf8Byte, 1, 8, 7)),
        (b"#!bovnar 1.1\n.s = \"\\x41\\u{1F600}\";", Ok(())),
        (b"#!bovnar 1.x\n", err(InvalidSpecVersion, 1, 13, 12)),
        (
            b".a = \x00\x01\x02\x00ab\x00;\n.c = @;",
            err(UnexpectedInputByte, 2, 6, 19),
        ),
        (b".a = \x00\x05", err(OctetStreamOutOfSync, 1, 6, 6)),
        (b".a = [1, 2;", err(GotIncompleteBvnrStream, 1, 11, 10)),
        (
            b".z = 0; # bad \xff byte\n.q = 9;",
            err(InvalidUtf8Byte, 1, 15, 14),
        ),
        (
            b".s = {.a = 1 }; .b = 2;",
            err(UnexpectedInputByte, 1, 14, 13),
        ),
        (
            b".a = 1;\r\n\t.b = 2",
            err(GotIncompleteBvnrStream, 2, 10, 16),
        ),
    ];
    for (input, expected) in cases {
        let (res, recoveries, skipped) = check_splits(input, false, &limits);
        assert_eq!(&res, expected, "{}", input.escape_ascii());
        assert_eq!((recoveries, skipped), (0, 0));
    }
}

#[test]
fn recovery_matches_reference_in_every_split() {
    use ErrorKind::*;
    let limits = Limits::default();
    let cases: &[(&[u8], Outcome)] = &[
        (b".a = 1;\n.b = @;", (Ok(()), 1, 1)),
        (b".a = \"x\xff\";", (Ok(()), 1, 2)),
        (
            b"#!bovnar 1.x\n",
            (err(GotIncompleteBvnrStream, 2, 0, 13), 1, 0),
        ),
        (b".a = [1, 2;", (Ok(()), 1, 0)),
        (b".a = 1; .b = @; .c = 2;", (Ok(()), 1, 1)),
        (b".z = 0; # bad \xff byte\n.q = 9;", (Ok(()), 1, 1)),
        (b".s = {.a = 1 }; .b = 2;", (Ok(()), 1, 1)),
        (b".a = \x00\x05", (err(OctetStreamOutOfSync, 1, 6, 6), 0, 0)),
    ];
    for (input, expected) in cases {
        let got = check_splits(input, true, &limits);
        assert_eq!(got, *expected, "{}", input.escape_ascii());
    }
}

#[test]
fn octet_streams_are_resumable() {
    let limits = Limits::default();
    // 65536 is written as length 0.
    for len in [1, 300, 65_536] {
        let doc = octet_doc(len);
        let at = u64::try_from(doc.len()).unwrap_or(0) - 2;
        let (res, ..) = check_splits(&doc, false, &limits);
        assert_eq!(res, err(ErrorKind::UnexpectedInputByte, 2, 6, at));
        // Truncated inside the payload: the octet reader's pull fails at EOF.
        let cut = doc.get(..10).unwrap_or(&[]);
        let (res, ..) = check_splits(cut, false, &limits);
        assert_eq!(res, err(ErrorKind::ReadCompleteChunkFailed, 1, 6, 10));
    }
}

#[test]
fn input_cap_refuses_the_first_byte_over_it() {
    let limits = Limits {
        max_input_bytes: 20,
        ..Limits::default()
    };
    let (res, ..) = check_splits(&b".x = 1;".repeat(4), false, &limits);
    assert_eq!(res, err(ErrorKind::FileTooLong, 1, 20, 20));
    let (res, ..) = check_splits(&b".x = 1;".repeat(2), false, &limits);
    assert_eq!(res, Ok(()));
}

#[test]
fn version_directive() {
    let cases: &[(&[u8], VerDir)] = &[
        (b"!bovnar 1.2", VerDir::Ok(1, 2)),
        (b"!bovnar \t 0.10  ", VerDir::Ok(0, 10)),
        (b"!bovnarish 1.0", VerDir::None),
        (b" comment", VerDir::None),
        (b"!bovnar 01.0", VerDir::Invalid),
        (b"!bovnar 1.", VerDir::Invalid),
        (b"!bovnar 65536.0", VerDir::Invalid),
        (b"!bovnar 1.1 x", VerDir::Invalid),
    ];
    for (text, expected) in cases {
        let got = parse_version_directive(text);
        let same = match (&got, expected) {
            (VerDir::Ok(a, b), VerDir::Ok(c, d)) => (a, b) == (c, d),
            (VerDir::None, VerDir::None) | (VerDir::Invalid, VerDir::Invalid) => true,
            _ => false,
        };
        assert!(same, "{}", text.escape_ascii());
    }
    let mut val = Validator::new();
    let mut lex = Lexer::new(&Limits::default(), false);
    let res = lex.feed(
        &mut val,
        b"\xef\xbb\xbf  #!bovnar 1.1                         \n",
    );
    assert_eq!(res, Ok(()));
    assert_eq!(val.declared_version, Some(SpecVersion::V1_1));
}
