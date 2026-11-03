//! A minimal arbitrary-precision unsigned integer.
//!
//! Only what the range checks need: multiply-add by a small factor, shifts,
//! comparison, division and decimal rendering. Callers bound every operand
//! (the C reference caps its big integers at 32768 bits), so the simple
//! quadratic algorithms here stay fast.

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Unsigned big integer, little-endian 32-bit limbs, never with a zero top
/// limb (zero is the empty vector).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Big {
    limbs: Vec<u32>,
}

impl Big {
    /// The value zero.
    pub(super) fn zero() -> Self {
        Self::default()
    }

    /// A big integer holding `v`.
    pub(super) fn from_u64(v: u64) -> Self {
        let mut b = Self {
            limbs: vec![v as u32, (v >> 32) as u32],
        };
        b.normalize();
        b
    }

    /// `2^n`.
    pub(super) fn pow2(n: u64) -> Self {
        let mut b = Self::from_u64(1);
        b.shl(n);
        b
    }

    fn normalize(&mut self) {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
    }

    pub(super) fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// Number of significant bits (0 for zero).
    pub(super) fn bit_len(&self) -> u64 {
        match self.limbs.last() {
            None => 0,
            Some(&top) => (self.limbs.len() as u64 - 1) * 32 + u64::from(32 - top.leading_zeros()),
        }
    }

    /// Bit `i` (0 = least significant).
    pub(super) fn bit(&self, i: u64) -> bool {
        let Ok(word) = usize::try_from(i / 32) else {
            return false;
        };
        self.limbs
            .get(word)
            .is_some_and(|&l| (l >> (i % 32)) & 1 == 1)
    }

    /// Whether any bit below position `n` is set.
    pub(super) fn any_bit_below(&self, n: u64) -> bool {
        let full = usize::try_from(n / 32).unwrap_or(usize::MAX);
        let rest = (n % 32) as u32;
        if self.limbs.iter().take(full).any(|&l| l != 0) {
            return true;
        }
        rest != 0
            && self
                .limbs
                .get(full)
                .is_some_and(|&l| l & ((1u32 << rest) - 1) != 0)
    }

    /// `self = self * m + a`.
    pub(super) fn mul_add_small(&mut self, m: u32, a: u32) {
        let mut carry = u64::from(a);
        for l in &mut self.limbs {
            let p = u64::from(*l) * u64::from(m) + carry;
            *l = p as u32;
            carry = p >> 32;
        }
        if carry != 0 {
            self.limbs.push(carry as u32);
        }
        self.normalize();
    }

    /// `self += a`.
    pub(super) fn add_small(&mut self, a: u32) {
        self.mul_add_small(1, a);
    }

    /// `self -= 1`; zero stays zero.
    pub(super) fn dec(&mut self) {
        for l in &mut self.limbs {
            let (v, borrow) = l.overflowing_sub(1);
            *l = v;
            if !borrow {
                break;
            }
        }
        self.normalize();
    }

    /// `self <<= bits`. Callers keep `bits` small (a few ten thousand).
    pub(super) fn shl(&mut self, bits: u64) {
        if self.is_zero() || bits == 0 {
            return;
        }
        let words = usize::try_from(bits / 32).unwrap_or(usize::MAX);
        let bs = (bits % 32) as u32;
        if bs != 0 {
            let mut carry = 0u32;
            for l in &mut self.limbs {
                let next = *l >> (32 - bs);
                *l = (*l << bs) | carry;
                carry = next;
            }
            if carry != 0 {
                self.limbs.push(carry);
            }
        }
        if words != 0 {
            let mut v = vec![0u32; words];
            v.extend_from_slice(&self.limbs);
            self.limbs = v;
        }
    }

    /// `self >>= bits`.
    pub(super) fn shr(&mut self, bits: u64) {
        let words = usize::try_from(bits / 32).unwrap_or(usize::MAX);
        if words >= self.limbs.len() {
            self.limbs.clear();
            return;
        }
        self.limbs.drain(..words);
        let bs = (bits % 32) as u32;
        if bs != 0 {
            let mut carry = 0u32;
            for l in self.limbs.iter_mut().rev() {
                let next = *l << (32 - bs);
                *l = (*l >> bs) | carry;
                carry = next;
            }
        }
        self.normalize();
    }

    /// Divides in place by `d` (non-zero) and returns the remainder.
    pub(super) fn div_rem_small(&mut self, d: u32) -> u32 {
        if d == 0 {
            return 0;
        }
        let mut rem = 0u64;
        for l in self.limbs.iter_mut().rev() {
            let cur = (rem << 32) | u64::from(*l);
            *l = (cur / u64::from(d)) as u32;
            rem = cur % u64::from(d);
        }
        self.normalize();
        rem as u32
    }

