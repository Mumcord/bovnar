//! The writer reports what the C writer (`bvnr_write_event`) reports.
//!
//! `tests/writer/events.txt` holds event sequences in the format of
//! `tests/writer/replay.rs`, each with the result of replaying it through the
//! C reference build (UCUM and CF off): the bytes written, or the first
//! error and the event it occurred at. A third column marks a sequence where
//! bovnar-rs deliberately differs and records what it does instead: it counts
//! nested array rows as the readers do, and it refuses events whose output
//! no reader takes (see UPSTREAM_NOTES.md and the README). The fixture is
//! written by `scripts/writer-diff.sh`, which compares the two writers on
//! generated sequences and fails on any other difference.

#[path = "writer/replay.rs"]
mod replay;

const FIXTURE: &str = include_str!("writer/events.txt");

#[test]
fn writer_errors_match_the_c_writer() {
    let mut failures = Vec::new();
    let mut checked = 0usize;
    for line in FIXTURE.lines().filter(|l| !l.is_empty()) {
        let mut fields = line.split('\t');
        let sequence = fields.next().expect("sequence");
        let c = fields.next().expect("C result");
        let expected = fields.next().unwrap_or(c);
        let got = replay::replay(sequence);
        if got != expected {
            failures.push(format!(
                "{sequence}\n  expected: {expected}\n  got:      {got}"
            ));
        }
        checked += 1;
    }
    assert!(checked > 500, "fixture too small: {checked} sequences");
    assert!(
        failures.is_empty(),
        "{} of {checked} sequences differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
