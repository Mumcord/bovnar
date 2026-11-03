//! Differential tester: runs the bovnar-rs IUT adapter and the C reference
//! IUT adapter (`bvnr_conformance_iut`) on the same inputs and compares
//! stdout and exit code byte for byte. The C reference is the oracle.
//!
//! ```text
//! bovnar-diff --c <c-iut> --rust <rust-iut> --corpus tests/conformance/corpus.json
//!             [--examples <upstream>/examples] [--count 20000] [--seed 1]
//!             [--jobs N] [--show 10] [--dump <dir>]
//! ```
//!
//! Inputs, in order: every corpus case with its limit options, then a mix
//! of grammar-generated documents and byte-level mutations of corpus inputs,
//! a quarter of them with random limit options, until `--count` inputs ran.

mod gen;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, ExitStatus, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use base64::Engine as _;
use gen::{mutate, Generator, Pools, Rng};

/// How long one IUT run may take before it counts as hung.
const TIMEOUT: Duration = Duration::from_secs(10);
/// Inputs above this size are reported unminimised.
const MINIMISE_MAX_LEN: usize = 4096;
/// Upper bound on IUT pairs run while minimising one mismatch.
const MINIMISE_BUDGET: usize = 400;

struct Options {
    c_iut: PathBuf,
    rust_iut: PathBuf,
    corpus: PathBuf,
    examples: Option<PathBuf>,
    count: usize,
    seed: u64,
    jobs: usize,
    show: usize,
    /// Write the derived inputs to this directory instead of running them.
    dump: Option<PathBuf>,
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: bovnar-diff --c <c-iut> --rust <rust-iut> --corpus <corpus.json> \
         [--examples <dir>] [--count N] [--seed S] [--jobs N] [--show N] [--dump <dir>]"
    );
    ExitCode::from(2)
}

fn parse_options() -> Option<Options> {
    let mut args = env::args().skip(1);
    let mut opts = Options {
        c_iut: PathBuf::new(),
        rust_iut: PathBuf::new(),
        corpus: PathBuf::new(),
        examples: None,
        count: 20_000,
        seed: 1,
        jobs: thread::available_parallelism().map_or(4, |n| n.get()),
        show: 10,
        dump: None,
    };
    while let Some(flag) = args.next() {
        let value = args.next()?;
        match flag.as_str() {
            "--c" => opts.c_iut = value.into(),
            "--rust" => opts.rust_iut = value.into(),
            "--corpus" => opts.corpus = value.into(),
            "--examples" => opts.examples = Some(value.into()),
            "--count" => opts.count = value.parse().ok()?,
            "--seed" => opts.seed = value.parse().ok()?,
            "--jobs" => opts.jobs = value.parse::<usize>().ok()?.max(1),
            "--show" => opts.show = value.parse().ok()?,
            "--dump" => opts.dump = Some(value.into()),
            _ => return None,
        }
    }
    let complete = !opts.c_iut.as_os_str().is_empty()
        && !opts.rust_iut.as_os_str().is_empty()
        && !opts.corpus.as_os_str().is_empty();
    complete.then_some(opts)
}

/// One input with the limit options both IUTs get.
#[derive(Clone)]
struct Case {
    origin: String,
    args: Vec<String>,
    input: Vec<u8>,
}

/// What an IUT printed and how it exited.
#[derive(PartialEq, Eq, Clone)]
struct Outcome {
    stdout: Vec<u8>,
    /// Exit code, or a description of how the process failed.
    status: Result<i32, String>,
}

fn run(iut: &Path, case: &Case) -> Outcome {
    let failed = |what: String| Outcome {
        stdout: Vec::new(),
        status: Err(what),
    };
    let mut child = match Command::new(iut)
        .args(&case.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(e) => return failed(format!("spawn failed: {e}")),
    };
    let (Some(mut stdin), Some(mut stdout)) = (child.stdin.take(), child.stdout.take()) else {
        return failed("no pipes".into());
    };
    let input = case.input.clone();
    // An IUT may exit before reading all of stdin; the broken pipe is fine.
    let writer = thread::spawn(move || drop(stdin.write_all(&input)));
    let reader = thread::spawn(move || {
        let mut out = Vec::new();
        stdout.read_to_end(&mut out).map(|_| out)
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if start.elapsed() > TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                break Err("timeout".to_owned());
            }
            Ok(None) => thread::sleep(Duration::from_micros(200)),
            Err(e) => break Err(format!("wait failed: {e}")),
        }
    };
    let _ = writer.join();
    let stdout = match reader.join() {
        Ok(Ok(out)) => out,
        _ => return failed("reading stdout failed".into()),
    };
    let status = status.and_then(|s| s.code().ok_or_else(|| no_exit_code(s)));
    Outcome { stdout, status }
}

#[cfg(unix)]
fn no_exit_code(status: ExitStatus) -> String {
    use std::os::unix::process::ExitStatusExt;
    format!("killed by signal {}", status.signal().unwrap_or(0))
}