    /// Quotient and remainder of `self / d` (Knuth, TAOCP vol. 2, 4.3.1 D).
    /// Division by zero yields `(0, 0)`; no caller divides by zero.
    pub(super) fn div_rem(&self, d: &Self) -> (Self, Self) {
        if d.is_zero() {
            return (Self::zero(), Self::zero());
        }
        if self.cmp(d) == Ordering::Less {
            return (Self::zero(), self.clone());
        }
        if let [single] = d.limbs[..] {
            let mut q = self.clone();
            let r = q.div_rem_small(single);
            return (q, Self::from_u64(u64::from(r)));
        }
        // Normalise so the divisor's top bit is set: the quotient-digit
        // estimate is then off by at most two.
        let s = u64::from(d.limbs.last().map_or(0, |t| t.leading_zeros()));
        let mut v = d.clone();
        v.shl(s);
        let mut un = self.clone();
        un.shl(s);
        let n = v.limbs.len();
        let mut u = un.limbs;
        u.resize(self.limbs.len() + 1, 0);
        let m = u.len() - n - 1;
        let v = v.limbs;
        let vn1 = u64::from(v[n - 1]);
        let vn2 = u64::from(v[n - 2]);
        const B: u64 = 1 << 32;
        let mut q = vec![0u32; m + 1];
        for j in (0..=m).rev() {
            let num = (u64::from(u[j + n]) << 32) | u64::from(u[j + n - 1]);
            let mut qhat = num / vn1;
            let mut rhat = num % vn1;
            while qhat >= B || qhat * vn2 > ((rhat << 32) | u64::from(u[j + n - 2])) {
                qhat -= 1;
                rhat += vn1;
                if rhat >= B {
                    break;
                }
            }
            let mut borrow = 0i64;
            let mut carry = 0u64;
            for i in 0..n {
                let p = qhat * u64::from(v[i]) + carry;
                carry = p >> 32;
                let t = i64::from(u[i + j]) - borrow - (p & 0xffff_ffff) as i64;
                u[i + j] = t as u32;
                borrow = i64::from(t < 0);
            }
            let t = i64::from(u[j + n]) - borrow - carry as i64;
            u[j + n] = t as u32;
            if t < 0 {
                // The estimate was one too large: add the divisor back.
                qhat -= 1;
                let mut c = 0u64;
                for i in 0..n {
                    let sum = u64::from(u[i + j]) + u64::from(v[i]) + c;
                    u[i + j] = sum as u32;
                    c = sum >> 32;
                }
                u[j + n] = u[j + n].wrapping_add(c as u32);
            }
            q[j] = qhat as u32;
        }
        let mut quot = Self { limbs: q };
        quot.normalize();
        u.truncate(n);
        let mut rem = Self { limbs: u };
        rem.normalize();
        rem.shr(s);
        (quot, rem)
    }

    /// Decimal digits (ASCII), most significant first; `"0"` for zero.
    pub(super) fn to_decimal(&self) -> Vec<u8> {
        if self.is_zero() {
            return vec![b'0'];
        }
        let mut t = self.clone();
        let mut chunks = Vec::new();
        while !t.is_zero() {
            chunks.push(t.div_rem_small(1_000_000_000));
        }
        let mut out = Vec::with_capacity(chunks.len() * 9);
        for (i, c) in chunks.iter().rev().enumerate() {
            let mut buf = [0u8; 9];
            let mut c = *c;
            for b in buf.iter_mut().rev() {
                *b = b'0' + (c % 10) as u8;
                c /= 10;
            }
            let digits: &[u8] = if i == 0 {
                let lead = buf.iter().take_while(|&&b| b == b'0').count();
                buf.get(lead..).unwrap_or_default()
            } else {
                &buf
            };
            out.extend_from_slice(digits);
        }
        out
    }
}

impl PartialOrd for Big {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Big {
    fn cmp(&self, other: &Self) -> Ordering {
        self.limbs
            .len()
            .cmp(&other.limbs.len())
            .then_with(|| self.limbs.iter().rev().cmp(other.limbs.iter().rev()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministic pseudo-random numbers (xorshift64*).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }
        fn big(&mut self, max_limbs: u64) -> Big {
            let n = self.next() % max_limbs + 1;
            let mut b = Big::zero();
            for _ in 0..n {
                let r = self.next();
                // Mix in runs of all-ones and zeros to hit the add-back path.
                let limb = match r % 4 {
                    0 => u32::MAX,
                    1 => 0,
                    _ => (r >> 32) as u32,
                };
                b.shl(32);
                b.add_small(limb);
            }
            b
        }
    }

    /// Reference division: one quotient bit at a time.
    fn slow_div(a: &Big, d: &Big) -> (Big, Big) {
        let mut q = Big::zero();
        let mut r = Big::zero();
        for i in (0..a.bit_len()).rev() {
            r.shl(1);
            r.add_small(u32::from(a.bit(i)));
            q.shl(1);
            if r >= *d {
                // r - d via r = r - d computed as (r) with bitwise long subtraction.
                r = sub(&r, d);
                q.add_small(1);
            }
        }
        (q, r)
    }

    fn sub(a: &Big, b: &Big) -> Big {
        let mut out = Vec::new();
        let mut borrow = 0i64;
        for (i, &x) in a.limbs.iter().enumerate() {
            let y = b.limbs.get(i).copied().unwrap_or(0);
            let t = i64::from(x) - i64::from(y) - borrow;
            out.push(t as u32);
            borrow = i64::from(t < 0);
        }
        let mut r = Big { limbs: out };
        r.normalize();
        r
    }

    #[test]
    fn division_matches_bitwise_reference() {
        let mut rng = Rng(0x1234_5678_9abc_def1);
        for _ in 0..400 {
            let a = rng.big(12);
            let d = rng.big(6);
            if d.is_zero() {
                continue;
            }
            assert_eq!(a.div_rem(&d), slow_div(&a, &d));
        }
    }

    #[test]
    fn decimal_rendering() {
        assert_eq!(Big::zero().to_decimal(), b"0");
        assert_eq!(Big::from_u64(1_000_000_000).to_decimal(), b"1000000000");
        let mut m = Big::pow2(128);
        m.dec();
        assert_eq!(
            m.to_decimal(),
            b"340282366920938463463374607431768211455".as_slice()
        );
    }

    #[test]
    fn shifts_and_bits() {
        let mut b = Big::from_u64(0b1011);
        b.shl(70);
        assert_eq!(b.bit_len(), 74);
        assert!(b.bit(70) && b.bit(71) && !b.bit(72) && b.bit(73));
        assert!(!b.any_bit_below(70));
        assert!(b.any_bit_below(71));
        b.shr(71);
        assert_eq!(b, Big::from_u64(0b101));
    }
}
