//! Port of `python/tests/test_unit_source.py`: the unit's own spelling
//! (`bvnr_data_t.unit_source`) through the bindings: the reader hands it
//! over, a typed parse keeps it on the `Quantity`, `to_string` writes it back,
//! and the DOM exposes it.
//!
//! `loads`/`loads(typed=True)`/`dumps` are `parse`/`parse_typed`/`to_string`.
//!
//! Skipped:
//! - `test_array_to_bvnr_writes_a_profile_only_unit`: exercises the NumPy
//!   bridge (`array_to_bvnr`), which is not mirrored.
//!
//! In `test_a_spelling_that_disagrees_with_the_unit_is_refused`, the Python
//! check that a non-string `unit_source` (`unit_source=5`) is refused has no
//! runtime counterpart: `Quantity::unit_source` is an `Option<String>`, so
//! the compiler refuses it.

use bovnar::{
    DomDoc, Error, ErrorCode, Event, EventData, Quantity, ReadOptions, Reader, Unit, Value,
    ValueTypeFamily, ValueTypeSpec, parse_typed, to_string,
};

const DOC: &[u8] = b"#!bovnar 1.2\n\
.a = <float:64,ucum:mL{total}> 1.0;\n\
.b = <float:64,ucum:{RBC}/uL> 2.0;\n\
.c = <float:64,k~m> [1.0, 2.0];\n\
.d = <uint:8,km> [1, 2] / [3, 4];\n\
.e = <float:64,udunits:kg m-2 s-1> [1.0, 2.0];\n\
.s = <utf8> \"x\" m;\n\
.n = 7;\n";

fn field<'v>(v: &'v Value, key: &str) -> &'v Value {
    let Value::Struct(entries) = v else {
        panic!("not a struct: {v:?}");
    };
    &entries.iter().find(|(k, _)| k == key).unwrap().1
}

fn field_mut<'v>(v: &'v mut Value, key: &str) -> &'v mut Value {
    let Value::Struct(entries) = v else {
        panic!("not a struct: {v:?}");
    };
    &mut entries.iter_mut().find(|(k, _)| k == key).unwrap().1
}

fn quantity(v: &Value) -> &Quantity {
    match v {
        Value::Quantity(q) => q,
        other => panic!("not a quantity: {other:?}"),
    }
}

fn items(v: &Value) -> &[Value] {
    match v {
        Value::Array(a) => a,
        other => panic!("not an array: {other:?}"),
    }
}

fn items_mut(v: &mut Value) -> &mut Vec<Value> {
    match v {
        Value::Array(a) => a,
        other => panic!("not an array: {other:?}"),
    }
}

fn vt(family: ValueTypeFamily, width: u32) -> ValueTypeSpec {
    ValueTypeSpec {
        family: family as i32,
        width,
        base: 0,
    }
}

fn doc_of(q: Quantity) -> Value {
    Value::Struct(vec![("a".into(), Value::Quantity(Box::new(q)))])
}

fn dumps(v: &Value) -> Result<Vec<u8>, Error> {
    to_string(v, true)
}

#[test]
fn test_reader_event_carries_the_spelling() {
    let mut seen = Vec::new();
    let mut on_event = |d: &EventData<'_>| {
        if d.event == Event::Data {
            seen.push(
                (!d.unit_source.is_empty())
                    .then(|| String::from_utf8_lossy(d.unit_source).into_owned()),
            );
        }
        true
    };
    let mut r = Reader::new().unwrap();
    r.read_mem(
        b"#!bovnar 1.2\n.a = <float:64,ucum:mL{total}> 1.0;.b = 2 km; .c = 3;",
        &ReadOptions::default(),
        Some(&mut on_event),
        None,
    )
    .unwrap();
    assert_eq!(
        seen,
        vec![
            Some("ucum:mL{total}".to_string()),
            Some("km".to_string()),
            None
        ]
    );
}

#[test]
fn test_typed_load_keeps_it_on_the_quantity() {
    let d = parse_typed(DOC).unwrap();
    let a = quantity(field(&d, "a"));
    assert_eq!(a.unit_source.as_deref(), Some("ucum:mL{total}"));
    // `unit` is what it means; the annotation lives only in the spelling.
    assert_eq!(a.unit.to_string(), "m~L");
    let d00 = quantity(&items(&items(field(&d, "d"))[0])[0]);
    assert_eq!(d00.unit_source.as_deref(), Some("km"));
    assert_eq!(quantity(field(&d, "s")).unit_source.as_deref(), Some("m"));
    assert_eq!(quantity(field(&d, "n")).unit_source, None);
}

#[test]
fn test_typed_round_trip_is_byte_identical() {
    // Every spelling here is one the canonical form would have replaced: two
    // UCUM annotations, a compact prefix, a space-multiplied UDUNITS code.
    assert_eq!(dumps(&parse_typed(DOC).unwrap()).unwrap(), DOC);
}

