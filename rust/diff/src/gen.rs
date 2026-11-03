//! Input generation: a seeded RNG, a grammar-based document generator fed
//! with fragments harvested from the corpus, and byte-level mutations.

/// SplitMix64: tiny, seedable and good enough to drive a generator.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n`; `n` must be non-zero.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    /// True with probability `1/n`.
    pub fn one_in(&mut self, n: usize) -> bool {
        self.below(n) == 0
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }
}

/// Fragments taken from real documents, recombined by the generator.
#[derive(Default)]
pub struct Pools {
    /// Annotation family parts such as `float:64` or `uint`.
    families: Vec<String>,
    /// Other annotation parameters: bases, `q`, epochs, units.
    params: Vec<String>,
    /// Unit spellings seen inline after numbers or inside annotations.
    units: Vec<String>,
    /// Right-hand sides of one-line assignments.
    values: Vec<Vec<u8>>,
}

impl Pools {
    /// Harvests fragments from the given documents.
    pub fn harvest<'a>(docs: impl IntoIterator<Item = &'a [u8]>) -> Self {
        let mut pools = Self::default();
        for doc in docs {
            let text = String::from_utf8_lossy(doc);
            for line in text.lines() {
                pools.harvest_line(line);
            }
        }
        for list in [&mut pools.families, &mut pools.params, &mut pools.units] {
            list.sort();
            list.dedup();
        }
        pools.values.sort();
        pools.values.dedup();
        pools
    }

    fn harvest_line(&mut self, line: &str) {
        let code = line.split('#').next().unwrap_or("");
        let mut rest = code;
        while let Some(start) = rest.find('<') {
            let Some(len) = rest[start + 1..].find('>') else {
                break;
            };
            let body = &rest[start + 1..start + 1 + len];
            if !plain(body) {
                rest = &rest[start + 1 + len..];
                continue;
            }
            let mut parts = body.split(',');
            if let Some(family) = parts.next() {
                self.families.push(family.trim().to_owned());
            }
            for param in parts {
                let param = param.trim().to_owned();
                if !param.starts_with('_') && !param.starts_with('q') {
                    self.units.push(param.clone());
                }
                self.params.push(param);
            }
            rest = &rest[start + 1 + len..];
        }
        if let (Some(eq), Some(semi)) = (code.find('='), code.rfind(';')) {
            if eq < semi && !code[eq..semi].contains('{') {
                let value = code[eq + 1..semi].trim();
                if !value.is_empty() && plain(value) {
                    self.values.push(value.as_bytes().to_vec());
                }
            }
        }
    }
}

/// Text worth reusing: short and free of control characters and of the
/// replacement character that marks bytes that were not UTF-8 (octet
/// streams in the corpus).
fn plain(text: &str) -> bool {
    text.len() <= 80
        && !text
            .chars()
            .any(|c| c.is_control() || c == char::REPLACEMENT_CHARACTER)
}

