//! An exact decimal tie must round half to even, as IEEE 754 requires.
//!
//! The decimal-interchange helpers used to encode the parsed `bvn_float`, a
//! value already rounded into binary; a decimal tie is in general not exact
//! in binary, so the decimal rounding followed whichever side the binary
//! landed on. They now go straight from the literal (`bvn_float_strtodec`).
//!
//! The ties come from `Mt19937` below (MT19937 seeded as below, with
//! `random()` and `randrange()` draws) with seed 3, so the 2000 literals are
//! reproducible. The expected value is `round_half_even` to 16 significant
//! digits (the exponents stay far from any limit).

use bovnar::{Decimal, Quantity, Value, parse_typed};
use num_bigint::BigUint;
use num_integer::Integer;

/// An MT19937 generator with `random()` and `randrange()` draws, as far as
/// `ties` uses it.
struct Mt19937 {
    mt: [u32; 624],
    index: usize,
}

impl Mt19937 {
    /// `random.Random(seed)` for a small non-negative int: `init_by_array`
    /// over its 32-bit words.
    fn new(seed: u32) -> Self {
        let mut r = Mt19937 {
            mt: [0; 624],
            index: 624,
        };
        r.init_genrand(19_650_218);
        let key = [seed];
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..624.max(key.len()) {
            let prev = r.mt[i - 1] ^ (r.mt[i - 1] >> 30);
            r.mt[i] = (r.mt[i] ^ prev.wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..623 {
            let prev = r.mt[i - 1] ^ (r.mt[i - 1] >> 30);
            r.mt[i] = (r.mt[i] ^ prev.wrapping_mul(1_566_083_941)).wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                r.mt[0] = r.mt[623];
                i = 1;
            }
        }
        r.mt[0] = 0x8000_0000;
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..624 {
            let prev = self.mt[i - 1] ^ (self.mt[i - 1] >> 30);
            self.mt[i] = 1_812_433_253u32.wrapping_mul(prev).wrapping_add(i as u32);
        }
        self.index = 624;
    }

    fn genrand_u32(&mut self) -> u32 {
        if self.index >= 624 {
            for k in 0..624 {
                let y = (self.mt[k] & 0x8000_0000) | (self.mt[(k + 1) % 624] & 0x7fff_ffff);
                let mag = if y & 1 == 1 { 0x9908_b0df } else { 0 };
                self.mt[k] = self.mt[(k + 397) % 624] ^ (y >> 1) ^ mag;
            }
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `random()`: 53 random bits in [0, 1).
    fn random(&mut self) -> f64 {
        let a = f64::from(self.genrand_u32() >> 5);
        let b = f64::from(self.genrand_u32() >> 6);
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `getrandbits(k)` for `k <= 64`.
    fn getrandbits(&mut self, k: u32) -> u64 {
        if k <= 32 {
            return u64::from(self.genrand_u32() >> (32 - k));
        }
        let lo = u64::from(self.genrand_u32());
        let hi = u64::from(self.genrand_u32() >> (64 - k));
        lo | (hi << 32)
    }

    /// `randrange(start, stop)`.
    fn randrange(&mut self, start: i64, stop: i64) -> i64 {
        let n = (stop - start) as u64;
        let k = 64 - n.leading_zeros();
        let mut r = self.getrandbits(k);
        while r >= n {
            r = self.getrandbits(k);
        }
        start + r as i64
    }
}

/// Literals with a 17-digit coefficient ending in 5: exactly half a unit in
/// decimal64's last kept place.
fn ties(n: usize, seed: u32) -> Vec<String> {
    let mut rnd = Mt19937::new(seed);
    (0..n)
        .map(|_| {
            let coeff = rnd.randrange(10i64.pow(15), 10i64.pow(16)) * 10 + 5;
            let sign = if rnd.random() < 0.5 { "-" } else { "" };
            format!("{sign}{coeff}e{}", rnd.randrange(-60, 60))
        })
        .collect()
}

/// `Context(prec=prec, rounding=ROUND_HALF_EVEN).create_decimal(d)` for a
/// finite `d` well inside the context's exponent range.
fn round_half_even(d: &Decimal, prec: usize) -> Decimal {
    let (coeff, exp) = (d.coefficient().unwrap(), d.exponent().unwrap());
    let digits = coeff.to_str_radix(10).len();
    if digits <= prec {
        return d.clone();
    }
    let drop = (digits - prec) as u32;
    let factor = BigUint::from(10u32).pow(drop);
    let (mut q, r) = coeff.div_rem(&factor);
    let twice = r * 2u32;
    if twice > factor || (twice == factor && q.is_odd()) {
        q += 1u32;
    }
    let mut exp = exp + i64::from(drop);
    if q.to_str_radix(10).len() > prec {
        q /= 10u32;
        exp += 1;
    }
    Decimal::new(d.is_signed(), q, exp)
}

fn stored_value(lit: &str) -> Decimal {
    let doc = format!("#!bovnar 1.1\n.v = <float_dec:64> {lit};\n");
    let Value::Struct(fields) = parse_typed(doc.as_bytes()).unwrap() else {
        panic!("not a struct")
    };
    let Value::Quantity(q) = &fields[0].1 else {
        panic!("not a quantity")
    };
    let q: &Quantity = q;
    q.stored_value().unwrap().unwrap()
}

#[test]
fn test_exact_decimal64_ties_round_half_to_even() {
    let lits = ties(2000, 3);
    // Seed 3's first draws pin the generator above to a known sequence.
    assert_eq!(lits[0], "63380354856222695e-13");
    let mut bad = Vec::new();
    let mut n = 0;
    for lit in lits {
        let q = stored_value(&lit);
        n += 1;
        let want = round_half_even(&lit.parse().unwrap(), 16);
        if q != want {
            bad.push((lit, q.to_string(), want.to_string()));
        }
    }
    assert!(n >= 2000);
    assert!(
        bad.is_empty(),
        "{}/{n} ties rounded wrong, first: {:?}",
        bad.len(),
        &bad[..bad.len().min(3)]
    );
}

/// Spelled out so a regression names itself rather than reporting a rate.
#[test]
fn test_named_tie_cases() {
    let cases = [
        ("1.0000000000000015", "1.000000000000002"), // last kept digit odd -> up
        ("1.0000000000000025", "1.000000000000002"), // even -> stays
        ("52906102648424555e-10", "5290610.264842456"),
        ("57123991439313165e-11", "571239.9143931316"),
    ];
    for (lit, want) in cases {
        let got = stored_value(lit);
        assert_eq!(
            got,
            want.parse::<Decimal>().unwrap(),
            "{lit} -> {got}, want {want}"
        );
    }
}
