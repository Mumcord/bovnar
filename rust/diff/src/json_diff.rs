//! Differential tester for `bovnar::json` against the C CLI's `bovnar convert`.
//!
//! ```text
//! bovnar-json-diff --cli <cref>/bovnar --corpus tests/conformance/corpus.json
//!                  [--fixtures <upstream>/tests/json] [--examples <upstream>/examples]
//!                  [--count 20000] [--seed 1] [--jobs N] [--show 10]
//!                  [--capture <file.json>]
//! ```
//!
//! JSON to Bovnar: the upstream JSON fixtures, then generated JSON documents
//! and byte-level mutations of them, until `--count` inputs ran. Compared:
//! exit status, stdout byte for byte, and the error class (stderr mapped to
//! a `JsonError` variant).
//!
//! Bovnar to JSON: every corpus input, the upstream examples, grammar-
//! generated documents, and the Bovnar output of every JSON input that
//! converted. Compared: exit status, stdout byte for byte, error class, and
//! the per-kind loss counts of the stderr report.
//!
//! `--capture` writes the results for the fixtures, the corpus, the examples
//! and the first 300 generated inputs as the expectation file that
//! `tests/json.rs` checks. `--fixtures` may be given more than once.

#[allow(dead_code)]
mod gen;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use std::{env, fs, thread};

use base64::Engine as _;
use bovnar::json::{JsonError, LossKind};
use bovnar::Limits;
use gen::{mutate, Generator, Pools, Rng};

/// Generated inputs recorded by `--capture` after the fixed ones.
const CAPTURE_GENERATED: usize = 300;

struct Options {
    cli: PathBuf,
    corpus: PathBuf,
    fixtures: Vec<PathBuf>,
    examples: Option<PathBuf>,
    count: usize,
    seed: u64,
    jobs: usize,
    show: usize,
    capture: Option<PathBuf>,
}

fn parse_options() -> Option<Options> {
    let mut args = env::args().skip(1);
    let mut opts = Options {
        cli: PathBuf::new(),
        corpus: PathBuf::new(),
        fixtures: Vec::new(),
        examples: None,
        count: 20_000,
        seed: 1,
        jobs: thread::available_parallelism().map_or(4, |n| n.get()),
        show: 10,
        capture: None,
    };
    while let Some(flag) = args.next() {
        let value = args.next()?;
        match flag.as_str() {
            "--cli" => opts.cli = value.into(),
            "--corpus" => opts.corpus = value.into(),
            "--fixtures" => opts.fixtures.push(value.into()),
            "--examples" => opts.examples = Some(value.into()),
            "--count" => opts.count = value.parse().ok()?,
            "--seed" => opts.seed = value.parse().ok()?,
            "--jobs" => opts.jobs = value.parse::<usize>().ok()?.max(1),
            "--show" => opts.show = value.parse().ok()?,
            "--capture" => opts.capture = Some(value.into()),
            _ => return None,
        }
    }
    (!opts.cli.as_os_str().is_empty() && !opts.corpus.as_os_str().is_empty()).then_some(opts)
}

/// The limits `bovnar convert` runs with.
fn cli_limits() -> Limits {
    let mut l = Limits::default();
    l.max_struct_nesting = 255;
    l.max_array_nesting = 255;
    l.max_input_bytes = 0;
    l.max_dom_nodes = u64::MAX;
    l
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Direction {
    FromJson,
    ToJson,
}

impl Direction {
    fn flag(self) -> &'static str {
        match self {
            Self::FromJson => "json",
            Self::ToJson => "bvnr",
        }
    }
}

/// The comparable result of one conversion.
#[derive(PartialEq, Eq, Clone, Debug)]
struct Outcome {
    status: i32,
    stdout: Vec<u8>,
    /// Error class, empty on success.
    class: String,
    /// Loss counts in report order (Bovnar to JSON only).
    loss: Vec<u32>,
}

