//! Differential tester for the event writer: replays generated event
//! sequences through the C reference's `bvnr_write_event` (via the
//! `writer_replay` harness, see `diff/c/writer_replay.c`) and through
//! `bovnar::Writer`, and compares the output or the first error, sequence by
//! sequence. The C reference is the oracle.
//!
//! ```text
//! bovnar-writer-diff --c <writer_replay> [--count 50000] [--seed 1]
//!                    [--show 3] [--input FILE] [--capture FILE]
//! ```
//!
//! Sequences come from `--input` (one per line; anything after a tab is
//! ignored, so a fixture can be replayed) or are generated: a few fixed
//! ones, then `--count` generated from `--seed`. Where the writers disagree
//! the difference is judged (see [`Verdict`]): bovnar-rs keeps two fixes
//! over the C writer, its row counting and refusing events whose output no
//! reader takes, and everything else counts as a divergence and fails the
//! run. `--capture` writes the fixture `tests/writer_error_parity.rs` checks.

#[path = "../../tests/writer/replay.rs"]
mod replay;

use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, ExitCode, Stdio};
use std::{env, fs, thread};

/// xorshift64*: deterministic and dependency-free.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % (n as u64)).unwrap_or(0)
    }
    fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Percent-encodes a field: everything outside printable ASCII, and the