const FAMILIES: &[&str] = &[
    "uint",
    "sint",
    "float",
    "float_fix",
    "float_dec",
    "bool",
    "utf8",
    "datetime",
    "int",
    "",
];
const WIDTHS: &[&str] = &[
    "8",
    "16",
    "32",
    "64",
    "128",
    "256",
    "0",
    "7",
    "65",
    "512",
    "99999999999",
];
const BASES: &[&str] = &[
    "_2", "_8", "_10", "_16", "_32", "_64", "_85", "_3", "_0", "q8", "q0", "q63", "q64",
];
const EPOCHS: &[&str] = &["unix", "tai", "gps", "utc", "ntp", "excel"];
const UNITS: &[&str] = &[
    "m",
    "k~g",
    "m/s^2",
    "m/s²",
    "kg·m·s⁻²",
    "no_unit",
    "%",
    "ΔK",
    "°C",
    "µ~m",
    "Ki~B",
    "1/s",
    "m^-1",
    "s-1",
    "N*m",
    "(m/s)",
    "unece:KGM",
    "unece:MTR",
    "unece:XBX",
    "qudt:M-PER-SEC",
    "qudt:HectoPA",
    "qudt-qk:Mass",
    "udunits:m s-1",
    "udunits:kg m-2 s-1",
    "om:kilometre",
    "ucum:mm[Hg]",
    "ucum:Cel",
    "cf:sea_surface_temperature",
    "bogus:x",
    "unece:",
    "qudt:ZZZ",
    "EUR",
    "USD",
    "BTC",
    "k~EUR",
];
const NUMBERS: &[&str] = &[
    "0",
    "1",
    "-1",
    "42",
    "-0",
    "007",
    "1.5",
    "-2.25",
    ".5",
    "5.",
    "1e3",
    "1E-3",
    "-1.5e+10",
    "0x1F",
    "0X1f",
    "-0x80",
    "0b1011",
    "0o17",
    "0x",
    "1e",
    "1e400",
    "4.9e-324",
    "255",
    "256",
    "65535",
    "65536",
    "4294967295",
    "4294967296",
    "9223372036854775807",
    "9223372036854775808",
    "-9223372036854775808",
    "-9223372036854775809",
    "18446744073709551615",
    "18446744073709551616",
    "340282366920938463463374607431768211455",
    "inf",
    "-inf",
    "nan",
    "+1",
    "1_000",
    "0.1",
    "3.14159265358979323846264338327950288",
];
const DATETIMES: &[&str] = &[
    "2026-06-15",
    "2026-06-15T12:00:00Z",
    "2026-06-15T12:00:00",
    "1970-01-01T00:00:00Z",
    "1960-01-01",
    "2016-12-31T23:59:60Z",
    "2026-02-29",
    "2024-02-29",
    "2026-13-01",
    "2026-06-15T12:00:00.123Z",
    "2026-06-15T12:00:00.123456789+02:00",
    "2026-06-15T24:00:00Z",
    "2026-06-15T12:00:00-23:59",
    "0000-01-01",
    "9999-12-31T23:59:59Z",
    "2026-06-15T12:00Z",
];
const SYMBOLS: &[&str] = &[
    "true", "false", "null", "abc", "_x", "a1", "A_B", "nullx", "Null", "TRUE",
];
const ESCAPES: &[&str] = &[
    "\\\"",
    "\\\\",
    "\\n",
    "\\t",
    "\\r",
    "\\u00e9",
    "\\U0001F600",
    "é",
    "😀",
];
const BAD_ESCAPES: &[&str] = &[
    "\\0",
    "\\x41",
    "\\xff",
    "\\uD800",
    "\\U00110000",
    "\\q",
    "\\",
    "\\f",
    "\u{7f}",
    "\\u12",
];

/// Grammar-based document generator.
pub struct Generator<'a> {
    pub pools: &'a Pools,
    out: Vec<u8>,
    depth: usize,
    /// A dirty document also uses the spellings that are always wrong; a
    /// clean one mostly stays valid, so the generator reaches deep checks.
    dirty: bool,
}

impl<'a> Generator<'a> {
    pub fn new(pools: &'a Pools) -> Self {
        Self {
            pools,
            out: Vec::new(),
            depth: 0,
            dirty: false,
        }
    }

    /// One document.
    pub fn document(&mut self, rng: &mut Rng) -> Vec<u8> {
        self.out.clear();
        self.depth = 0;
        self.dirty = rng.one_in(3);
        if rng.one_in(40) {
            self.push("\u{feff}");
        }
        if rng.one_in(3) {
            let version = if self.flaw(rng, 2) {
                *rng.pick(&["1.3", "2.0", "1", "01.2", "1.2 "])
            } else {
                *rng.pick(&["1.0", "1.1", "1.2", "1.2"])
            };
            self.push("#!bovnar ");
            self.push(version);
            self.push(if self.flaw(rng, 2) {
                rng.pick(&[" \n", ""])
            } else {
                rng.pick(&["\n", "\r\n", "\n\n"])
            });
        }
        let count = rng.below(6);
        for _ in 0..count {
            self.ws(rng);
            self.assignment(rng);
        }
        self.ws(rng);
        if self.flaw(rng, 10) {
            self.out.truncate(rng.below(self.out.len() + 1));
        }
        std::mem::take(&mut self.out)
    }