/// Maps the C CLI's stderr to an error class.
fn c_class(dir: Direction, stderr: &str) -> String {
    let table: &[(&str, &str)] = &[
        ("JSON nesting exceeds", "too_deep"),
        ("NUL (\\u0000)", "nul"),
        ("integer literal out of range", "integer_out_of_range"),
        ("too large for a 64-bit float", "float_overflow"),
        ("trailing content after JSON value", "trailing"),
        ("JSON root must be an object", "root"),
        ("is not a valid bovnar identifier", "invalid_key"),
        ("contains the control character", "control_character"),
        ("is not valid UTF-8", "invalid_utf8"),
        ("sub-second digits", "fractional_datetime"),
    ];
    for (needle, class) in table {
        if stderr.contains(needle) {
            return (*class).to_string();
        }
    }
    if let Some(rest) = stderr.split("Bovnar writer error: ").nth(1) {
        return format!("writer:{}", rest.split_whitespace().next().unwrap_or(""));
    }
    if let Some(rest) = stderr.split("no bovnar representation: ").nth(1) {
        return format!(
            "unrepresentable:{}",
            rest.split_whitespace().next().unwrap_or("")
        );
    }
    if let Some(rest) = stderr.split("Parse error in ").nth(1) {
        let kind = rest.split(": ").nth(1).unwrap_or("").trim();
        return format!("parse:{kind}");
    }
    if stderr.contains("Failed to parse JSON") || stderr.contains("malformed JSON number") {
        return "syntax".into();
    }
    if dir == Direction::ToJson && stderr.contains("Failed to parse") {
        return "parse:".into();
    }
    String::new()
}

fn rust_class(e: &JsonError) -> String {
    match e {
        JsonError::TooLarge => "too_large".into(),
        JsonError::Syntax { .. } => "syntax".into(),
        JsonError::TooDeep { .. } => "too_deep".into(),
        JsonError::Nul { .. } => "nul".into(),
        JsonError::IntegerOutOfRange { .. } => "integer_out_of_range".into(),
        JsonError::FloatOverflow { .. } => "float_overflow".into(),
        JsonError::TrailingContent { .. } => "trailing".into(),
        JsonError::RootNotObject => "root".into(),
        JsonError::InvalidKey { .. } => "invalid_key".into(),
        JsonError::ControlCharacter { .. } => "control_character".into(),
        JsonError::InvalidUtf8 { .. } => "invalid_utf8".into(),
        JsonError::Writer(e) => format!("writer:{}", e.kind().as_str()),
        JsonError::Unrepresentable(e) => format!("unrepresentable:{}", e.kind().as_str()),
        JsonError::Parse(e) => format!("parse:{}", e.kind().as_str()),
        JsonError::FractionalDatetime { .. } => "fractional_datetime".into(),
        _ => "other".into(),
    }
}

/// The loss counts of the C report, in `LossKind::ALL` order.
fn c_loss(stderr: &str) -> Vec<u32> {
    let phrases = [
        "value(s) lose their unit",
        "integer(s) wider than 64 bits",
        "integer(s) lose their numeric base",
        "non-finite float(s)",
        "float(s) wider or more exact",
        "datetime(s) become bare integers",
        "symbol(s) become plain strings",
        "reference(s) become plain strings",
        "octet stream(s) become hex",
        "multi-dimensional array(s)",
    ];
    phrases
        .iter()
        .map(|p| {
            stderr
                .lines()
                .find(|l| l.contains(p))
                .and_then(|l| l.split_whitespace().next())
                .and_then(|n| n.parse().ok())
                .unwrap_or(0)
        })
        .collect()
}

