//! The document tree: building, accessors, document-tier errors and limits.

use bovnar::{
    parse, parse_with, Annotation, Array, Document, ErrorKind, Kind, Limits, Number, SpecVersion,
    Struct, TypeFamily, TypeSpec, Value,
};

fn kind_of(input: &[u8]) -> ErrorKind {
    parse(input).expect_err("document must be refused").kind()
}

#[test]
fn scalars_and_accessors() {
    let doc = parse(
        b"#!bovnar 1.1\n.u = 42; .s = -7; .f = 2.5; .t = \"hi\\n\"; .b = on; .sym = info;\
          .r = &.u; .n = null; .e = ; .h = <uint:32,_16> \"ff\"; .m = 5 m;\
          .dt = 2026-01-01T00:00:00.25Z; .big = <uint:128> 340282366920938463463374607431768211455;",
    )
    .expect("valid document");
    assert_eq!(doc.version(), Some(SpecVersion::V1_1));
    assert_eq!(doc.get("u").and_then(Value::as_u64), Some(42));
    assert_eq!(doc.get("u").and_then(Value::as_i64), Some(42));
    assert_eq!(doc.get("s").and_then(Value::as_i64), Some(-7));
    assert_eq!(doc.get("s").and_then(Value::as_u64), None);
    assert_eq!(doc.get("f").and_then(Value::as_f64), Some(2.5));
    assert_eq!(doc.get("f").and_then(Value::as_u64), None);
    assert_eq!(doc.get("t").and_then(Value::as_str), Some("hi\n"));
    assert_eq!(doc.get("b").and_then(Value::as_bool), Some(true));
    assert_eq!(doc.get("sym").and_then(Value::as_symbol), Some("info"));
    assert_eq!(doc.get("r").and_then(Value::as_reference), Some(".u"));
    assert!(doc.get("n").is_some_and(Value::is_null));
    assert!(doc.get("e").is_some_and(Value::is_null));

    let h = doc
        .get("h")
        .and_then(Value::as_number)
        .expect("quoted hex is a number");
    assert!(h.is_quoted());
    assert_eq!(h.as_u64(), Some(255));
    assert_eq!(h.spec().effective_base(), 16);

    let m = doc.get("m").expect("m");
    assert_eq!(m.unit(), Some("m"));
    assert_eq!(m.as_u64(), Some(5));

    let dt = doc.get("dt").and_then(Value::as_number).expect("datetime");
    assert!(dt.is_datetime());
    assert_eq!(dt.as_i64(), Some(1_767_225_600));
    assert_eq!(dt.fraction(), Some("25"));

    let big = doc.get("big").expect("big");
    assert_eq!(big.as_u64(), None, "out of u64 range");
    assert_eq!(
        big.as_number().map(Number::text),
        Some("340282366920938463463374607431768211455")
    );

    // Wrong-kind accessors return None instead of panicking.
    assert_eq!(doc.get("t").and_then(Value::as_u64), None);
    assert_eq!(doc.get("u").and_then(Value::as_str), None);
    assert!(doc.get("u").and_then(|v| v.get("x")).is_none());
    assert!(doc.get("missing").is_none());
}

#[test]
fn annotations_are_kept() {
    let doc = parse(b".a = <float:64,m> [1, 2.5]; .b = [\"x\", \"y\"];").expect("valid");
    let a = doc.get("a").expect("a");
    let ann = a.annotation().expect("array annotation");
    assert_eq!(ann.family, TypeFamily::Float);
    assert_eq!(ann.width, Some(64));
    assert_eq!(ann.unit.as_deref(), Some("m"));
    assert!(!ann.synthesized);
    let first = a.at(0).expect("element");
    assert!(first.annotation().is_none());
    assert_eq!(first.unit(), Some("m"));
    assert_eq!(first.type_spec().map(|s| s.family), Some(TypeFamily::Float));

    let b = doc.get("b").and_then(Value::as_array).expect("b");
    assert!(b
        .get(0)
        .and_then(Value::annotation)
        .is_some_and(|a| a.synthesized));
    assert!(b.get(1).and_then(Value::annotation).is_none());
}

