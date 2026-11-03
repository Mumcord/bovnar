//! Port of `python/tests/test_reader.py`.
//!
// Skipped Python tests:
// - test_context_manager: Python `with` and the private `_ptr`; a Rust reader
//   is released when dropped.
// - test_closed_read_raises: a dropped Rust reader cannot be used again (the
//   compiler rejects it), so there is no closed-reader error.
// - test_non_bytes_raises: a Python type error; `read_mem` takes `&[u8]`.
// - test_library_loads_without_attribute_error: checks the ctypes loader of
//   the Python binding; the Rust crate links the library at build time.

use bovnar::{
    DomDoc, Error, ErrorCode, Event, EventData, ReadOptions, Reader, Value, ValueTypeFamily,
    ValueTypeSpec, parse, parse_typed, to_string,
};

const SIMPLE_SCALAR_UINT: &[u8] = b".port = <uint:16> 8080;\n";
const SIMPLE_SCALAR_SINT: &[u8] = b".offset = <sint:32> -42;\n";
const SIMPLE_SCALAR_FLOAT: &[u8] = b".velocity = <float:64,m/s> 9.81;\n";
const SIMPLE_SCALAR_STRING: &[u8] = b".host = \"localhost\";\n";
const UNTYPED_INTEGER: &[u8] = b".count = 1000;\n";
const UNTYPED_FLOAT: &[u8] = b".ratio = 3.14;\n";
const UNTYPED_NEGATIVE: &[u8] = b".delta = -7;\n";
const STRUCT_PAYLOAD: &[u8] =
    b".config = {\n    .host = \"localhost\";\n    .port = <uint:16> 8080;\n};\n";
const NESTED_STRUCT: &[u8] = b".server = {\n    .host = \"example.com\";\n    .limits = {\n        .max_conn = <uint:32> 100;\n        .timeout = <float:64,s> 2.5;\n    };\n};\n";
const ARRAY_1D: &[u8] = b".values = [1, 2, 3];\n";
const ARRAY_2D: &[u8] = b".matrix = [1, 2, 3]/[4, 5, 6];\n";
const NULL_VALUE: &[u8] = b".empty = ;\n";
const NULL_TYPED: &[u8] = b".placeholder = <uint:32> ;\n";
const SPECIAL_FLOATS: &[u8] = b".not_a_number = nan;\n.pos_inf = inf;\n.neg_inf = ninf;\n";
const MULTI_ASSIGNMENT: &[u8] =
    b".name = \"Alice\";\n.age = <uint:8> 30;\n.score = <float:64> 98.5;\n.active = true;\n";
const DEEP_STRUCT: &[u8] = b".a = {\n    .b = {\n        .c = {\n            .d = {\n                .e = 42;\n            };\n        };\n    };\n};\n";
const STRING_CONCAT: &[u8] = b".s = \"hello \" \"world\";\n";
const COMMENT_PAYLOAD: &[u8] =
    b"# top-level comment\n.foo = 42;  # inline comment\n# another comment\n.bar = 7;\n";

/// Reads `data` with default options and hands every verified event to `f`.
fn read_with(data: &[u8], mut f: impl FnMut(&EventData<'_>)) -> Result<(), Error> {
    let mut cb = |d: &EventData<'_>| {
        f(d);
        true
    };
    Reader::new()
        .unwrap()
        .read_mem(data, &ReadOptions::default(), Some(&mut cb), None)
}

fn collect(data: &[u8]) -> Vec<(Event, Vec<u8>)> {
    let mut events = Vec::new();
    read_with(data, |d| events.push((d.event, d.raw.to_vec()))).unwrap();
    events
}

fn event_kinds(data: &[u8]) -> Vec<Event> {
    collect(data).into_iter().map(|(e, _)| e).collect()
}

fn raws_of(data: &[u8], kind: Event) -> Vec<Vec<u8>> {
    collect(data)
        .into_iter()
        .filter(|(e, _)| *e == kind)
        .map(|(_, r)| r)
        .collect()
}

fn keys(data: &[u8]) -> Vec<String> {
    raws_of(data, Event::AssignmentStart)
        .into_iter()
        .map(|r| String::from_utf8(r).unwrap())
        .collect()
}

fn index_of(events: &[Event], kind: Event) -> usize {
    events.iter().position(|e| *e == kind).unwrap()
}

// TestReaderLifecycle

#[test]
fn test_create_and_close() {
    let r = Reader::new().unwrap();
    drop(r);
}