fn run_c(cli: &Path, dir: Direction, input: &[u8], scratch: &Path) -> Outcome {
    let file = scratch.join(if dir == Direction::FromJson {
        "in.json"
    } else {
        "in.bvnr"
    });
    if fs::write(&file, input).is_err() {
        return Outcome {
            status: -1,
            stdout: Vec::new(),
            class: "scratch write failed".into(),
            loss: Vec::new(),
        };
    }
    match Command::new(cli)
        .args(["convert", "--from", dir.flag()])
        .arg(&file)
        .output()
    {
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            let status = out.status.code().unwrap_or(-1);
            let class = if status == 0 {
                String::new()
            } else {
                c_class(dir, &stderr)
            };
            let (class, loss) = if dir == Direction::ToJson {
                let loss = c_loss(&stderr);
                // A lossy conversion exits 1 without an error.
                let class = if class.is_empty() && status != 0 && !out.stdout.is_empty() {
                    String::new()
                } else {
                    class
                };
                (class, loss)
            } else {
                (class, Vec::new())
            };
            Outcome {
                status,
                stdout: if status == 0 || dir == Direction::ToJson {
                    out.stdout
                } else {
                    Vec::new()
                },
                class,
                loss,
            }
        }
        Err(e) => Outcome {
            status: -1,
            stdout: Vec::new(),
            class: format!("spawn failed: {e}"),
            loss: Vec::new(),
        },
    }
}

fn run_rust(dir: Direction, input: &[u8]) -> Outcome {
    let limits = cli_limits();
    match dir {
        Direction::FromJson => match bovnar::json::to_bovnar(input, &limits) {
            Ok(text) => Outcome {
                status: 0,
                stdout: text.into_bytes(),
                class: String::new(),
                loss: Vec::new(),
            },
            Err(e) => Outcome {
                status: 1,
                stdout: Vec::new(),
                class: rust_class(&e),
                loss: Vec::new(),
            },
        },
        Direction::ToJson => match bovnar::json::from_bovnar(input, &limits) {
            Ok(out) => Outcome {
                status: i32::from(!out.loss.is_empty()),
                stdout: out.json.into_bytes(),
                class: String::new(),
                loss: LossKind::ALL.iter().map(|&k| out.loss.count(k)).collect(),
            },
            Err(e) => Outcome {
                status: 1,
                stdout: Vec::new(),
                class: rust_class(&e),
                loss: vec![0; LossKind::ALL.len()],
            },
        },
    }
}

/// Compares what both sides can be held to: a C parse error is compared by
/// class only, since the C reader's error names are the oracle there.
fn same(c: &Outcome, rust: &Outcome) -> bool {
    c.status == rust.status
        && c.stdout == rust.stdout
        && c.class == rust.class
        && c.loss == rust.loss
}

// ---------------------------------------------------------------------------
// JSON generation

const KEYS: &[&str] = &[
    "a",
    "b",
    "id",
    "name",
    "value",
    "x_y",
    "k-1",
    "k+2",
    "_",
    "Z9",
    "\u{e9}t\u{e9}",
    "a b",
    "1a",
    "",
    "\u{c2}",
    "k.k",
    "\u{1F600}",
    "a\u{7f}",
    "-a",
    "A",
];

const FLOATS: &[&str] = &[
    "0.0",
    "-0.0",
    "0.1",
    "2.0",
    "1e10",
    "1E-7",
    "1.5e+300",
    "1e308",
    "1.7976931348623157e308",
    "1e309",
    "-1e309",
    "5e-324",
    "2.2250738585072014e-308",
    "2.2250738585072009e-308",
    "4.9e-324",
    "1e-999",
    "123456789012345678901234567890.5",
    "0.30000000000000004",
    "9007199254740993.0",
    "1e16",
    "1e17",
    "18014398509481988.0",
    "100.0",
    "3.5",
    "-2.5e-5",
    "6.02214076e23",
    "1e-5",
    "0.0001",
];

const INTS: &[&str] = &[
    "0",
    "-0",
    "1",
    "-1",
    "42",
    "18446744073709551615",
    "18446744073709551616",
    "9223372036854775807",
    "-9223372036854775808",
    "-9223372036854775809",
    "99999999999999999999",
    "007",
    "-",
    "1.",
    ".5",
    "1e",
];

struct JsonGen<'r> {
    rng: &'r mut Rng,
    out: Vec<u8>,
}