/// format's own separators.
fn enc(bytes: &[u8]) -> String {
    let mut out = String::new();
    for &b in bytes {
        if (0x21..0x7f).contains(&b) && b != b'%' && b != b':' {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

const KEYS: &[&[u8]] = &[
    b"a",
    b"_x",
    b"a1",
    b"key_2",
    b"1a",
    b"",
    b"a-b",
    b"a.b",
    "\u{e4}".as_bytes(),
    b"a b",
    b"true",
    b"null",
    b"nan",
    b"a\n",
    b"ab;",
    "\u{e9}t\u{e9}".as_bytes(),
    b"a\0",
    b"uint",
    b"-",
    b"_",
    b"a=b",
    b"\xff",
    b"\"a\"",
    b"A",
    b"a/b",
    b"<a>",
    b"0",
    b"on",
    b"a\t",
    "\u{1F600}".as_bytes(),
];
const FAMILIES: &[&str] = &[
    "uint",
    "sint",
    "float",
    "float_fix",
    "float_dec",
    "utf8",
    "bool",
    "datetime",
    "plain",
];
const WIDTHS: &[&str] = &[
    "", "", "", "0", "1", "7", "8", "16", "32", "48", "64", "65", "96", "128", "256", "512",
    "1024", "1056", "1088", "2048", "4096", "4097", "99999",
];
const BASES: &[&str] = &[
    "", "", "", "0", "1", "2", "8", "10", "16", "36", "62", "63", "64", "85", "86",
];
const QS: &[&str] = &["", "", "", "0", "1", "8", "31", "63", "64", "200"];
const EPOCHS: &[&str] = &["", "", "", "unix", "tai", "gps", "mjd", "bogus", "UNIX"];
const UNITS: &[&[u8]] = &[
    b"",
    b"",
    b"",
    b"m",
    b"k~m",
    b"km",
    b"m/s",
    b"m*s^-1",
    b"no_unit",
    b"kg*m^2",
    b"xyz",
    b"ucum:mm[Hg]",
    b"cf:K",
    b"$EUR",
    b"$XXQ",
    b"m>",
    b"1",
    b"m^0",
    b"m^101",
    b"s",
    b"Ki~B",
    b"K",
    b"m m",
    b"m;",
    b"mmHg",
    b"%",
    b"m\0",
    b"\xff",
    b"deg",
    b"rad",
];
const NUMBERS: &[&[u8]] = &[
    b"0",
    b"5",
    b"-5",
    b"300",
    b"255",
    b"256",
    b"-129",
    b"1.5",
    b"-1.5",
    b"1e3",
    b"1E-3",
    b"nan",
    b"inf",
    b"ninf",
    b"-inf",
    b"ff",
    b"FF",
    b"18F",
    b"3e",
    b"310EE",
    b"0x10",
    b"1_000",
    b"",
    b"+5",
    b"99999999999999999999999",
    b"18446744073709551616",
    b"-9223372036854775809",
    b"-0",
    b"1.",
    b".5",
    b"..",
    b"1e",
    b"1e99999",
    b"2024-01-01T00:00:00Z",
    b"2024-01-01T00:00:00.5Z",
    b"1700000000",
    b"-1700000000",
    b"5 m",
    b"12",
    b"101",
    b"z",
    b"Zz",
    b"+",
    b"-",
    b"0.1",
    b"1e-400",
    b"1.7976931348623157e309",
    b"9999-12-31T23:59:59Z",
    b"10000-01-01T00:00:00Z",
    b"2024-13-01T00:00:00Z",
    b"7",
    b"1/2",
];
const STRINGS: &[&[u8]] = &[
    b"",
    b"hi",
    b"a\"b",
    b"back\\slash",
    b"\x01",
    b"\x7f",
    b"\xff",
    "\u{e9}".as_bytes(),
    b"12",
    b"ff",
    b"1.5",
    b"-3",
    b"tab\there",
    b"line\nbreak",
    b"\x0b\x0c\r",
    b"nan",
    b"zz",
    b"300",
    b"1e3",
    b"\xc3",
    b"\xed\xa0\x80",
    b"0",
    b"-0",
    b"+1",
    b"18F",
    b" 5",
    b"5 ",
    b"10.5e2",
];
const SYMBOLS: &[&[u8]] = &[
    b"foo",
    b"true",
    b"1abc",
    b"",
    b"a b",
    b"a;b",
    b"nan",
    b"null",
    b"_s",
    b"a.b",
    b"foo-bar",
    "\u{e9}".as_bytes(),
    b"a=b",
    b"inf",
    b"on",
    b"x\0",
    b"uint",
];
const REFS: &[&[u8]] = &[
    b"a",
    b"1",
    b"",
    b"a.b",
    b"a b",
    b"0",
    b"a.1",
    b"_x",
    b"a..b",
    b".a",
    b"a.",
    b"&a",
    b"a;",
    b"999999999999999999999",
    b"-1",
    "\u{e4}".as_bytes(),
];
const BOOLS: &[&[u8]] = &[b"true", b"false", b"on", b"off", b"yes", b"", b"TRUE", b"1"];
const FRACS: &[&[u8]] = &[b"", b"", b"", b"", b"5", b"123", b"x", b"000"];
const OCTETS: &[usize] = &[1, 3, 0, 65536, 65537, 200];

struct Gen<'r> {
    rng: &'r mut Rng,
    events: Vec<String>,
}

impl Gen<'_> {
    fn push(&mut self, ev: impl Into<String>) {
        self.events.push(ev.into());
    }

    fn key(&mut self) {
        let key = if self.rng.one_in(8) {
            *self.rng.pick(KEYS)
        } else {
            *self.rng.pick(&[&b"a"[..], b"b", b"value", b"x_1"])
        };
        if self.rng.one_in(60) {
            let n = *self.rng.pick(&[255usize, 256, 300]);
            self.push(format!("k:{}", "k".repeat(n)));
        } else {
            self.push(format!("k:{}", enc(key)));
        }
    }

    fn annotation(&mut self, family: Option<&str>) {
        let fam = family.unwrap_or_else(|| {
            // `plain` is not an annotation the reader takes; keep it rare.
            let fam = *self.rng.pick(FAMILIES);
            if fam == "plain" && !self.rng.one_in(4) {
                "uint"
            } else {
                fam
            }
        });
        let sparse = self.rng.one_in(2);
        let field = |pool: &[&'static str], rng: &mut Rng| -> &'static str {
            if sparse && !rng.one_in(4) {
                ""
            } else {
                rng.pick(pool)
            }
        };
        let width = field(WIDTHS, self.rng);
        // The C events carry base, Q and epoch in one field, as the reader
        // fills it: at most one of them per annotation.
        // Mostly the parameter the family takes, so that sequences get past
        // the annotation to the values.
        let (mut base, mut q, mut epoch) = ("", "", "");
        let slot = if self.rng.one_in(6) {
            self.rng.below(3)
        } else {
            match fam {
                "float_fix" => 1,
                "datetime" => 2,
                _ => 0,
            }
        };
        match slot {
            0 => base = field(BASES, self.rng),
            1 => q = field(QS, self.rng),
            _ => epoch = field(EPOCHS, self.rng),
        }
        let unit = if sparse && !self.rng.one_in(3) {
            &b""[..]
        } else {
            *self.rng.pick(UNITS)
        };
        let unit = if self.rng.one_in(200) {
            "m".repeat(250).into_bytes()
        } else {
            unit.to_vec()
        };
        self.push(format!("a:{fam}:{width}:{base}:{q}:{epoch}:{}", enc(&unit)));
    }

    fn data(&mut self, in_array: bool) {
        let r = self.rng.below(20);
        let (kind, text): (&str, Vec<u8>) = match r {
            0..=7 => (
                if in_array && !self.rng.one_in(4) {
                    "anum"
                } else {
                    "num"
                },
                self.rng.pick(NUMBERS).to_vec(),
            ),
            8..=11 => {
                let text = if self.rng.one_in(150) {
                    vec![b'x'; *self.rng.pick(&[65535usize, 65536, 70000])]
                } else {
                    self.rng.pick(STRINGS).to_vec()
                };
                (
                    if in_array && !self.rng.one_in(4) {
                        "astr"
                    } else {
                        "str"
                    },
                    text,
                )
            }
            12..=13 => ("sym", self.rng.pick(SYMBOLS).to_vec()),
            14..=15 => ("ref", self.rng.pick(REFS).to_vec()),
            16..=17 => ("bool", self.rng.pick(BOOLS).to_vec()),
            _ => ("null", Vec::new()),
        };
        let frac = if kind.ends_with("num") {
            *self.rng.pick(FRACS)
        } else {
            &b""[..]
        };
        let unit = if self.rng.one_in(4) {
            *self.rng.pick(UNITS)
        } else {
            &b""[..]
        };
        self.push(format!(
            "d:{kind}:{}:{}:{}",
            enc(&text),
            enc(frac),
            enc(unit)
        ));
    }

    /// One value: maybe annotated scalar, array, struct or octet stream.
    fn value(&mut self, depth: usize, in_array: bool) {
        let r = if depth > 3 {
            self.rng.below(8)
        } else {
            self.rng.below(14)
        };
        match r {
            0..=7 => {
                if self.rng.one_in(2) {
                    self.annotation(None);
                }
                self.data(in_array);
            }
            8..=10 => {
                if self.rng.one_in(3) {
                    self.annotation(None);
                }
                self.array(depth + 1);
            }
            11..=12 => {
                if self.rng.one_in(8) {
                    self.annotation(None);
                }
                self.push("{");
                for _ in 0..self.rng.below(3) {
                    self.key();
                    self.value(depth + 1, false);
                }
                self.push("}");
            }
            _ => {
                if self.rng.one_in(8) {
                    self.annotation(None);
                }
                self.push("o(");
                for _ in 0..self.rng.below(3) {
                    let n = *self.rng.pick(OCTETS);
                    self.push(format!("o:{}", enc(&vec![b'z'; n])));
                }
                self.push("o)");
            }
        }
    }

    fn row(&mut self, depth: usize, width: usize) {
        self.push("[");
        for _ in 0..width {
            self.value(depth, true);
        }
        self.push("]");
    }

    fn array(&mut self, depth: usize) {
        let width = self.rng.below(4);
        self.row(depth, width);
        for _ in 0..self.rng.below(3) {
            self.push("/");
            let w = if self.rng.one_in(4) {
                self.rng.below(4)
            } else {
                width
            };
            self.row(depth, w);
        }
    }

    fn document(&mut self) {
        if self.rng.one_in(3) {
            let v = *self.rng.pick(&[
                "1.0", "1.1", "1.2", "1.1", "1.2", "1.1", "1.2", "2.0", "0.9", "1.3",
            ]);
            self.push(format!("v:{v}"));
        }
        if self.rng.one_in(4) {
            self.push("ss");
        }
        for _ in 0..=self.rng.below(3) {
            self.key();
            self.value(0, false);
        }
        if self.rng.one_in(4) {
            self.push("se");
        }
    }

    /// A random event of any kind, for mutations.
    fn any_event(&mut self) -> String {
        let mut sub = Gen {
            rng: self.rng,
            events: Vec::new(),
        };
        match sub.rng.below(14) {
            0 => sub.key(),
            1 => sub.annotation(None),
            2 | 3 => {
                let in_array = sub.rng.one_in(2);
                sub.data(in_array);
            }
            4 => sub.push("v:1.1"),
            _ => {
                let ev = *sub
                    .rng
                    .pick(&["ss", "se", "{", "}", "[", "]", "/", "o(", "o)", "o:z"]);
                sub.push(ev);
            }
        }
        sub.events.remove(0)
    }
}

fn sequence(seed: u64, index: u64) -> String {
    let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15)
        ^ (index + 1).wrapping_mul(0xbf58_476d_1ce4_e5b9)
        | 1);
    let mut g = Gen {
        rng: &mut rng,
        events: Vec::new(),
    };
    g.document();
    if g.rng.one_in(3) {
        for _ in 0..=g.rng.below(3) {
            let len = g.events.len();
            match g.rng.below(4) {
                0 if len > 1 => {
                    let i = g.rng.below(len);
                    g.events.remove(i);
                }
                1 => {
                    let i = g.rng.below(len + 1);
                    let ev = g.any_event();
                    g.events.insert(i, ev);
                }
                2 if len > 1 => {
                    let i = g.rng.below(len - 1);
                    g.events.swap(i, i + 1);
                }
                _ => {
                    let i = g.rng.below(len);
                    let ev = g.events[i].clone();
                    g.events.insert(i, ev);
                }
            }
        }
    }
    g.events.join(" ")
}