#[test]
fn test_error_properties_before_read() {
    let r = Reader::new().unwrap();
    assert_eq!(r.error_code(), Some(ErrorCode::None));
    assert_eq!(r.error_line(), 0);
    assert_eq!(r.recovery_count(), 0);
}

// TestEventDelivery

#[test]
fn test_stream_begin_fired() {
    assert_eq!(collect(SIMPLE_SCALAR_UINT)[0].0, Event::StreamStart);
}

#[test]
fn test_assignment_start_key() {
    assert!(raws_of(SIMPLE_SCALAR_UINT, Event::AssignmentStart).contains(&b"port".to_vec()));
}

#[test]
fn test_data_event_value() {
    assert!(raws_of(SIMPLE_SCALAR_UINT, Event::Data).contains(&b"8080".to_vec()));
}

#[test]
fn test_type_annotation_family() {
    let families = raws_of(SIMPLE_SCALAR_UINT, Event::TypeAnnotationTypeFamily);
    assert!(families.iter().any(|f| f.starts_with(b"uint")));
}

#[test]
fn test_annotation_events_order() {
    let events = event_kinds(SIMPLE_SCALAR_FLOAT);
    let ann_start = index_of(&events, Event::TypeAnnotationStart);
    let ann_end = index_of(&events, Event::TypeAnnotationEnd);
    assert!(ann_start < ann_end);
    assert!(index_of(&events, Event::Data) > ann_end);
}

#[test]
fn test_struct_events() {
    let events = event_kinds(STRUCT_PAYLOAD);
    assert!(index_of(&events, Event::StructStart) < index_of(&events, Event::StructEnd));
}

#[test]
fn test_array_row_events() {
    let events = event_kinds(ARRAY_1D);
    assert!(events.contains(&Event::ArrayRowStart));
    assert!(events.contains(&Event::ArrayRowEnd));
}

#[test]
fn test_array_2d_events() {
    assert!(event_kinds(ARRAY_2D).contains(&Event::ArrayDimStart));
}

// TestValueTypeFamily

fn type_specs(data: &[u8], kinds: &[Event]) -> Vec<ValueTypeSpec> {
    let mut items = Vec::new();
    read_with(data, |d| {
        if kinds.contains(&d.event) {
            items.push(d.value_type);
        }
    })
    .unwrap();
    items
}

fn annotation_specs(data: &[u8]) -> Vec<ValueTypeSpec> {
    type_specs(
        data,
        &[
            Event::TypeAnnotationTypeFamily,
            Event::TypeAnnotationTypeFamilyParam,
        ],
    )
}

fn families(specs: &[ValueTypeSpec]) -> Vec<ValueTypeFamily> {
    specs
        .iter()
        .filter_map(|s| ValueTypeFamily::from_raw(s.family))
        .collect()
}

#[test]
fn test_uint16_type() {
    assert!(families(&annotation_specs(SIMPLE_SCALAR_UINT)).contains(&ValueTypeFamily::Uint));
}

#[test]
fn test_sint_type() {
    assert!(families(&annotation_specs(SIMPLE_SCALAR_SINT)).contains(&ValueTypeFamily::Sint));
}

#[test]
fn test_float_type() {
    assert!(families(&annotation_specs(SIMPLE_SCALAR_FLOAT)).contains(&ValueTypeFamily::Float));
}

#[test]
fn test_string_type() {
    let specs = annotation_specs(b".host = <utf8> \"localhost\";\n");
    assert!(families(&specs).contains(&ValueTypeFamily::Utf8));
}

fn has_spec(specs: &[ValueTypeSpec], family: ValueTypeFamily, width: Option<u32>) -> bool {
    specs.iter().any(|s| {
        ValueTypeFamily::from_raw(s.family) == Some(family) && width.is_none_or(|w| s.width == w)
    })
}

#[test]
fn test_untyped_integer_synthesises_uint64() {
    let specs = annotation_specs(UNTYPED_INTEGER);
    assert!(has_spec(&specs, ValueTypeFamily::Uint, Some(64)));
}

#[test]
fn test_untyped_float_synthesises_float64() {
    let specs = annotation_specs(UNTYPED_FLOAT);
    assert!(has_spec(&specs, ValueTypeFamily::Float, Some(64)));
}

#[test]
fn test_negative_integer_synthesises_sint64() {
    let specs = annotation_specs(UNTYPED_NEGATIVE);
    assert!(has_spec(&specs, ValueTypeFamily::Sint, None));
}

// TestIterMem