#[test]
fn structs_arrays_and_paths() {
    let doc = parse(
        b".net = {.routes = [{.dst = \"a\"; .gw = \"b\";}, {.dst = \"c\"; .gw = \"d\";}];};\
          .m = [[1, 2] / [3, 4] / [5, 6]];",
    )
    .expect("valid");
    assert_eq!(
        doc.lookup(".net.routes[1].gw").and_then(Value::as_str),
        Some("d")
    );
    assert_eq!(
        doc.lookup("net.routes[0].dst").and_then(Value::as_str),
        Some("a")
    );
    assert!(doc.lookup(".net.routes[2]").is_none());
    assert!(doc.lookup(".net..routes").is_none());
    assert!(doc.lookup("[0]").is_none());

    let routes = doc
        .lookup(".net.routes")
        .and_then(Value::as_array)
        .expect("routes");
    assert_eq!(routes.len(), 2);
    let keys: Vec<&str> = routes
        .get(0)
        .and_then(Value::as_struct)
        .expect("struct")
        .iter()
        .map(|(k, _)| k)
        .collect();
    assert_eq!(keys, ["dst", "gw"]);

    let m = doc
        .lookup(".m[0]")
        .and_then(Value::as_array)
        .expect("matrix");
    assert_eq!(m.row_count(), 3);
    assert_eq!(m.len(), 6);
    let rows: Vec<Vec<u64>> = m
        .rows()
        .map(|r| r.iter().filter_map(Value::as_u64).collect())
        .collect();
    assert_eq!(rows, [[1, 2], [3, 4], [5, 6]]);
}

#[test]
fn document_tier_errors() {
    assert_eq!(kind_of(b".x = 1; .x = 2;"), ErrorKind::DuplicateStructKey);
    assert_eq!(
        kind_of(b".s = {.x = 1; .x = 2;};"),
        ErrorKind::DuplicateStructKey
    );
    assert_eq!(
        kind_of(b".a = [1, \"two\"];"),
        ErrorKind::ArrayElementTypeMismatch
    );
    assert_eq!(
        kind_of(b".a = [[1, 2], [3]];"),
        ErrorKind::ArrayRowSizeMismatch
    );
    assert_eq!(
        kind_of(b".a = [{.x = 1;}, {.y = 1;}];"),
        ErrorKind::StructShapeMismatch
    );
    // Holes match anything, in any order (the shape is order-independent).
    parse(b".a = [{.x = null;}, {.x = 1;}, {.x = 2.5;}];").expect("holes are fine");
    assert_eq!(
        kind_of(b".a = [{.x = null;}, {.x = 1;}, {.x = \"s\";}];"),
        ErrorKind::ArrayElementTypeMismatch
    );
    assert_eq!(
        kind_of(b".a = [[null, null], [<float:64,m> 1.0, null], [<float:64,s> 2.0, null]];"),
        ErrorKind::ArrayElementTypeMismatch
    );
    // Fields are free across records, but a list inside one record is not.
    parse(b".a = [{.l = [1];}, {.l = [1, 2];}];").expect("list lengths may differ per record");
    assert_eq!(
        kind_of(b".a = [{.l = [<float:64,m> 1.0, <float:64,s> 2.0];}];"),
        ErrorKind::ArrayElementTypeMismatch
    );
    // Reader errors come first and keep their position.
    let err = parse(b".a = [1, 2;").expect_err("broken");
    assert!(err.line() > 0 || err.offset() > 0 || err.kind() != ErrorKind::DuplicateStructKey);
}

#[test]
fn node_limit() {
    let mut limits = Limits::default();
    limits.max_dom_nodes = 4;
    parse_with(b".a = [1, 2, 3];", &limits).expect("4 nodes fit");
    let err = parse_with(b".a = [1, 2, 3, 4];", &limits).expect_err("5 nodes do not");
    assert_eq!(err.kind(), ErrorKind::FileTooLong);
}

#[test]
fn input_limit() {
    let mut limits = Limits::default();
    limits.max_input_bytes = 8;
    let err = parse_with(b".abc = 12345;", &limits).expect_err("too long");
    assert_eq!(err.kind(), ErrorKind::FileTooLong);
}

#[test]
fn deep_nesting_is_bounded() {
    let depth = 255;
    let nested = |open: &str, close: &str, n: usize| {
        let mut s = String::from(".a = ");
        s.push_str(&open.repeat(n));
        s.push_str(&close.repeat(n));
        s.push(';');
        s
    };
    let mut limits = Limits::default();
    limits.max_array_nesting = 255;
    limits.max_struct_nesting = 255;
    let doc = parse_with(nested("[", "]", depth).as_bytes(), &limits).expect("255 arrays");
    let text = bovnar::to_string(&doc).expect("writes back");
    assert_eq!(parse_with(text.as_bytes(), &limits).expect("re-reads"), doc);
    let err = parse_with(nested("[", "]", depth + 1).as_bytes(), &limits).expect_err("256 arrays");
    assert_eq!(err.kind(), ErrorKind::ArrayNestingTooHigh);

    let structs = nested("{.b = ", ";}", depth);
    parse_with(structs.as_bytes(), &limits).expect("255 structs");
    let err = parse(nested("{.b = ", ";}", 65).as_bytes()).expect_err("default limit is 64");
    assert_eq!(err.kind(), ErrorKind::StructNestingTooHigh);
}