/// Hand-written sequences that pin down specific rules.
fn fixed() -> Vec<String> {
    let mut out: Vec<String> = FIXED.iter().map(|s| (*s).to_owned()).collect();
    // Nesting one past the default limit of 64, and at it.
    for depth in [64, 65] {
        out.push(format!("k:a{}", " { k:a".repeat(depth - 1) + " {"));
        out.push(format!("k:a{}", " [".repeat(depth)));
    }
    out
}

const FIXED: &[&str] = &[
    "k:a d:num:5::",
    "k:a a:uint:8:16:::: d:num:ff::",
    "k:a a:uint:64:16:::: d:str:ff::",
    "k:a a:float_fix:64::8::: d:num:1.5::",
    "k:a a:datetime::::unix: d:num:5::",
    "v:1.1 k:a a:datetime::::unix: d:num:5::",
    "v:1.1 k:a a:datetime::::tai: d:num:1700000000:5:",
    "k:a [ d:anum:1:: ] / [ [ d:anum:2:: ] ]",
    "k:a [ d:anum:1:: d:anum:2:: ] / [ [ d:anum:3:: ] ]",
    "k:a a:uint:8:::: { }",
    "k:a a:uint:8:::: { k:b d:num:1:: }",
    "k:a o( o:zz o)",
    "v:1.2 k:a a:float:64::::ucum%3Amm[Hg]: d:num:1::",
    "v:1.1 k:a a:datetime::::unix: d:num:300000000000:5:",
    "v:1.1 k:a a:datetime::::unix: d:num:-300000000000:5:",
    "v:1.1 k:a a:datetime::::tai: d:num:-62135596800:5:",
    "k:a d:ref:.a[0]::",
    "v:1.1 k:a d:ref:.a[0]::",
    "k:a d:ref:.a[0]b::",
    "k:a d:ref:.a.b[12][3].c::",
    "k:a d:sym:a%00b::",
    "k:a%00b d:null:::",
    "k:a d:num:5::%2F",
    "k:a d:num:5::m",
    "k:a a:float::::: d:num:5::m",
    "k:a a:float:::::m d:num:5::m",
    "k:a a:float:::::m d:num:5::s",
    "k:a a:float:::::no_unit d:num:5::s",
    "k:a a:utf8:::::no_unit d:str:x::",
    "k:a d:str:x::m",
    "k:a [ d:anum:5::m ]",
    "k:a d:num:nan::m",
    "k:a a:uint:8:16:::: d:num:%2B5::",
];