// Only Unix processes can end without an exit code; this keeps the workspace
// building on other hosts.
#[cfg(not(unix))]
fn no_exit_code(_status: ExitStatus) -> String {
    "ended without an exit code".to_owned()
}

struct Corpus {
    cases: Vec<Case>,
    docs: Vec<Vec<u8>>,
}

fn load_corpus(path: &Path, examples: Option<&Path>) -> Result<Corpus, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut cases = Vec::new();
    for case in json["cases"].as_array().ok_or("corpus has no cases")? {
        let input = base64::engine::general_purpose::STANDARD
            .decode(case["input"].as_str().unwrap_or_default())
            .map_err(|e| format!("case {}: {e}", case["id"]))?;
        let mut args = Vec::new();
        if let Some(limits) = case["limits"].as_object() {
            for (name, value) in limits {
                args.push(format!("--{}", name.replace('_', "-")));
                args.push(value.to_string());
            }
        }
        let origin = format!("corpus {}", case["id"].as_str().unwrap_or("?"));
        cases.push(Case {
            origin,
            args,
            input,
        });
    }
    let mut docs: Vec<Vec<u8>> = cases.iter().map(|c| c.input.clone()).collect();
    if let Some(dir) = examples {
        let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let mut paths: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
        paths.sort();
        for path in paths
            .iter()
            .filter(|p| p.extension().is_some_and(|x| x == "bvnr"))
        {
            docs.push(fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?);
        }
    }
    Ok(Corpus { cases, docs })
}

const LIMIT_OPTIONS: &[&str] = &[
    "--max-id",
    "--max-str",
    "--max-num",
    "--max-sym",
    "--max-ref",
    "--max-snest",
    "--max-anest",
    "--max-aitems",
];
const LIMIT_VALUES: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "8",
    "16",
    "255",
    "256",
    "65535",
    "65536",
    "4294967296",
    "-1",
    "x",
    "007",
];

fn random_limits(rng: &mut Rng) -> Vec<String> {
    let mut args = Vec::new();
    if rng.one_in(4) {
        for _ in 0..1 + rng.below(3) {
            args.push((*rng.pick(LIMIT_OPTIONS)).to_owned());
            let value = if rng.one_in(2) {
                rng.below(12).to_string()
            } else {
                (*rng.pick(LIMIT_VALUES)).to_owned()
            };
            args.push(value);
        }
    }
    args
}

/// The `index`-th input after the corpus; each is derived from its own seed
/// so any input can be regenerated from `(seed, index)`.
fn derived_case(seed: u64, index: usize, corpus: &Corpus, pools: &Pools) -> Case {
    let mut rng = Rng::new(seed ^ (index as u64).wrapping_mul(0xA24B_AED4_963E_E407));
    let args = random_limits(&mut rng);
    if rng.one_in(2) {
        let mut input = Generator::new(pools).document(&mut rng);
        // Edits on top of a mostly valid document reach checks that
        // neither a clean document nor a mutated corpus case gets to.
        if rng.one_in(4) {
            input = mutate(&mut rng, &input);
        }
        Case {
            origin: format!("generated #{index}"),
            args,
            input,
        }
    } else {
        let base = rng.pick(&corpus.docs);
        let input = mutate(&mut rng, base);
        Case {
            origin: format!("mutated #{index}"),
            args,
            input,
        }
    }
}

struct Mismatch {
    case: Case,
    c: Outcome,
    rust: Outcome,
}

/// Shrinks the input while the two IUTs still disagree (delta debugging,
/// bounded by `MINIMISE_BUDGET` runs).
fn minimise(opts: &Options, mismatch: &Mismatch) -> Case {
    let mut case = mismatch.case.clone();
    if case.input.len() > MINIMISE_MAX_LEN {
        return case;
    }
    let budget = std::cell::Cell::new(MINIMISE_BUDGET);
    let differs = |candidate: &Case| {
        budget.set(budget.get().saturating_sub(1));
        run(&opts.c_iut, candidate) != run(&opts.rust_iut, candidate)
    };
    let mut parts = 2;
    while case.input.len() >= 2 && budget.get() > 0 {
        let chunk = (case.input.len() / parts).max(1);
        let mut reduced = false;
        let mut start = 0;
        while start < case.input.len() && budget.get() > 0 {
            let mut candidate = case.clone();
            candidate
                .input
                .drain(start..(start + chunk).min(case.input.len()));
            if differs(&candidate) {
                case = candidate;
                reduced = true;
                break;
            }
            start += chunk;
        }
        if reduced {
            parts = (parts - 1).max(2);
        } else if chunk == 1 {
            break;
        } else {
            parts = (parts * 2).min(case.input.len());
        }
    }
    case
}

fn show_bytes(bytes: &[u8], max: usize) -> String {
    let shown: String = bytes
        .iter()
        .take(max)
        .flat_map(|&b| std::ascii::escape_default(b))
        .map(char::from)
        .collect();
    if bytes.len() > max {
        format!("{shown}... ({} bytes)", bytes.len())
    } else {
        shown
    }
}

