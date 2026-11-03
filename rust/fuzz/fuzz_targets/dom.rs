//! Arbitrary bytes through the DOM: parse, walk every node, convert. Errors
//! are fine; crashes and panics are not.
#![no_main]

use bovnar::{DomDoc, DomNode};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(doc) = DomDoc::parse(data) else {
        return;
    };
    for (key, node) in doc.entries() {
        assert!(doc.get(&key).is_some());
        walk(node, 0);
    }
    let _ = doc.to_value();
    let _ = bovnar::json::to_json(data);
});

fn walk(node: DomNode<'_>, depth: usize) {
    let _ = (
        node.dom_type(),
        node.unit_str(),
        node.as_str(),
        node.as_bytes(),
    );
    let _ = node.as_int_str(10);
    let _ = node.to_value();
    if depth > 300 {
        return;
    }
    for (_, child) in node.entries() {
        walk(child, depth + 1);
    }
    for i in 0..node.len() {
        if let Some(child) = node.at(i) {
            walk(child, depth + 1);
        }
    }
}
