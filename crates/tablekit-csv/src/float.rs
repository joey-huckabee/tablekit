//! Floats as pandas writes them: the shortest digits that read back as the same value, laid
//! out as NumPy's `str` does.
//!
//! | | positional when | otherwise |
//! |---|---|---|
//! | `f64` | `1e-4 <= abs < 1e16`, or zero | scientific |
//! | `f32` | `1e-4 <= abs < 1e6`, or zero | scientific |
//!
//! Positional always has a decimal point (`123.0`, `0.0001`); scientific has one only with
//! more than one digit, and an exponent with a sign and at least two digits (`1e-05`,
//! `1.2345679e+08`, `1e-300`). Checked against NumPy's output in `testdata/floats/`.

use std::io::Write as _;

/// Below this magnitude, scientific notation.
const POSITIONAL_MIN: f64 = 1e-4;
/// From this magnitude up, scientific notation: NumPy's cutoffs.
const POSITIONAL_MAX_F64: f64 = 1e16;
const POSITIONAL_MAX_F32: f64 = 1e6;

/// Appends finite `v` to `out`; `scratch` is reused between calls so nothing is allocated.
// HOT PATH: per value.
pub(crate) fn write_f64(out: &mut Vec<u8>, scratch: &mut Vec<u8>, v: f64) {
    scratch.clear();
    let _ = write!(scratch, "{v:e}");
    let digits = mantissa_digits(scratch);
    if digits > 1 {
        // Rust's shortest digits break a tie between two equally short, equally close
        // candidates upward; Python and NumPy break it to the even digit, as Rust's
        // fixed-precision formatting does. Format again to that many digits, and keep it
        // if it reads back as `v` (the closest such number always should).
        let len = scratch.len();
        let _ = write!(scratch, "{:.*e}", digits - 1, v);
        let same = std::str::from_utf8(&scratch[len..])
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .is_some_and(|back| back.to_bits() == v.to_bits());
        if same {
            scratch.drain(..len);
        } else {
            scratch.truncate(len);
        }
    }
    lay_out(out, scratch, positional(v.abs(), POSITIONAL_MAX_F64));
}

/// Appends finite `v` to `out`, with `f32`'s shortest digits and cutoffs.
// HOT PATH: per value.
pub(crate) fn write_f32(out: &mut Vec<u8>, scratch: &mut Vec<u8>, v: f32) {
    scratch.clear();
    let _ = write!(scratch, "{v:e}");
    let digits = mantissa_digits(scratch);
    if digits > 1 {
        // As in `write_f64`.
        let len = scratch.len();
        let _ = write!(scratch, "{:.*e}", digits - 1, v);
        let same = std::str::from_utf8(&scratch[len..])
            .ok()
            .and_then(|s| s.parse::<f32>().ok())
            .is_some_and(|back| back.to_bits() == v.to_bits());
        if same {
            scratch.drain(..len);
        } else {
            scratch.truncate(len);
        }
    }
    lay_out(
        out,
        scratch,
        positional(f64::from(v.abs()), POSITIONAL_MAX_F32),
    );
}

/// The number of significant digits in Rust's `{:e}` form (`-1.25e3` has 3).
fn mantissa_digits(sci: &[u8]) -> usize {
    sci.iter()
        .take_while(|&&b| b != b'e')
        .filter(|b| b.is_ascii_digit())
        .count()
}

fn positional(abs: f64, max: f64) -> bool {
    abs == 0.0 || (POSITIONAL_MIN..max).contains(&abs)
}

/// Rewrites Rust's `{:e}` form (`-1.2345e-7`: sign, digits with an optional point, `e`,
/// exponent) in NumPy's layout.
// HOT PATH: per value.
fn lay_out(out: &mut Vec<u8>, sci: &[u8], positional: bool) {
    let (negative, sci) = match sci.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, sci),
    };
    let e_at = sci.iter().position(|&b| b == b'e').unwrap_or(sci.len());
    let (mantissa, exponent) = sci.split_at(e_at);
    let exponent = parse_exponent(exponent.get(1..).unwrap_or_default());
    let first = mantissa.first().copied().unwrap_or(b'0');
    let rest = mantissa.get(2..).unwrap_or_default(); // after `d.`
    let digits = 1 + rest.len();
    let digit = |i: usize| if i == 0 { first } else { rest[i - 1] };

    if negative {
        out.push(b'-');
    }
    if positional {
        // The value is 0.DIGITS x 10^point.
        let point = exponent + 1;
        if point <= 0 {
            out.extend_from_slice(b"0.");
            out.extend(std::iter::repeat_n(b'0', point.unsigned_abs() as usize));
            out.extend((0..digits).map(digit));
        } else {
            let point = point.unsigned_abs() as usize;
            if point >= digits {
                out.extend((0..digits).map(digit));
                out.extend(std::iter::repeat_n(b'0', point - digits));
                out.extend_from_slice(b".0");
            } else {
                out.extend((0..point).map(digit));
                out.push(b'.');
                out.extend((point..digits).map(digit));
            }
        }
    } else {
        out.push(first);
        if !rest.is_empty() {
            out.push(b'.');
            out.extend_from_slice(rest);
        }
        out.push(b'e');
        out.push(if exponent < 0 { b'-' } else { b'+' });
        let e = exponent.unsigned_abs();
        if e < 10 {
            out.push(b'0');
        }
        let _ = write!(out, "{e}");
    }
}

fn parse_exponent(text: &[u8]) -> i32 {
    let (negative, digits) = match text.split_first() {
        Some((b'-', rest)) => (true, rest),
        _ => (false, text),
    };
    let magnitude = digits
        .iter()
        .fold(0i32, |n, &d| n * 10 + i32::from(d.wrapping_sub(b'0')));
    if negative { -magnitude } else { magnitude }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f64s(v: f64) -> String {
        let (mut out, mut scratch) = (Vec::new(), Vec::new());
        write_f64(&mut out, &mut scratch, v);
        String::from_utf8(out).unwrap()
    }

    fn f32s(v: f32) -> String {
        let (mut out, mut scratch) = (Vec::new(), Vec::new());
        write_f32(&mut out, &mut scratch, v);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn as_numpy_writes_them() {
        let cases: &[(f64, &str)] = &[
            (0.1, "0.1"),
            (123.0, "123.0"),
            (-0.0, "-0.0"),
            (0.0, "0.0"),
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (0.000_123_4, "0.0001234"),
            (1e16, "1e+16"),
            (9_999_999_999_999_998.0, "9999999999999998.0"),
            (1.5e15, "1500000000000000.0"),
            (1.234_567_890_123_456_8e17, "1.2345678901234568e+17"),
            (5e-324, "5e-324"),
            (1e-300, "1e-300"),
            (-2.5e-3, "-0.0025"),
        ];
        for &(v, want) in cases {
            assert_eq!(f64s(v), want, "{v:?}");
        }
        let cases: &[(f32, &str)] = &[
            (0.1, "0.1"),
            (1e-4, "1e-04"),
            (0.000_11, "0.00011"),
            (65504.0, "65504.0"),
            (999_999.94, "999999.94"),
            (1e6, "1e+06"),
            (16_777_216.0, "1.6777216e+07"),
            (3.4e38, "3.4e+38"),
            (1e-45, "1e-45"),
            (-0.0, "-0.0"),
        ];
        for &(v, want) in cases {
            assert_eq!(f32s(v), want, "{v:?}");
        }
    }
}