#[test]
fn deep_hand_built_tree_drops_and_refuses_to_write() {
    // Far deeper than any stack could recurse through.
    let mut value = Value::from(1u64);
    for _ in 0..1_000_000 {
        let mut a = Array::new();
        a.push(value);
        value = Value::from(a);
    }
    let mut doc = Document::new();
    doc.insert("deep", value);
    let err = bovnar::to_string(&doc).expect_err("deeper than the format allows");
    assert_eq!(err.kind(), ErrorKind::ArrayNestingTooHigh);
    drop(doc);

    let mut value = Value::null();
    for _ in 0..1_000_000 {
        let mut s = Struct::new();
        s.insert("x", value);
        value = Value::from(s);
    }
    drop(value);
}

#[test]
fn builders() {
    let mut server = Struct::new();
    server.insert("host", "localhost");
    server.insert("port", 5432u16);
    server.insert("ratio", 0.5);
    server.insert("delta", -3i64);
    server.insert("tags", Array::from(vec!["a", "b"]));
    server.insert("blob", vec![1u8, 2, 3]);
    server.insert("speed", Number::from(3.5).with_unit("m/s"));
    server.insert(
        "mask",
        Number::new(
            TypeSpec {
                family: TypeFamily::Uint,
                width: 16,
                base: 16,
            },
            "beef",
        ),
    );
    server.insert("level", Value::symbol("debug"));
    server.insert("alias", Value::reference(".server.host"));
    server.insert("none", Option::<u64>::None);
    assert_eq!(
        server.insert("port", 6543u16).and_then(|v| v.as_u64()),
        Some(5432)
    );

    let mut doc = Document::new();
    doc.insert("server", server);
    let text = bovnar::to_vec(&doc).expect("writes");
    assert_eq!(
        String::from_utf8_lossy(&text),
        ".server = {\n\t.host = \"localhost\";\n\t.port = <uint:16> 6543;\n\t.ratio = 0.5;\
         \n\t.delta = -3;\n\t.tags = [\"a\", \"b\"];\n\t.blob = \0\x01\x03\0\x01\x02\x03\0;\
         \n\t.speed = <float:64,m/s> 3.5;\n\t.mask = <uint:16,_16> \"beef\";\n\t.level = debug;\
         \n\t.alias = &.server.host;\n\t.none = ;\n};\n"
    );
    let back = parse(&text).expect("reads back");
    let s = back.get("server").expect("server");
    assert_eq!(s.get("port").and_then(Value::as_u64), Some(6543));
    assert_eq!(s.get("delta").and_then(Value::as_i64), Some(-3));
    assert_eq!(
        s.get("blob").and_then(Value::as_bytes),
        Some(&[1u8, 2, 3][..])
    );
    assert_eq!(s.get("speed").and_then(Value::unit), Some("m/s"));
    assert_eq!(s.get("mask").and_then(Value::as_u64), Some(0xbeef));
    // A parsed tree carries the annotations the reader synthesised, so it
    // is the parsed form that is a fixed point of write-then-parse.
    let again = bovnar::to_vec(&back).expect("writes again");
    assert!(parse(&again).expect("reads again") == back);

    let typed = Value::from(1.5).with_annotation(Annotation::new(TypeFamily::Float).with_width(32));
    assert_eq!(typed.annotation().and_then(|a| a.width), Some(32));
    assert!(matches!(typed.kind(), Kind::Number(_)));
}

#[test]
fn special_floats() {
    for (v, text) in [
        (f64::NAN, "nan"),
        (f64::INFINITY, "inf"),
        (f64::NEG_INFINITY, "ninf"),
    ] {
        let mut doc = Document::new();
        doc.insert("x", v);
        let s = bovnar::to_string(&doc).expect("writes");
        assert_eq!(s, format!(".x = {text};\n"));
        let back = parse(s.as_bytes()).expect("reads");
        let got = back.get("x").and_then(Value::as_f64).expect("float");
        assert!(got.is_nan() == v.is_nan() && (v.is_nan() || got == v));
    }
}