fn show_outcome(outcome: &Outcome) -> String {
    let status = match &outcome.status {
        Ok(code) => format!("exit {code}"),
        Err(what) => what.clone(),
    };
    format!("{status}, stdout: {}", show_bytes(&outcome.stdout, 600))
}

/// A short key that groups mismatches of the same kind for the summary.
fn signature(m: &Mismatch) -> String {
    let last = |o: &Outcome| {
        let text = String::from_utf8_lossy(&o.stdout).into_owned();
        let line = text.lines().last().unwrap_or("").to_owned();
        let kind = if line.starts_with("ERROR") {
            line
        } else {
            "events".to_owned()
        };
        format!(
            "{kind} ({})",
            o.status
                .as_ref()
                .map_or_else(Clone::clone, |c| c.to_string())
        )
    };
    format!("C {} vs Rust {}", last(&m.c), last(&m.rust))
}

/// Writes the derived inputs (not the corpus cases) to `dir`, one file each,
/// for inspecting what the generator produces.
fn dump(dir: &Path, opts: &Options, corpus: &Corpus, pools: &Pools, total: usize) -> ExitCode {
    if let Err(e) = fs::create_dir_all(dir) {
        eprintln!("bovnar-diff: {}: {e}", dir.display());
        return ExitCode::from(2);
    }
    for index in corpus.cases.len()..total {
        let case = derived_case(opts.seed, index, corpus, pools);
        let name = case.origin.replace(" #", "-");
        if let Err(e) = fs::write(dir.join(name), &case.input) {
            eprintln!("bovnar-diff: {}: {e}", dir.display());
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let Some(opts) = parse_options() else {
        return usage();
    };
    let corpus = match load_corpus(&opts.corpus, opts.examples.as_deref()) {
        Ok(corpus) => corpus,
        Err(e) => {
            eprintln!("bovnar-diff: {e}");
            return ExitCode::from(2);
        }
    };
    let pools = Pools::harvest(corpus.docs.iter().map(Vec::as_slice));
    let total = opts.count.max(corpus.cases.len());
    if let Some(dir) = &opts.dump {
        return dump(dir, &opts, &corpus, &pools, total);
    }
    let next = AtomicUsize::new(0);
    let mismatches = Mutex::new(Vec::new());
    let by_origin = Mutex::new(BTreeMap::<&str, usize>::new());
    let failures = AtomicUsize::new(0);
    let accepted = AtomicUsize::new(0);
    let start = Instant::now();

    thread::scope(|scope| {
        for _ in 0..opts.jobs {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                if index >= total {
                    break;
                }
                let case = match corpus.cases.get(index) {
                    Some(case) => case.clone(),
                    None => derived_case(opts.seed, index, &corpus, &pools),
                };
                let kind = case.origin.split(' ').next().unwrap_or("");
                let kind = if kind == "corpus" {
                    "corpus"
                } else if kind == "generated" {
                    "generated"
                } else {
                    "mutated"
                };
                *by_origin
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .entry(kind)
                    .or_default() += 1;
                let c = run(&opts.c_iut, &case);
                let rust = run(&opts.rust_iut, &case);
                if c.status.is_err() || rust.status.is_err() {
                    failures.fetch_add(1, Ordering::Relaxed);
                }
                if c.status == Ok(0) {
                    accepted.fetch_add(1, Ordering::Relaxed);
                }
                if c != rust {
                    mismatches
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(Mismatch { case, c, rust });
                }
            });
        }
    });

    let mut mismatches = mismatches.into_inner().unwrap_or_else(|e| e.into_inner());
    mismatches.sort_by_key(|m| {
        m.case
            .origin
            .split('#')
            .nth(1)
            .and_then(|n| n.parse::<usize>().ok())
            .unwrap_or(0)
    });
    let counts = by_origin.into_inner().unwrap_or_else(|e| e.into_inner());
    println!(
        "bovnar-diff: {total} inputs ({}) in {:.1}s, seed {}, {} valid per C, {} mismatches, {} IUT failures",
        counts.iter().map(|(k, v)| format!("{v} {k}")).collect::<Vec<_>>().join(", "),
        start.elapsed().as_secs_f64(),
        opts.seed,
        accepted.load(Ordering::Relaxed),
        mismatches.len(),
        failures.load(Ordering::Relaxed),
    );
    let mut kinds = BTreeMap::<String, usize>::new();
    for m in &mismatches {
        *kinds.entry(signature(m)).or_default() += 1;
    }
    for (kind, n) in &kinds {
        println!("  {n:>6}  {kind}");
    }
    for m in mismatches.iter().take(opts.show) {
        let small = minimise(&opts, m);
        let (c, rust) = (run(&opts.c_iut, &small), run(&opts.rust_iut, &small));
        println!("--- {} args {:?}", m.case.origin, small.args);
        println!("input: {}", show_bytes(&small.input, 400));
        println!("C:     {}", show_outcome(&c));
        println!("Rust:  {}", show_outcome(&rust));
    }
    if mismatches.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