#[test]
fn test_dumps_declares_1_2_only_when_the_spelling_needs_it() {
    let out = dumps(&parse_typed(b".a = <float:64,km> 1.0;").unwrap()).unwrap();
    assert_eq!(out, b".a = <float:64,km> 1.0;\n");
    let out =
        dumps(&parse_typed(b"#!bovnar 1.2\n.a = <float:64,ucum:mm[Hg]> 1.0;").unwrap()).unwrap();
    assert_eq!(out, b"#!bovnar 1.2\n.a = <float:64,ucum:mm[Hg]> 1.0;\n");
}

#[test]
fn test_dumps_writes_a_profile_only_unit() {
    // This failed before spellings were kept: [IU] has no native form, the C
    // writer refuses it without a 1.2 directive, and dumps() never wrote one.
    let src: &[u8] = b"#!bovnar 1.2\n.a = <float:64,ucum:[IU]> 2.0;\n";
    assert_eq!(dumps(&parse_typed(src).unwrap()).unwrap(), src);
    let q = Quantity::new(
        "2.0",
        vt(ValueTypeFamily::Float, 64),
        Unit::parse("ucum:[IU]").unwrap(),
    );
    assert_eq!(dumps(&doc_of(q)).unwrap(), src);
}

#[test]
fn test_an_array_keeps_one_spelling_or_none() {
    let mut d = parse_typed(b".a = <uint:8,km> [1, 2];").unwrap();
    assert_eq!(dumps(&d).unwrap(), b".a = <uint:8,km> [1, 2];\n");
    // Leaves that disagree cannot share an annotation's one spelling.
    let first = quantity(&items(field(&d, "a"))[0]).clone();
    items_mut(field_mut(&mut d, "a"))[1] =
        Value::Quantity(Box::new(Quantity::new("2", first.vtype, first.unit))); // no spelling kept
    assert_eq!(dumps(&d).unwrap(), b".a = <uint:8,k~m> [1, 2];\n");
}

#[test]
fn test_a_spelling_that_disagrees_with_the_unit_is_refused() {
    let mut q = Quantity::new(
        "1.0",
        vt(ValueTypeFamily::Float, 64),
        Unit::parse("m").unwrap(),
    );
    q.unit_source = Some("s".into());
    let e = dumps(&doc_of(q)).unwrap_err();
    assert!(matches!(e, Error::Write { .. }), "{e:?}");
    assert_eq!(e.code(), Some(ErrorCode::UnitMismatch));
}

#[test]
fn test_dom_exposes_it_where_the_unit_cannot_give_it_back() {
    let dom = DomDoc::parse(DOC).unwrap();
    assert_eq!(
        dom.lookup(".a").unwrap().unit_source(),
        Some("ucum:mL{total}")
    );
    // canonical already
    assert_eq!(dom.lookup(".c").unwrap().at(0).unwrap().unit_source(), None);
    assert_eq!(dom.lookup(".n").unwrap().unit_source(), None);
}

#[test]
fn test_dumps_refuses_an_array_whose_units_it_cannot_write() {
    // An array has one annotation and its elements are written bare. Leaves
    // that disagree on the unit used to be written with NO unit -- [1 s, 3 m]
    // came out as [1, 3]. The format refuses that array as well, so there is
    // nothing valid to write; dumps() says so instead of dropping the units.
    let d = parse_typed(b".a = [<float:64,s> 1, <float:64,m> 3];").unwrap();
    assert!(matches!(dumps(&d), Err(Error::Argument(_))));
    // A Quantity beside a plain number is the same loss by another route.
    let mut d = parse_typed(b".a = <float:64,s> [1, 3];").unwrap();
    items_mut(field_mut(&mut d, "a"))[1] = Value::Float(3.0);
    assert!(matches!(dumps(&d), Err(Error::Argument(_))));
    // Units that agree are written under the one annotation, as before.
    let d = parse_typed(b".a = <float:64,s> [1, 3];").unwrap();
    assert_eq!(dumps(&d).unwrap(), b".a = <float:64,s> [1, 3];\n");
}

#[test]
fn test_a_spelling_on_a_unitless_quantity_is_refused() {
    // A spelling is checked against the unit it spells; with no unit there
    // was nothing to check it against and it was dropped in silence.
    let unitless = |src: &str| {
        let mut q = Quantity::new("5", vt(ValueTypeFamily::Uint, 64), Unit::default());
        q.unit_source = Some(src.into());
        doc_of(q)
    };
    assert!(matches!(dumps(&unitless("m")), Err(Error::Argument(_))));
    // A spelling that itself means "no unit" says nothing false.
    assert_eq!(to_string(&unitless("no_unit"), false).unwrap(), b".a=5;");
}
