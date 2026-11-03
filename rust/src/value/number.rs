//! Lossless numbers: the literal as written plus its effective type.

use alloc::format;
use alloc::string::{String, ToString};

use crate::num;
use crate::types::{TypeFamily, TypeSpec};

/// A number exactly as the document wrote it.
///
/// The literal text is kept verbatim together with its effective type
/// ([`TypeSpec`], including width and base) and unit, so nothing is lost
/// between reading and writing. The typed accessors convert on demand and
/// return `None` when the value does not fit, never a rounded or truncated
/// result (except [`Number::as_f64`], which is a float conversion by nature).
///
/// ```
/// use bovnar::Number;
///
/// let n = Number::from(300u64);
/// assert_eq!(n.as_u64(), Some(300));
/// assert_eq!(n.as_i64(), Some(300));
/// assert_eq!(n.as_f64(), Some(300.0));
/// assert_eq!(n.text(), "300");
/// ```
#[derive(Debug, Clone)]
pub struct Number {
    pub(crate) spec: TypeSpec,
    pub(crate) text: String,
    pub(crate) unit: Option<String>,
    pub(crate) frac: String,
    pub(crate) quoted: bool,
    pub(crate) inline_unit: bool,
}

impl Number {
    /// A number with an explicit type and literal text (digits in the type's
    /// base, as a document would spell them).
    ///
    /// The text is not checked here; the writer refuses a literal that does
    /// not read back as the same value.
    ///
    /// ```
    /// use bovnar::{Number, TypeFamily, TypeSpec};
    ///
    /// let spec = TypeSpec { family: TypeFamily::Uint, width: 32, base: 16 };
    /// let n = Number::new(spec, "ff");
    /// assert_eq!(n.as_u64(), Some(255));
    /// ```
    pub fn new(spec: TypeSpec, text: impl Into<String>) -> Self {
        let text = text.into();
        // Above base 10 digits can be letters, which only a string literal
        // can carry; plain decimal digits stay bare as the C writer has them.
        let digits = text.strip_prefix('-').unwrap_or(&text);
        let quoted = spec.effective_base() > 10
            && !num::is_special_number(text.as_bytes())
            && (digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()));
        Self {
            spec,
            text,
            unit: None,
            frac: String::new(),
            quoted,
            inline_unit: false,
        }
    }

    /// The same number with a unit (`m`, `k~g`, `$USD`, ...).
    ///
    /// ```
    /// use bovnar::Number;
    ///
    /// let n = Number::from(9.81).with_unit("m/s2");
    /// assert_eq!(n.unit(), Some("m/s2"));
    /// ```
    #[must_use]
    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        let unit = unit.into();
        self.unit = (!unit.is_empty()).then_some(unit);
        self.inline_unit = false;
        self
    }

    /// The effective type: family, width and base (or `q`, or epoch).
    pub fn spec(&self) -> TypeSpec {
        self.spec
    }

    /// The literal as written (for a datetime: whole epoch seconds).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The unit as the document spelled it, if any.
    pub fn unit(&self) -> Option<&str> {
        self.unit.as_deref()
    }

    /// The verbatim sub-second digits of a datetime written as an ISO-8601
    /// literal (without the `.`), if it had any.
    pub fn fraction(&self) -> Option<&str> {
        (!self.frac.is_empty()).then_some(self.frac.as_str())
    }

    /// Whether the literal was written as a quoted string (`<uint:8,_16> "ff"`).
    pub fn is_quoted(&self) -> bool {
        self.quoted
    }

    /// Whether this is a datetime value.
    pub fn is_datetime(&self) -> bool {
        self.spec.family == TypeFamily::Datetime
    }

    /// Whether the literal is an integer (as opposed to a float, a fraction
    /// or a special value like `nan`), following the C reference's rule: the
    /// integer families always hold integers, others only when the text has
    /// no `.`, `e` or `E`.
    pub fn is_integer(&self) -> bool {
        let t = self.text.as_bytes();
        match self.spec.family {
            TypeFamily::Float | TypeFamily::FloatFix | TypeFamily::FloatDec => false,
            TypeFamily::Uint | TypeFamily::Sint => self.quoted || !num::is_special_number(t),
            _ => !num::is_special_number(t) && !t.iter().any(|b| matches!(b, b'.' | b'e' | b'E')),
        }
    }

    /// The value as `u64`, if it is an integer that fits.
    pub fn as_u64(&self) -> Option<u64> {
        let (negative, magnitude) = self.integer()?;
        if negative && magnitude != 0 {
            return None;
        }
        Some(magnitude)
    }

    /// The value as `i64`, if it is an integer that fits. For a datetime this
    /// is the whole epoch seconds.
    pub fn as_i64(&self) -> Option<i64> {
        let (negative, magnitude) = self.integer()?;
        if negative {
            0i64.checked_sub_unsigned(magnitude)
        } else {
            i64::try_from(magnitude).ok()
        }
    }

    /// The value as `f64`, rounded to the nearest double where needed.
    ///
    /// `None` for a literal this crate does not convert: a float in a base
    /// other than 10.
    pub fn as_f64(&self) -> Option<f64> {
        match self.text.as_str() {
            "nan" => return Some(f64::NAN),
            "inf" => return Some(f64::INFINITY),
            "ninf" => return Some(f64::NEG_INFINITY),
            _ => {}
        }
        if self.is_integer() {
            if let Some((negative, magnitude)) = self.integer() {
                #[allow(clippy::cast_precision_loss)]
                let v = magnitude as f64;
                return Some(if negative { -v } else { v });
            }
        }
        if self.spec.effective_base() != 10 {
            return None;
        }
        self.text.parse::<f64>().ok()
    }

    /// Sign and magnitude of an integer literal in its base.
    fn integer(&self) -> Option<(bool, u64)> {
        if !self.is_integer() {
            return None;
        }
        let base = self.spec.effective_base();
        let bytes = self.text.as_bytes();
        let signed = base != 64 && base != 85;
        let (negative, digits) = match bytes.split_first() {
            Some((b'-', rest)) if signed => (true, rest),
            Some((b'+', rest)) if signed => (false, rest),
            _ => (false, bytes),
        };
        if digits.is_empty() {
            return None;
        }
        let mut value: u64 = 0;
        for &c in digits {
            let d = num::char_to_digit(c, base);
            if d >= base {
                return None;
            }
            value = value
                .checked_mul(u64::from(base))?
                .checked_add(u64::from(d))?;
        }
        Some((negative, value))
    }

    /// Whether the reader would synthesise exactly this number's type for
    /// its text (spec §6.6), so that no annotation is needed to write it.
    pub(crate) fn is_default_typed(&self) -> bool {
        if self.quoted || !self.frac.is_empty() {
            return false;
        }
        let t = self.text.as_bytes();
        let float_like =
            num::is_special_number(t) || t.iter().any(|b| matches!(b, b'.' | b'e' | b'E'));
        let want = if float_like {
            TypeFamily::Float
        } else if t.first() == Some(&b'-') {
            TypeFamily::Sint
        } else {
            TypeFamily::Uint
        };
        self.spec.family == want
            && matches!(self.spec.width, 0 | 64)
            && matches!(self.spec.base, 0 | 10)
            && self.unit.is_none()
    }
}