#[test]
fn test_yields_event_payloads() {
    let events = Reader::new()
        .unwrap()
        .iter_mem(MULTI_ASSIGNMENT, true, 0)
        .unwrap();
    assert!(!events.is_empty());
}

#[test]
fn test_text_property() {
    let events = Reader::new()
        .unwrap()
        .iter_mem(SIMPLE_SCALAR_STRING, true, 0)
        .unwrap();
    let keys: Vec<String> = events
        .iter()
        .filter(|e| e.event == Event::AssignmentStart)
        .map(|e| e.text())
        .collect();
    assert!(keys.contains(&"host".to_string()));
}

#[test]
fn test_assignment_key_found() {
    let events = Reader::new()
        .unwrap()
        .iter_mem(MULTI_ASSIGNMENT, true, 0)
        .unwrap();
    let keys: Vec<String> = events
        .iter()
        .filter(|e| e.event == Event::AssignmentStart)
        .map(|e| e.text())
        .collect();
    assert!(keys.contains(&"name".to_string()));
    assert!(keys.contains(&"age".to_string()));
}

// TestCallbackAbort

#[test]
fn test_false_return_raises() {
    let mut aborting = |_: &EventData<'_>| false;
    let err = Reader::new()
        .unwrap()
        .read_mem(
            b".x = 1;",
            &ReadOptions::default(),
            Some(&mut aborting),
            None,
        )
        .unwrap_err();
    assert!(matches!(err, Error::Parse { .. }), "{err:?}");
    assert_eq!(err.code(), Some(ErrorCode::ScannerCallbackFailed));
}

/// Python's exception raised in the callback is a panic here: it must reach
/// the caller of `read_mem`, not unwind into C.
#[test]
fn test_exception_in_callback_raises() {
    let payload = std::panic::catch_unwind(|| {
        let mut crashing = |_: &EventData<'_>| -> bool { panic!("boom") };
        Reader::new().unwrap().read_mem(
            b".x = 1;",
            &ReadOptions::default(),
            Some(&mut crashing),
            None,
        )
    })
    .unwrap_err();
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"boom"));
}

// TestReadFile

/// The assignment keys `read_reader` delivers for the file at `path`.
fn keys_of_file(path: &std::path::Path) -> Result<Vec<String>, Error> {
    let file = std::fs::File::open(path).unwrap();
    let mut keys = Vec::new();
    let mut cb = |d: &EventData<'_>| {
        if d.event == Event::AssignmentStart {
            keys.push(String::from_utf8(d.raw.to_vec()).unwrap());
        }
        true
    };
    Reader::new()
        .unwrap()
        .read_reader(file, &ReadOptions::default(), Some(&mut cb), None)?;
    Ok(keys)
}