struct Options {
    c: String,
    count: u64,
    seed: u64,
    show: usize,
    capture: Option<String>,
    input: Option<String>,
}

fn parse_options() -> Option<Options> {
    let mut o = Options {
        c: String::new(),
        count: 50_000,
        seed: 1,
        show: 3,
        capture: None,
        input: None,
    };
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        let v = args.next()?;
        match a.as_str() {
            "--c" => o.c = v,
            "--count" => o.count = v.parse().ok()?,
            "--seed" => o.seed = v.parse().ok()?,
            "--show" => o.show = v.parse().ok()?,
            "--capture" => o.capture = Some(v),
            "--input" => o.input = Some(v),
            _ => return None,
        }
    }
    (!o.c.is_empty()).then_some(o)
}

fn run_c(harness: &str, input: &[String]) -> Result<Vec<String>, String> {
    let mut child = Command::new(harness)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("{harness}: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let body: String = input.iter().map(|l| format!("{l}\n")).collect();
    let feeder = thread::spawn(move || stdin.write_all(body.as_bytes()));
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let lines: Vec<String> = BufReader::new(stdout)
        .lines()
        .map_while(Result::ok)
        .collect();
    let _ = feeder.join();
    let status = child.wait().map_err(|e| e.to_string())?;
    if !status.success() || lines.len() != input.len() {
        return Err(format!(
            "C harness failed ({status}, {} of {} lines)",
            lines.len(),
            input.len()
        ));
    }
    Ok(lines)
}

/// Whether the C output of an `ok` result reads back (with every value
/// limit at its maximum, as the writers assume).
fn readable(result: &str) -> bool {
    let hex = result.strip_prefix("ok ").unwrap_or("");
    let bytes: Vec<u8> = (0..hex.len() / 2)
        .filter_map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok())
        .collect();
    let mut limits = bovnar::Limits::default();
    limits.max_identifier_length = u16::MAX;
    limits.max_string_length = u16::MAX;
    limits.max_number_length = u16::MAX;
    limits.max_symbol_length = u16::MAX;
    limits.max_reference_length = u16::MAX;
    limits.max_struct_nesting = 255;
    limits.max_array_nesting = 255;
    limits.max_input_bytes = u64::MAX;
    bovnar::read_all(&bytes, &limits, |_| {}).is_ok()
}