impl JsonGen<'_> {
    fn ws(&mut self) {
        if self.rng.one_in(3) {
            let ws: &[&[u8]] = &[b" ", b"\n", b"\t", b"\r\n", b"  ", b"\x0b", b"\x0c"];
            let w = self.rng.pick(ws);
            self.out.extend_from_slice(w);
        }
    }

    fn key(&mut self, used: &mut Vec<String>) {
        let mut key = if self.rng.one_in(12) {
            (*self.rng.pick(KEYS)).to_string()
        } else {
            format!(
                "{}{}",
                self.rng.pick(&["k", "v", "item", "x"]),
                self.rng.below(40)
            )
        };
        if !self.rng.one_in(40) {
            while used.contains(&key) {
                key.push('_');
            }
        }
        used.push(key.clone());
        self.string_lit(&key);
    }

    fn string_lit(&mut self, s: &str) {
        self.out.push(b'"');
        for c in s.chars() {
            match c {
                '"' => self.out.extend_from_slice(b"\\\""),
                '\\' => self.out.extend_from_slice(b"\\\\"),
                c if (c as u32) < 0x20 => self
                    .out
                    .extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes()),
                c => {
                    let mut b = [0; 4];
                    self.out.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                }
            }
        }
        self.out.push(b'"');
    }

    fn string(&mut self) {
        self.out.push(b'"');
        for _ in 0..self.rng.below(8) {
            let pieces: &[&[u8]] = &[
                b"abc",
                b" ",
                b"\\n",
                b"\\t",
                b"\\\"",
                b"\\\\",
                b"\\/",
                b"\\u00e9",
                b"\\uD83D\\uDE00",
                b"\xc3\xa4",
                b"\xe2\x82\xac",
                b"#",
                b"'",
                b"x",
                b"\\b",
                b"\\f",
                b"\\r",
                b"\\u001f",
                b"\\u007f",
                b"\\u0000",
                b"\\uDC00",
                b"\\uD800x",
                b"\\x",
                b"\x01",
                b"\xff",
                b"\xed\xa0\x80",
                b"\\u0041",
                b"\\u20AC",
            ];
            let p = if self.rng.one_in(6) {
                *self.rng.pick(pieces)
            } else {
                *self.rng.pick(&pieces[..8])
            };
            self.out.extend_from_slice(p);
        }
        self.out.push(b'"');
    }

    fn float(&mut self) {
        let text = match self.rng.below(4) {
            0 => (*self.rng.pick(FLOATS)).to_string(),
            1 => format!("{:?}", f64::from_bits(self.rng.next())),
            2 => format!("{:e}", (self.rng.next() % 100_000) as f64 / 8.0),
            _ => format!("{}.{}", self.rng.below(1000), self.rng.below(1000)),
        };
        let text = if text.contains(['n', 'N', 'i']) {
            "1.25".to_string()
        } else {
            text
        };
        self.out.extend_from_slice(text.as_bytes());
    }

    fn int(&mut self) {
        let text = match self.rng.below(3) {
            0 => (*self.rng.pick(INTS)).to_string(),
            1 => (self.rng.next() as i64).to_string(),
            _ => self.rng.below(1000).to_string(),
        };
        self.out.extend_from_slice(text.as_bytes());
    }

    /// A scalar of one kind: 0 null, 1 bool, 2 int, 3 float, 4 string.
    fn scalar(&mut self, kind: usize) {
        match kind {
            0 => self.out.extend_from_slice(b"null"),
            1 => self.out.extend_from_slice(if self.rng.one_in(2) {
                b"true"
            } else {
                b"false"
            }),
            2 => self.int(),
            3 => self.float(),
            _ => self.string(),
        }
    }

    fn value(&mut self, depth: usize) {
        let pick = if depth > 5 {
            self.rng.below(5)
        } else {
            self.rng.below(8)
        };
        match pick {
            5 => self.array(depth + 1),
            6 => self.object(depth + 1, None),
            7 => {
                // An array of objects, mostly of one shape.
                let n = self.rng.below(4);
                let shape: Vec<String> = (0..1 + self.rng.below(3))
                    .map(|i| format!("f{i}"))
                    .collect();
                self.out.push(b'[');
                for i in 0..n {
                    if i > 0 {
                        self.out.push(b',');
                    }
                    let keys = if self.rng.one_in(8) {
                        None
                    } else {
                        Some(shape.as_slice())
                    };
                    self.object(depth + 1, keys);
                }
                self.out.push(b']');
            }
            k => self.scalar(k),
        }
    }

    fn array(&mut self, depth: usize) {
        self.out.push(b'[');
        self.ws();
        let n = self.rng.below(5);
        let kind = self.rng.below(5);
        let nested = depth < 5 && self.rng.one_in(4);
        for i in 0..n {
            if i > 0 {
                self.out.push(b',');
                self.ws();
            }
            if nested {
                self.array(depth + 1);
            } else if self.rng.one_in(10) {
                self.value(depth);
            } else {
                self.scalar(kind);
            }
        }
        self.ws();
        self.out.push(b']');
    }

    fn object(&mut self, depth: usize, keys: Option<&[String]>) {
        self.out.push(b'{');
        self.ws();
        let mut used = Vec::new();
        let n = keys.map_or_else(|| self.rng.below(5), <[String]>::len);
        for i in 0..n {
            if i > 0 {
                self.out.push(b',');
                self.ws();
            }
            match keys.and_then(|k| k.get(i)) {
                Some(k) => self.string_lit(k),
                None => self.key(&mut used),
            }
            self.ws();
            self.out.push(b':');
            self.ws();
            if keys.is_some() {
                self.scalar(2);
            } else {
                self.value(depth);
            }
        }
        self.ws();
        self.out.push(b'}');
    }
}