#[test]
fn test_read_file_by_path() {
    // Python's read_file(path) is read_reader over the opened File.
    let path = std::env::temp_dir().join(format!("bovnar-test-reader-{}.bvnr", std::process::id()));
    std::fs::write(&path, SIMPLE_SCALAR_UINT).unwrap();
    let keys = keys_of_file(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(keys.unwrap().contains(&"port".to_string()));
}

#[test]
fn test_read_fd() {
    // Python reads an fd opened on a temporary file; Rust reads the File.
    let path =
        std::env::temp_dir().join(format!("bovnar-test-reader-fd-{}.bvnr", std::process::id()));
    std::fs::write(&path, SIMPLE_SCALAR_UINT).unwrap();
    let keys = keys_of_file(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(keys.unwrap().contains(&"port".to_string()));
}

// TestNestedStructures

#[test]
fn test_nested_struct_keys() {
    let keys = keys(NESTED_STRUCT);
    for k in ["server", "host", "limits", "max_conn", "timeout"] {
        assert!(keys.contains(&k.to_string()), "{k}");
    }
}

#[test]
fn test_deep_nesting() {
    let structs = event_kinds(DEEP_STRUCT)
        .into_iter()
        .filter(|e| *e == Event::StructStart)
        .count();
    assert_eq!(structs, 4);
}

#[test]
fn test_reader_nesting_matches_cli_limit() {
    // The Reader uses the 255-level hard cap of the CLI, dom_parse and the
    // stream module, not the bare reader default of 64; 256 is rejected.
    fn depth_ok(src: String) -> bool {
        match read_with(src.as_bytes(), |_| {}) {
            Ok(()) => true,
            Err(Error::Parse { .. }) => false,
            Err(e) => panic!("unexpected error {e:?}"),
        }
    }
    let structs = |d: usize| format!(".a = {}1{};", "{.b = ".repeat(d), ";}".repeat(d));
    let arrays = |d: usize| format!(".a = {}1{};", "[".repeat(d), "]".repeat(d));
    for builder in [&structs as &dyn Fn(usize) -> String, &arrays] {
        assert!(depth_ok(builder(65)));
        assert!(depth_ok(builder(255)));
        assert!(!depth_ok(builder(256)));
    }
}

#[test]
fn test_array_element_count() {
    assert_eq!(raws_of(ARRAY_1D, Event::Data).len(), 3);
}

// TestNullValues

#[test]
fn test_null_value_empty_raw() {
    assert!(raws_of(NULL_VALUE, Event::Data).contains(&Vec::new()));
}

#[test]
fn test_typed_null() {
    let specs = type_specs(NULL_TYPED, &[Event::TypeAnnotationTypeFamilyParam]);
    assert!(families(&specs).contains(&ValueTypeFamily::Uint));
}

// TestSpecialFloats

#[test]
fn test_nan_infinity_parsed() {
    let values: Vec<String> = raws_of(SPECIAL_FLOATS, Event::Data)
        .into_iter()
        .filter(|v| !v.is_empty())
        .map(|v| String::from_utf8(v).unwrap())
        .collect();
    for want in ["nan", "inf", "ninf"] {
        assert!(values.contains(&want.to_string()), "{want}");
    }
}

// TestComments

#[test]
fn test_comments_ignored() {
    assert_eq!(keys(COMMENT_PAYLOAD), ["foo", "bar"]);
}

// TestStringFeatures

fn data_texts(data: &[u8]) -> Vec<String> {
    raws_of(data, Event::Data)
        .into_iter()
        .map(|v| String::from_utf8(v).unwrap())
        .collect()
}

#[test]
fn test_string_value() {
    assert!(data_texts(SIMPLE_SCALAR_STRING).contains(&"localhost".to_string()));
}

#[test]
fn test_string_concat() {
    assert!(data_texts(STRING_CONCAT).contains(&"hello world".to_string()));
}

// TestMaxFileSize

#[test]
fn test_max_file_size_exceeded() {
    let data = b".x = 1; .y = 2; .z = 3; .w = 4; .v = 5; .u = 6; .t = 7;\n";
    let options = ReadOptions {
        max_file_size: 10,
        ..ReadOptions::default()
    };
    let err = Reader::new()
        .unwrap()
        .read_mem(data, &options, None, None)
        .unwrap_err();
    assert!(matches!(err, Error::Parse { .. }), "{err:?}");
    assert_eq!(err.code(), Some(ErrorCode::FileTooLong));
}

#[test]
fn test_max_file_size_not_exceeded() {
    let options = ReadOptions {
        max_file_size: 1024,
        ..ReadOptions::default()
    };
    Reader::new()
        .unwrap()
        .read_mem(SIMPLE_SCALAR_UINT, &options, None, None)
        .unwrap();
}

// TestInvalidArguments

/// An input that fails as reading an invalid fd does.
struct InvalidFd;

impl std::io::Read for InvalidFd {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("Bad file descriptor"))
    }
}

#[test]
fn test_invalid_fd() {
    // Python refuses fd -1 before reading (BovnarArgumentError). Rust has no
    // fd to refuse, so the input fails instead; the read is refused with the
    // code the C library gives an fd it cannot read from.
    let err = Reader::new()
        .unwrap()
        .read_reader(InvalidFd, &ReadOptions::default(), None, None)
        .unwrap_err();
    assert!(matches!(err, Error::Io { .. }), "{err:?}");
    assert_eq!(err.code(), Some(ErrorCode::ReadingFromSourceFd));
}

// TestTypedRoundtripFidelity

fn typed_roundtrip(src: &[u8]) -> String {
    String::from_utf8(to_string(&parse_typed(src).unwrap(), true).unwrap()).unwrap()
}

#[test]
fn test_symbol_and_reference_keep_kind() {
    // A symbol and a plain reference survive a typed round-trip as a symbol /
    // &.path, not downgraded to a quoted string.
    let out = typed_roundtrip(b".target = 1;\n.ref = &.target;\n.sym = tcp;\n");
    assert!(out.contains(".ref = &.target;"), "{out}");
    assert!(out.contains(".sym = tcp;"), "{out}");
    assert!(!out.contains('"'), "{out}");
    assert_eq!(typed_roundtrip(out.as_bytes()), out);
}