/// `events[..=upto]` closed into a complete document: what is open (an
/// octet stream, a pending row after `/`, a value, rows and structs) is
/// closed in order, so the C output for that prefix can be read back.
fn completed_prefix(seq: &str, upto: usize) -> String {
    let events: Vec<&str> = seq
        .split(' ')
        .filter(|e| !e.is_empty())
        .take(upto + 1)
        .collect();
    let (mut stack, mut awaiting, mut octet, mut after_dim) = (Vec::new(), false, false, false);
    for ev in &events {
        match ev.split(':').next().unwrap_or("") {
            "k" | "a" => awaiting = true,
            "d" => awaiting = false,
            "{" => {
                stack.push("}");
                awaiting = false;
            }
            "[" => {
                stack.push("]");
                awaiting = false;
                after_dim = false;
            }
            "}" | "]" => {
                stack.pop();
            }
            "/" => after_dim = true,
            "o(" => {
                octet = true;
                awaiting = false;
            }
            "o)" => octet = false,
            _ => {}
        }
    }
    let mut out = events.join(" ");
    if octet {
        out.push_str(" o)");
    }
    if after_dim {
        // The row a `/` promises is also the value a key may be waiting for.
        out.push_str(" [ ]");
        awaiting = false;
    }
    if awaiting {
        out.push_str(" d:null:::");
    }
    while let Some(close) = stack.pop() {
        out.push(' ');
        out.push_str(close);
    }
    out
}

/// The index of the event an `err` result failed at; `None` for `ok`.
fn failed_at(result: &str) -> Option<usize> {
    result
        .strip_prefix("err ")
        .and_then(|r| r.split(' ').nth(1))
        .and_then(|i| i.parse().ok())
}

fn category(result: &str) -> &str {
    let mut parts = result.split(' ');
    match parts.next() {
        Some("err") => parts.next().unwrap_or("err"),
        _ => "ok",
    }
}

/// How a sequence the two writers disagree on is judged.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Verdict {
    /// The C writer counts a nested row as two elements and fails a row
    /// width check its reader passes (UPSTREAM_NOTES.md); bovnar-rs counts
    /// as the readers do.
    RowCount,
    /// bovnar-rs refuses an event the C writer accepts, and the C writer's
    /// output up to that event (closed into a document) is one no reader
    /// takes.
    Stricter,
    /// bovnar-rs refuses earlier, but closing the prefix into a document
    /// did not give the C writer a complete one to compare (usually a row
    /// width the closing broke).
    Inconclusive,
    /// A real divergence.
    Diverges,
}