fn generate_json(rng: &mut Rng) -> Vec<u8> {
    let mut g = JsonGen {
        rng,
        out: Vec::new(),
    };
    g.ws();
    match g.rng.below(40) {
        0 => {
            // Deep nesting around the reference's limits.
            let depth = *g
                .rng
                .pick(&[64usize, 65, 254, 255, 256, 511, 512, 513, 514]);
            let (open, close) = if g.rng.one_in(2) {
                ("[", "]")
            } else {
                ("{\"a\":", "}")
            };
            g.out.extend_from_slice(b"{\"d\":");
            g.out.extend_from_slice(open.repeat(depth).as_bytes());
            g.out.extend_from_slice(b"1");
            g.out.extend_from_slice(close.repeat(depth).as_bytes());
            g.out.push(b'}');
        }
        1 => g.array(0),
        2 => g.scalar(4),
        _ => g.object(0, None),
    }
    g.ws();
    if g.rng.one_in(30) {
        let tails: &[&[u8]] = &[b"x", b"\0", b"\0junk", b" {}", b"\n"];
        let t = g.rng.pick(tails);
        g.out.extend_from_slice(t);
    }
    g.out
}

// ---------------------------------------------------------------------------

struct Case {
    origin: String,
    dir: Direction,
    input: Vec<u8>,
}

struct Mismatch {
    case: Case,
    c: Outcome,
    rust: Outcome,
}

fn load_inputs(opts: &Options) -> Result<(Vec<Case>, Vec<Case>, Vec<Vec<u8>>), String> {
    let mut fixtures = Vec::new();
    for dir in &opts.fixtures {
        let mut paths: Vec<_> = fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        paths.sort();
        for p in paths {
            let input = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            fixtures.push(Case {
                origin: format!("fixture {name}"),
                dir: Direction::FromJson,
                input,
            });
        }
    }
    let text =
        fs::read_to_string(&opts.corpus).map_err(|e| format!("{}: {e}", opts.corpus.display()))?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut corpus = Vec::new();
    let mut docs = Vec::new();
    for case in json["cases"].as_array().ok_or("corpus has no cases")? {
        let input = base64::engine::general_purpose::STANDARD
            .decode(case["input"].as_str().unwrap_or_default())
            .map_err(|e| e.to_string())?;
        docs.push(input.clone());
        corpus.push(Case {
            origin: format!("corpus {}", case["id"].as_str().unwrap_or("?")),
            dir: Direction::ToJson,
            input,
        });
    }
    if let Some(dir) = &opts.examples {
        let mut paths: Vec<_> = fs::read_dir(dir)
            .map_err(|e| format!("{}: {e}", dir.display()))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "bvnr"))
            .collect();
        paths.sort();
        for p in paths {
            let input = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            docs.push(input.clone());
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            corpus.push(Case {
                origin: format!("example {name}"),
                dir: Direction::ToJson,
                input,
            });
        }
    }
    Ok((fixtures, corpus, docs))
}