#[test]
fn test_reference_indexing_roundtrips_with_directive() {
    // &.m[0][1] is spec 1.1; serialising must prepend #!bovnar 1.1.
    let out = typed_roundtrip(b"#!bovnar 1.1\n.m = [1,2,3]/[4,5,6];\n.cell = &.m[0][1];\n");
    assert!(out.starts_with("#!bovnar 1.1"), "{out}");
    assert!(out.contains("&.m[0][1]"), "{out}");
    assert_eq!(typed_roundtrip(out.as_bytes()), out);
}

#[test]
fn test_untyped_symbol_reference_stay_str() {
    let v = parse(b".sym = tcp;\n.ref = &.sym;\n").unwrap();
    assert_eq!(
        v,
        Value::Struct(vec![
            ("sym".into(), Value::String("tcp".into())),
            ("ref".into(), Value::String(".sym".into())),
        ])
    );
}

#[test]
fn test_loads_is_streaming_not_dom_validation() {
    // parse() is a single-pass streaming parse: duplicate keys, homogeneity
    // and rectangularity are only enforced by the DOM.
    for src in [
        &b".a = 1;\n.a = 2;\n"[..],
        b".m = [[1, 2], [3, 4, 5]];\n",
        b".a = [1, \"two\"];\n",
    ] {
        parse(src).unwrap();
        assert!(
            matches!(DomDoc::parse(src), Err(Error::Parse { .. })),
            "{}",
            String::from_utf8_lossy(src)
        );
    }
}

/// Python kept a callback's payload past the callback and read freed reader
/// memory. In Rust an `EventData` borrows the reader, so keeping it is a
/// compile error; what can be kept is what its accessors return. Those must
/// give, after the reader is gone, the answers they gave inside the callback.
#[test]
fn test_a_kept_payload_outlives_the_reader() {
    use bovnar::{BaseUnit, Exponent, SIPrefix, Unit};

    type Seen = (
        Vec<u8>,
        String,
        String,
        String,
        bool,
        Option<String>,
        Option<(String, String)>,
        Option<String>,
    );
    fn accessors(d: &EventData<'_>) -> Seen {
        let lossy = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        (
            d.raw.to_vec(),
            d.text(),
            lossy(d.frac),
            lossy(d.unit_source),
            d.converted,
            (d.converted && !d.converted_text.is_empty()).then(|| lossy(d.converted_text)),
            d.converted_rational(),
            if d.converted {
                d.converted_in_base(16).unwrap()
            } else {
                None
            },
        )
    }

    let mut kept = Vec::new();
    let mut want = |d: &EventData<'_>| match d.unit_source {
        // 1 m in miles: no finite expansion
        b"m" => Some((
            Unit::si(BaseUnit::Mile, SIPrefix::None, Exponent::LINEAR),
            10,
        )),
        b"k~m" => Some((
            Unit::si(BaseUnit::Meter, SIPrefix::None, Exponent::LINEAR),
            10,
        )),
        _ => None,
    };
    let mut on_event = |d: &EventData<'_>| {
        if d.event == Event::Data && d.token_type != Some(bovnar::TokenType::Identifier) {
            kept.push(accessors(d));
        }
        true
    };
    let mut doc = b"#!bovnar 1.2\n.d = 1 m;\n.k = 5 k~m;\n\
        .t = 2024-01-02T03:04:05.123456Z;\n.u = 5 ucum:mL{total};\n.s = \""
        .to_vec();
    doc.extend_from_slice(&[b'x'; 300]);
    doc.extend_from_slice(b"\";\n");
    {
        let mut r = Reader::new().unwrap();
        r.read_mem_want_unit(
            &doc,
            &ReadOptions {
                want_unit_allow_nonterminating: true,
                ..ReadOptions::default()
            },
            &mut want,
            Some(&mut on_event),
        )
        .unwrap();
    }
    // reuse the freed memory before looking again
    let churn: Vec<Vec<u8>> = (0..2000).map(|_| vec![b'Z'; 4096]).collect();
    drop(churn);

    assert_eq!(
        kept[0].6,
        Some(("125".into(), "201168".into())) // 1 m / 1609.344 m
    );
    assert_eq!(kept[0].5, None);
    assert_eq!(kept[1].5.as_deref(), Some("5000"));
    assert_eq!(kept[1].7.as_deref(), Some("1388"));
    assert_eq!(kept[2].2, "123456");
    assert_eq!(kept[3].3, "ucum:mL{total}");
    assert_eq!(kept[4].0, vec![b'x'; 300]);
}