fn main() -> ExitCode {
    let Some(opts) = parse_options() else {
        eprintln!(
            "usage: bovnar-writer-diff --c <writer_replay> [--count N] [--seed N] \
             [--show N] [--input FILE] [--capture FILE]"
        );
        return ExitCode::from(2);
    };
    let input: Vec<String> = match &opts.input {
        Some(path) => match fs::read_to_string(path) {
            Ok(text) => text
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| l.split('\t').next().unwrap_or("").to_owned())
                .collect(),
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::from(2);
            }
        },
        None => fixed()
            .into_iter()
            .chain((0..opts.count).map(|i| sequence(opts.seed, i)))
            .collect(),
    };
    let c = match run_c(&opts.c, &input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let rust: Vec<String> = input.iter().map(|s| replay::replay(s)).collect();
    // Refusals before the C writer's: run C on each prefix, closed.
    let earlier: Vec<usize> = (0..input.len())
        .filter(|&i| {
            rust[i] != c[i]
                && failed_at(&rust[i]).is_some_and(|r| failed_at(&c[i]).map_or(true, |c| r < c))
        })
        .collect();
    let prefixes: Vec<String> = earlier
        .iter()
        .map(|&i| completed_prefix(&input[i], failed_at(&rust[i]).unwrap_or(0)))
        .collect();
    let prefix_results = match run_c(&opts.c, &prefixes) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let prefix_of: HashMap<usize, &String> = earlier.iter().copied().zip(&prefix_results).collect();
    let mut groups: BTreeMap<(Verdict, String, String), Vec<usize>> = BTreeMap::new();
    let mut agree = 0usize;
    for i in 0..input.len() {
        if rust[i] == c[i] {
            agree += 1;
            continue;
        }
        let verdict = match prefix_of.get(&i) {
            Some(p) if p.starts_with("ok") && !readable(p) => Verdict::Stricter,
            Some(p) if p.starts_with("ok") => Verdict::Diverges,
            Some(_) => Verdict::Inconclusive,
            None if category(&c[i]) == "array_row_size_mismatch" => Verdict::RowCount,
            None => Verdict::Diverges,
        };
        let key = (
            verdict,
            category(&c[i]).to_owned(),
            category(&rust[i]).to_owned(),
        );
        groups.entry(key).or_default().push(i);
    }
    let clip = |s: &str| s.chars().take(200).collect::<String>();
    let mut diverging = 0usize;
    for ((verdict, cc, rc), items) in &mut groups {
        items.sort_by_key(|&i| input[i].len());
        println!("{:6}  {verdict:?}: C {cc} / Rust {rc}", items.len());
        if *verdict == Verdict::Diverges {
            diverging += items.len();
        }
        if *verdict != Verdict::Stricter {
            for &i in items.iter().take(opts.show) {
                println!(
                    "        {}\n          C:    {}\n          Rust: {}",
                    clip(&input[i]),
                    clip(&c[i]),
                    clip(&rust[i])
                );
            }
        }
    }
    println!(
        "{} sequences: {agree} agree, {} differ ({diverging} diverge)",
        input.len(),
        input.len() - agree
    );
    if let Some(path) = &opts.capture {
        if let Err(e) = fs::write(path, capture(&input, &c, &rust, &groups)) {
            eprintln!("{path}: {e}");
            return ExitCode::from(2);
        }
    }
    if diverging == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// The fixture for the writer_error_parity test: the shortest sequences of
/// every C result kind, and of every kind of verified difference, with the
/// C result and, for a difference, what bovnar-rs does instead.
fn capture(
    input: &[String],
    c: &[String],
    rust: &[String],
    groups: &BTreeMap<(Verdict, String, String), Vec<usize>>,
) -> String {
    const PER_KIND: usize = 40;
    const PER_DIFFERENCE: usize = 3;
    let small = |i: &usize| input[*i].len() < 400 && c[*i].len() < 400;
    let mut by_kind: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for i in (0..input.len()).filter(|i| rust[*i] == c[*i] && small(i)) {
        by_kind.entry(category(&c[i])).or_default().push(i);
    }
    let mut out = String::new();
    for items in by_kind.values_mut() {
        items.sort_by_key(|&i| (input[i].len(), i));
        for &i in items.iter().take(PER_KIND) {
            out.push_str(&format!("{}\t{}\n", input[i], c[i]));
        }
    }
    for ((verdict, _, _), items) in groups {
        if matches!(verdict, Verdict::RowCount | Verdict::Stricter) {
            for &i in items.iter().filter(|i| small(i)).take(PER_DIFFERENCE) {
                out.push_str(&format!("{}\t{}\t{}\n", input[i], c[i], rust[i]));
            }
        }
    }
    out
}