fn derived(seed: u64, index: usize, pools: &Pools, fixtures: &[Case]) -> Case {
    let mut rng = Rng::new(seed ^ (index as u64).wrapping_mul(0x2545_F491_4F6C_DD1D));
    match rng.below(10) {
        0..=5 => Case {
            origin: format!("generated-json #{index}"),
            dir: Direction::FromJson,
            input: generate_json(&mut rng),
        },
        6 | 7 => {
            let base = if !fixtures.is_empty() && rng.one_in(3) {
                fixtures[rng.below(fixtures.len())].input.clone()
            } else {
                generate_json(&mut rng)
            };
            Case {
                origin: format!("mutated-json #{index}"),
                dir: Direction::FromJson,
                input: mutate(&mut rng, &base),
            }
        }
        _ => Case {
            origin: format!("generated-bvnr #{index}"),
            dir: Direction::ToJson,
            input: Generator::new(pools).document(&mut rng),
        },
    }
}

fn show(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(&bytes[..bytes.len().min(600)]);
    format!("{s:?}")
}

/// Longest stdout `--capture` stores verbatim.
const CAPTURE_STDOUT_MAX: usize = 2048;

/// 64-bit FNV-1a, for long captured outputs.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn write_capture(
    path: &Path,
    rows: &[(String, Direction, Vec<u8>, Outcome)],
) -> Result<(), String> {
    let cases: Vec<serde_json::Value> = rows
        .iter()
        .map(|(origin, dir, input, o)| {
            let mut v = serde_json::json!({
                "origin": origin,
                "direction": dir.flag(),
                "status": o.status,
                "class": o.class,
            });
            // Long outputs are kept as a hash so the file stays small.
            if o.stdout.len() > CAPTURE_STDOUT_MAX {
                v["stdout_fnv1a"] = format!("{:016x}", fnv1a(&o.stdout)).into();
            } else {
                v["stdout"] = String::from_utf8_lossy(&o.stdout).into();
            }
            if !origin.starts_with("corpus") && !origin.starts_with("example") {
                v["input"] = base64::engine::general_purpose::STANDARD
                    .encode(input)
                    .into();
            }
            if *dir == Direction::ToJson {
                v["loss"] = o.loss.clone().into();
            }
            v
        })
        .collect();
    let doc = serde_json::json!({
        "source": "bovnar convert of the C reference (UCUM and CF off), captured by bovnar-json-diff --capture",
        "cases": cases,
    });
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

fn main() -> ExitCode {
    let Some(opts) = parse_options() else {
        eprintln!(
            "usage: bovnar-json-diff --cli <bovnar> --corpus <corpus.json> [--fixtures <dir>] \
             [--examples <dir>] [--count N] [--seed S] [--jobs N] [--show N] [--capture <file>]"
        );
        return ExitCode::from(2);
    };
    let (fixtures, corpus, docs) = match load_inputs(&opts) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("bovnar-json-diff: {e}");
            return ExitCode::from(2);
        }
    };
    let pools = Pools::harvest(docs.iter().map(Vec::as_slice));
    let fixed: Vec<&Case> = fixtures.iter().chain(corpus.iter()).collect();
    let total = opts.count.max(fixed.len());
    let scratch_root = env::temp_dir().join(format!("bovnar-json-diff-{}", std::process::id()));
    let next = AtomicUsize::new(0);
    let mismatches = Mutex::new(Vec::new());
    let counts = Mutex::new(BTreeMap::<(Direction, bool), usize>::new());
    let followups = AtomicUsize::new(0);
    let captured = Mutex::new(Vec::new());
    let start = Instant::now();

    thread::scope(|scope| {
        for job in 0..opts.jobs {
            let scratch = scratch_root.join(job.to_string());
            let (next, mismatches, counts, followups, captured) =
                (&next, &mismatches, &counts, &followups, &captured);
            let (fixed, fixtures, pools, opts) = (&fixed, &fixtures, &pools, &opts);
            scope.spawn(move || {
                if fs::create_dir_all(&scratch).is_err() {
                    return;
                }
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    if index >= total {
                        break;
                    }
                    let case = match fixed.get(index) {
                        Some(c) => Case {
                            origin: c.origin.clone(),
                            dir: c.dir,
                            input: c.input.clone(),
                        },
                        None => derived(opts.seed, index, pools, fixtures),
                    };
                    let mut queue = vec![case];
                    while let Some(case) = queue.pop() {
                        let c = run_c(&opts.cli, case.dir, &case.input, &scratch);
                        let rust = run_rust(case.dir, &case.input);
                        *counts
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .entry((case.dir, c.status == 0))
                            .or_default() += 1;
                        if opts.capture.is_some() && index < fixed.len() + CAPTURE_GENERATED {
                            captured.lock().unwrap_or_else(|e| e.into_inner()).push((
                                case.origin.clone(),
                                case.dir,
                                case.input.clone(),
                                c.clone(),
                            ));
                        }
                        // Feed every converted JSON back the other way.
                        if case.dir == Direction::FromJson && c.status == 0 {
                            followups.fetch_add(1, Ordering::Relaxed);
                            queue.push(Case {
                                origin: format!("{} (back)", case.origin),
                                dir: Direction::ToJson,
                                input: c.stdout.clone(),
                            });
                        }
                        if !same(&c, &rust) {
                            mismatches
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .push(Mismatch { case, c, rust });
                        }
                    }
                }
                let _ = fs::remove_dir_all(&scratch);
            });
        }
    });
    let _ = fs::remove_dir_all(&scratch_root);

    let counts = counts.into_inner().unwrap_or_else(|e| e.into_inner());
    let mismatches = mismatches.into_inner().unwrap_or_else(|e| e.into_inner());
    let get = |d, ok| counts.get(&(d, ok)).copied().unwrap_or(0);
    println!(
        "bovnar-json-diff: {total} inputs + {} round trips in {:.1}s, seed {}; json->bvnr {} ok / {} refused, \
         bvnr->json {} exact / {} lossy or refused; {} mismatches",
        followups.load(Ordering::Relaxed),
        start.elapsed().as_secs_f64(),
        opts.seed,
        get(Direction::FromJson, true),
        get(Direction::FromJson, false),
        get(Direction::ToJson, true),
        get(Direction::ToJson, false),
        mismatches.len(),
    );
    for m in mismatches.iter().take(opts.show) {
        println!("--- {} ({:?})", m.case.origin, m.case.dir);
        println!("  input: {}", show(&m.case.input));
        println!(
            "  c:    status {} class {:?} loss {:?}\n        {}",
            m.c.status,
            m.c.class,
            m.c.loss,
            show(&m.c.stdout)
        );
        println!(
            "  rust: status {} class {:?} loss {:?}\n        {}",
            m.rust.status,
            m.rust.class,
            m.rust.loss,
            show(&m.rust.stdout)
        );
    }
    if let Some(path) = &opts.capture {
        let mut rows = captured.into_inner().unwrap_or_else(|e| e.into_inner());
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        if let Err(e) = write_capture(path, &rows) {
            eprintln!("bovnar-json-diff: {e}");
            return ExitCode::from(2);
        }
    }
    if mismatches.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