    fn push(&mut self, s: &str) {
        self.out.extend_from_slice(s.as_bytes());
    }

    fn ws(&mut self, rng: &mut Rng) {
        match rng.below(12) {
            0 => {}
            1 => self.push("\n"),
            2 => self.push("\t"),
            3 => self.push(" # comment\n"),
            4 => self.push("\r\n"),
            5 if self.dirty => self.push("\x0b"),
            _ => self.push(" "),
        }
    }

    /// True with probability `1/n`, but only in a dirty document.
    fn flaw(&self, rng: &mut Rng, n: usize) -> bool {
        self.dirty && rng.one_in(n)
    }

    fn key(&mut self, rng: &mut Rng) {
        let key = if self.flaw(rng, 4) {
            *rng.pick(&["1a", "a-b", "é", "a.b", ""])
        } else {
            *rng.pick(&[
                "a",
                "b",
                "key",
                "_k",
                "x1",
                "a_b",
                "K",
                "long_identifier_name_0123456789",
            ])
        };
        self.push(key);
        if rng.one_in(4) {
            let n = rng.below(1000).to_string();
            self.push(&n);
        }
    }

    fn assignment(&mut self, rng: &mut Rng) {
        self.push(if self.flaw(rng, 20) { "" } else { "." });
        self.key(rng);
        self.ws(rng);
        self.push(if self.flaw(rng, 20) { ":" } else { "=" });
        self.ws(rng);
        self.value(rng);
        self.ws(rng);
        if !self.flaw(rng, 15) {
            self.push(";");
        }
    }

    fn annotation(&mut self, rng: &mut Rng) {
        self.push("<");
        if !self.pools.families.is_empty() && !self.flaw(rng, 2) {
            let family = rng.pick(&self.pools.families).clone();
            self.push(&family);
        } else {
            self.push(rng.pick(FAMILIES));
            if rng.one_in(2) {
                self.push(":");
                self.push(rng.pick(WIDTHS));
            }
        }
        for _ in 0..rng.below(if self.dirty { 3 } else { 2 }) {
            self.push(rng.pick(&[",", ", ", ","]));
            match rng.below(if self.dirty { 4 } else { 3 }) + usize::from(!self.dirty) {
                0 => self.push(rng.pick(BASES)),
                1 => self.push(rng.pick(EPOCHS)),
                2 if !self.pools.params.is_empty() => {
                    let param = rng.pick(&self.pools.params).clone();
                    self.push(&param);
                }
                _ => self.unit(rng),
            }
        }
        self.push(">");
        self.ws(rng);
    }

    fn unit(&mut self, rng: &mut Rng) {
        if !self.pools.units.is_empty() && rng.one_in(2) {
            let unit = rng.pick(&self.pools.units).clone();
            self.push(&unit);
        } else {
            self.push(rng.pick(UNITS));
        }
    }

    fn value(&mut self, rng: &mut Rng) {
        if rng.one_in(3) {
            self.annotation(rng);
        }
        let nested = self.depth < 6;
        match rng.below(if nested { 13 } else { 10 }) {
            0 | 1 => {
                self.push(rng.pick(NUMBERS));
                if rng.one_in(4) {
                    self.push(rng.pick(&[" ", "", "  "]));
                    self.unit(rng);
                }
            }
            2 => self.push(rng.pick(DATETIMES)),
            3 => self.push(rng.pick(SYMBOLS)),
            4 | 5 => self.string(rng),
            6 => self.reference(rng),
            7 => self.octets(rng),
            8 | 9 if !self.pools.values.is_empty() => {
                let value = rng.pick(&self.pools.values).clone();
                self.out.extend_from_slice(&value);
            }
            8 | 9 => self.push("null"),
            10 | 11 => self.array(rng),
            _ => self.structure(rng),
        }
    }

    fn string(&mut self, rng: &mut Rng) {
        for i in 0..1 + usize::from(rng.one_in(5)) {
            if i > 0 {
                self.ws(rng);
            }
            self.push("\"");
            for _ in 0..rng.below(8) {
                if self.flaw(rng, 3) {
                    self.push(rng.pick(BAD_ESCAPES));
                } else if rng.one_in(3) {
                    self.push(rng.pick(ESCAPES));
                } else {
                    self.push(rng.pick(&["a", "b", " ", "xyz", "0", "#", ";", "<", "\t"]));
                }
            }
            if !self.flaw(rng, 20) {
                self.push("\"");
            }
        }
    }