/// Numbers are equal when they are written the same under the same
/// effective type: an unset width or base equals its default spelled out
/// (`<float>` and `<float:64,_10>` are one type).
impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        spec_key(self.spec) == spec_key(other.spec)
            && self.text == other.text
            && self.unit == other.unit
            && self.frac == other.frac
            && self.quoted == other.quoted
            && self.inline_unit == other.inline_unit
    }
}

/// The parts of a spec that change what a literal means.
fn spec_key(spec: TypeSpec) -> (TypeFamily, u32, u32, u32) {
    let raw_base = if spec.family == TypeFamily::Datetime {
        spec.base
    } else {
        0
    };
    (
        spec.family,
        spec.effective_width(),
        spec.effective_base(),
        spec.effective_q().max(raw_base),
    )
}

macro_rules! number_from_unsigned {
    ($($t:ty => $w:expr),*) => {$(
        impl From<$t> for Number {
            fn from(v: $t) -> Self {
                Number::new(TypeSpec { family: TypeFamily::Uint, width: $w, base: 0 }, v.to_string())
            }
        }
    )*};
}

macro_rules! number_from_signed {
    ($($t:ty => $w:expr),*) => {$(
        impl From<$t> for Number {
            fn from(v: $t) -> Self {
                Number::new(TypeSpec { family: TypeFamily::Sint, width: $w, base: 0 }, v.to_string())
            }
        }
    )*};
}

number_from_unsigned!(u8 => 8, u16 => 16, u32 => 32, u64 => 64);
number_from_signed!(i8 => 8, i16 => 16, i32 => 32, i64 => 64);

impl From<f64> for Number {
    fn from(v: f64) -> Self {
        Number::new(
            TypeSpec {
                family: TypeFamily::Float,
                width: 64,
                base: 0,
            },
            float_text(v),
        )
    }
}

impl From<f32> for Number {
    fn from(v: f32) -> Self {
        let text = if v.is_finite() {
            format!("{v:?}")
        } else {
            float_text(f64::from(v))
        };
        Number::new(
            TypeSpec {
                family: TypeFamily::Float,
                width: 32,
                base: 0,
            },
            text,
        )
    }
}

/// The shortest literal that reads back as `v`, always with a `.` or an
/// exponent so that it reads as a float.
fn float_text(v: f64) -> String {
    if v.is_nan() {
        "nan".to_string()
    } else if v == f64::INFINITY {
        "inf".to_string()
    } else if v == f64::NEG_INFINITY {
        "ninf".to_string()
    } else {
        format!("{v:?}")
    }
}