    fn reference(&mut self, rng: &mut Rng) {
        self.push("&");
        self.key(rng);
        for _ in 0..rng.below(3) {
            if rng.one_in(2) {
                self.push(".");
                self.key(rng);
            } else {
                self.push("[");
                self.push(rng.pick(&["0", "1", "10", "-1", "99999999999999999999", ""]));
                self.push("]");
            }
        }
    }

    fn octets(&mut self, rng: &mut Rng) {
        self.out.push(0);
        for _ in 0..rng.below(4) {
            let len = *rng.pick(&[1usize, 2, 3, 5, 16, 0]);
            self.out.push(1);
            self.out.extend_from_slice(&(len as u16).to_le_bytes());
            for _ in 0..len {
                self.out.push(rng.next() as u8);
            }
        }
        if self.flaw(rng, 5) {
            self.out.push(*rng.pick(&[2u8, 1, 0xff]));
        }
        self.out.push(0);
    }

    fn array(&mut self, rng: &mut Rng) {
        self.depth += 1;
        let rows = 1 + rng.below(3);
        let width = rng.below(4);
        for row in 0..rows {
            if row > 0 {
                self.ws(rng);
                self.push("/");
                self.ws(rng);
            }
            self.push("[");
            let items = if self.flaw(rng, 3) {
                rng.below(5)
            } else {
                width
            };
            for i in 0..items {
                if i > 0 {
                    self.push(rng.pick(&[",", ", ", " , "]));
                }
                if rng.one_in(3) {
                    self.value(rng);
                } else if rng.one_in(2) {
                    self.push(rng.pick(NUMBERS));
                } else {
                    self.string(rng);
                }
            }
            if self.flaw(rng, 10) {
                self.push(",");
            }
            self.push("]");
        }
        self.depth -= 1;
    }

    fn structure(&mut self, rng: &mut Rng) {
        self.depth += 1;
        self.push("{");
        for _ in 0..rng.below(4) {
            self.ws(rng);
            self.assignment(rng);
        }
        self.ws(rng);
        self.push("}");
        self.depth -= 1;
    }
}

/// Bytes worth splicing into a mutated input.
const SPLICES: &[&[u8]] = &[
    b";",
    b".",
    b"=",
    b"<",
    b">",
    b"{",
    b"}",
    b"[",
    b"]",
    b"/",
    b",",
    b"&",
    b"\"",
    b"\\",
    b"#",
    b"\n",
    b"\0",
    b"\x01",
    b"\xff",
    b"\xc3\xa9",
    b"\xef\xbb\xbf",
    b"#!bovnar 1.2\n",
    b"null",
    b"<uint:8>",
    b"e",
    b"-",
    b"0x",
    b"k~",
    b"unece:",
    b"\\u",
    b"T",
    b"Z",
    b":",
];

/// One random byte-level mutation of `input`, a few edits deep.
pub fn mutate(rng: &mut Rng, input: &[u8]) -> Vec<u8> {
    let mut out = input.to_vec();
    for _ in 0..1 + rng.below(4) {
        let pos = rng.below(out.len() + 1);
        match rng.below(7) {
            0 if pos < out.len() => {
                out.remove(pos);
            }
            1 if pos < out.len() => out[pos] = rng.next() as u8,
            2 if pos < out.len() => out[pos] ^= 1 << rng.below(8),
            3 => {
                let splice = rng.pick(SPLICES);
                out.splice(pos..pos, splice.iter().copied());
            }
            4 if pos < out.len() => {
                let end = (pos + 1 + rng.below(16)).min(out.len());
                let copy = out[pos..end].to_vec();
                let at = rng.below(out.len() + 1);
                out.splice(at..at, copy);
            }
            5 => out.truncate(pos),
            _ => out.insert(pos, rng.next() as u8),
        }
    }
    out
}
