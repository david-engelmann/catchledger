//! Decimal arithmetic mirroring the Python decimal pipeline.
//!
//! A value is a sign, an exact coefficient, and a power of ten
//! exponent, as a Python decimal carries them. Parsing strips
//! underscores wherever they stand before the grammar reads the
//! text, as the runtime reading does. The context operations round
//! their result to twenty eight significant digits with halves
//! rounding to even, as the runtime default context does, and the
//! quantize step closes a value at six decimal places with half up
//! rounding and reports the overflow the Python rounding step raises
//! on, so the caller can take the side the contract states.

use std::cmp::Ordering;

use crate::bigint::BigUint;

const CONTEXT_DIGITS: usize = 28;

/// A parsed decimal value, finite or not.
#[derive(Debug, Clone)]
pub enum DecimalValue {
    /// An infinite or not a number spelling. The pricing reading
    /// refuses every one of these the same way, so they share a side.
    NonFinite,
    /// A finite value, negative, coefficient times ten to the power.
    Finite {
        negative: bool,
        coeff: BigUint,
        exp10: i64,
    },
}

impl DecimalValue {
    /// The exact decimal for a whole number.
    pub fn from_i64(value: i64) -> DecimalValue {
        DecimalValue::Finite {
            negative: value < 0,
            coeff: BigUint::from_u64(value.unsigned_abs()),
            exp10: 0,
        }
    }

    fn parts(&self) -> (bool, &BigUint, i64) {
        match self {
            DecimalValue::Finite {
                negative,
                coeff,
                exp10,
            } => (*negative, coeff, *exp10),
            DecimalValue::NonFinite => panic!("a non finite value has no parts"),
        }
    }
}

fn digit_run(text: &[char], start: usize) -> (String, usize) {
    let mut digits = String::new();
    let mut index = start;
    while index < text.len() && text[index].is_ascii_digit() {
        digits.push(text[index]);
        index += 1;
    }
    (digits, index)
}

/// Read a decimal text the way the Python decimal reading reads it.
///
/// Underscores are stripped wherever they stand first. What remains
/// is an optional sign and then an infinite or not a number spelling
/// or a digit string with an optional point and fraction and an
/// optional exponent. The not a number spelling may carry a digit
/// payload. The stripped text is not returned, so a stored price
/// text stays the verbatim input elsewhere.
pub fn parse_decimal(text: &str) -> Result<DecimalValue, ()> {
    let stripped: String = text.chars().filter(|ch| *ch != '_').collect();
    let chars: Vec<char> = stripped.chars().collect();
    let mut index = 0;
    let mut negative = false;
    if matches!(chars.first(), Some('+') | Some('-')) {
        negative = chars[0] == '-';
        index = 1;
    }
    let rest: String = chars[index..].iter().collect::<String>().to_lowercase();
    if matches!(rest.as_str(), "inf" | "infinity") {
        return Ok(DecimalValue::NonFinite);
    }
    for word in ["snan", "nan"] {
        if let Some(payload) = rest.strip_prefix(word) {
            if payload.chars().all(|ch| ch.is_ascii_digit()) {
                return Ok(DecimalValue::NonFinite);
            }
        }
    }
    let (int_digits, after_int) = digit_run(&chars, index);
    index = after_int;
    let mut frac_digits = String::new();
    if chars.get(index) == Some(&'.') {
        index += 1;
        let (digits, after) = digit_run(&chars, index);
        frac_digits = digits;
        index = after;
    }
    if int_digits.is_empty() && frac_digits.is_empty() {
        return Err(());
    }
    let mut exponent: i64 = 0;
    if matches!(chars.get(index), Some('e') | Some('E')) {
        index += 1;
        let mut exp_negative = false;
        if matches!(chars.get(index), Some('+') | Some('-')) {
            exp_negative = chars[index] == '-';
            index += 1;
        }
        let (exp_digits, after) = digit_run(&chars, index);
        if exp_digits.is_empty() {
            return Err(());
        }
        exponent = exp_digits.parse::<i64>().unwrap_or(i64::MAX / 2);
        if exp_negative {
            exponent = -exponent;
        }
        index = after;
    }
    if index != chars.len() {
        return Err(());
    }
    let mut coeff_text = int_digits;
    coeff_text.push_str(&frac_digits);
    let exp10 = exponent - frac_digits.len() as i64;
    Ok(DecimalValue::Finite {
        negative,
        coeff: BigUint::from_decimal_digits(&coeff_text),
        exp10,
    })
}

/// Round a digit text to the context digit count, halves to even.
///
/// The dropped digits stand in full, so the tie reading is exact.
/// The sticky standing records a remainder beyond the digit text,
/// and an exact tie with the standing standing rounds up where
/// the bare digit test rounds to even. The rounded coefficient
/// stands returned with the count of dropped digits.
fn round_digit_text(text: &str, sticky: bool) -> (BigUint, i64) {
    let drop = text.len() - CONTEXT_DIGITS;
    let kept = &text[..CONTEXT_DIGITS];
    let dropped = &text[CONTEXT_DIGITS..];
    let half = format!("5{}", "0".repeat(drop - 1));
    let round_up = match dropped.cmp(&half) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => sticky || kept.as_bytes()[CONTEXT_DIGITS - 1] % 2 == 1,
    };
    let mut kept_value = BigUint::from_decimal_digits(kept);
    if round_up {
        kept_value = kept_value.add_u64(1);
    }
    (kept_value, drop as i64)
}

/// Round a coefficient to the context digit count, halves to even.
///
/// The dropped digits stand in full, so the tie reading is exact.
/// The exponent grows by the count of dropped digits.
fn context_round(coeff: BigUint, exp10: i64) -> (BigUint, i64) {
    if coeff.is_zero() {
        return (coeff, exp10);
    }
    let text = coeff.to_decimal_string();
    if text.len() <= CONTEXT_DIGITS {
        return (coeff, exp10);
    }
    let (kept_value, drop) = round_digit_text(&text, false);
    (kept_value, exp10 + drop)
}

/// Multiply two finite decimals under the context.
pub fn decimal_mul(a: &DecimalValue, b: &DecimalValue) -> DecimalValue {
    let (neg_a, coeff_a, exp_a) = a.parts();
    let (neg_b, coeff_b, exp_b) = b.parts();
    let negative = neg_a != neg_b;
    if coeff_a.is_zero() || coeff_b.is_zero() {
        return DecimalValue::Finite {
            negative,
            coeff: BigUint::zero(),
            exp10: exp_a + exp_b,
        };
    }
    let (coeff, exp10) = context_round(coeff_a.mul_big(coeff_b), exp_a + exp_b);
    DecimalValue::Finite {
        negative,
        coeff,
        exp10,
    }
}

/// Add two finite decimals under the context.
///
/// The coefficients are aligned at the smaller exponent before
/// they meet. An adjusted gap past sixty four digits leaves
/// the smaller term standing wholly inside the dropped text of
/// the sum, where it moves the twenty eight digit rounding
/// only at the tie standings and at the carry and borrow
/// standings the gap decision reads, the larger term its own
/// dropped text standing at the boundary where its coefficient
/// carries twenty nine digits or more. At every other standing
/// past the boundary the larger term stands as the sum exactly
/// as it stands, and every smaller gap is aligned exactly.
pub fn decimal_add(a: &DecimalValue, b: &DecimalValue) -> DecimalValue {
    let (neg_a, coeff_a, exp_a) = a.parts();
    let (neg_b, coeff_b, exp_b) = b.parts();
    if coeff_a.is_zero() {
        let (coeff, exp10) = context_round(coeff_b.clone(), exp_b);
        return DecimalValue::Finite {
            negative: neg_b,
            coeff,
            exp10,
        };
    }
    if coeff_b.is_zero() {
        let (coeff, exp10) = context_round(coeff_a.clone(), exp_a);
        return DecimalValue::Finite {
            negative: neg_a,
            coeff,
            exp10,
        };
    }
    let adjusted_a = coeff_a.decimal_digits() as i64 + exp_a;
    let adjusted_b = coeff_b.decimal_digits() as i64 + exp_b;
    if (adjusted_a - adjusted_b).abs() > 64 {
        return if adjusted_a > adjusted_b {
            gap_sum(neg_a, coeff_a, exp_a, neg_b, coeff_b, exp_b)
        } else {
            gap_sum(neg_b, coeff_b, exp_b, neg_a, coeff_a, exp_a)
        };
    }
    let common = exp_a.min(exp_b);
    let scaled_a = coeff_a.mul_pow10((exp_a - common) as u32);
    let scaled_b = coeff_b.mul_pow10((exp_b - common) as u32);
    let (negative, coeff) = if neg_a == neg_b {
        (neg_a, scaled_a.add_big(&scaled_b))
    } else {
        match scaled_a.cmp_big(&scaled_b) {
            Ordering::Greater => (neg_a, scaled_a.sub_big(&scaled_b)),
            Ordering::Less => (neg_b, scaled_b.sub_big(&scaled_a)),
            Ordering::Equal => {
                return DecimalValue::Finite {
                    negative: false,
                    coeff: BigUint::zero(),
                    exp10: common,
                }
            }
        }
    };
    let (coeff, exp10) = context_round(coeff, common);
    DecimalValue::Finite {
        negative,
        coeff,
        exp10,
    }
}

/// The sum of two nonzero finite decimals whose adjusted
/// exponents part by more than sixty four, decided from the
/// operands their own digit texts.
///
/// The smaller term stands wholly inside the dropped text of
/// the aligned sum, past the thirty seventh dropped digit at
/// the nearest, so the rounding of the sum stands decided by
/// the larger term its dropped text read against the tie,
/// moved by the smaller term only where the two standings
/// meet inside that text, at the tie standings and at the
/// carry and borrow standings. Where the smaller term cannot
/// move the rounding, the larger term stands returned as the
/// context rounding returns it, in value and in
/// representation, and no alignment as long as the gap stands
/// built at any standing.
fn gap_sum(
    big_negative: bool,
    big_coeff: &BigUint,
    big_exp: i64,
    small_negative: bool,
    small_coeff: &BigUint,
    small_exp: i64,
) -> DecimalValue {
    let big_text = big_coeff.to_decimal_string();
    if big_text.len() <= CONTEXT_DIGITS {
        let (coeff, exp10) = context_round(big_coeff.clone(), big_exp);
        return DecimalValue::Finite {
            negative: big_negative,
            coeff,
            exp10,
        };
    }
    let kept_text = &big_text[..CONTEXT_DIGITS];
    let dropped_text = &big_text[CONTEXT_DIGITS..];
    let kept_value = BigUint::from_decimal_digits(kept_text);
    let kept_odd = kept_text.as_bytes()[CONTEXT_DIGITS - 1] % 2 == 1;
    let result_exp = big_exp + dropped_text.len() as i64;
    let common = big_exp.min(small_exp);
    let small_height = small_coeff.decimal_digits() as i64 + small_exp - common;
    let big_shift = big_exp - common;
    let same_signs = big_negative == small_negative;
    let round_up = if big_shift >= small_height {
        let tie = format!("5{}", "0".repeat(dropped_text.len() - 1));
        match dropped_text.cmp(&tie) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => same_signs,
        }
    } else {
        let split = dropped_text.len() - (small_height - big_shift) as usize;
        let head_value = BigUint::from_decimal_digits(&dropped_text[..split]);
        let low_value =
            BigUint::from_decimal_digits(&dropped_text[split..]).mul_pow10(big_shift as u32);
        let small_value = small_coeff.mul_pow10((small_exp - common) as u32);
        let half_head = BigUint::from_decimal_digits(&format!("5{}", "0".repeat(split - 1)));
        let field = BigUint::from_u64(1).mul_pow10(small_height as u32);
        let (head_value, tail, borrow) = if same_signs {
            let tail = low_value.add_big(&small_value);
            if tail.cmp_big(&field) != Ordering::Less {
                (head_value.add_u64(1), tail.sub_big(&field), false)
            } else {
                (head_value, tail, false)
            }
        } else if small_value.cmp_big(&low_value) != Ordering::Greater {
            (head_value, low_value.sub_big(&small_value), false)
        } else if head_value.is_zero() {
            let (coeff, exp10) = context_round(big_coeff.clone(), big_exp);
            return DecimalValue::Finite {
                negative: big_negative,
                coeff,
                exp10,
            };
        } else {
            (
                head_value.sub_big(&BigUint::from_u64(1)),
                field.add_big(&low_value).sub_big(&small_value),
                true,
            )
        };
        match head_value.cmp_big(&half_head) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => borrow || !tail.is_zero() || kept_odd,
        }
    };
    let coeff = if round_up {
        kept_value.add_u64(1)
    } else {
        kept_value
    };
    DecimalValue::Finite {
        negative: big_negative,
        coeff,
        exp10: result_exp,
    }
}

/// Divide two finite decimals under the context.
///
/// The divisor is never zero at the call sites, which divide by
/// sixty or by a positive catch count or a positive denominator the
/// caller already checked. The quotient digits are generated past
/// the context digit count so the rounding reads the true tail.
pub fn decimal_div(a: &DecimalValue, b: &DecimalValue) -> DecimalValue {
    let (neg_a, coeff_a, exp_a) = a.parts();
    let (neg_b, coeff_b, exp_b) = b.parts();
    let negative = neg_a != neg_b;
    if coeff_a.is_zero() {
        return DecimalValue::Finite {
            negative,
            coeff: BigUint::zero(),
            exp10: exp_a - exp_b,
        };
    }
    // Scale so the integer quotient carries one or two digits past
    // the context count, then round its digit string with the
    // remainder standing as the sticky tail.
    let shift = (CONTEXT_DIGITS + 1) as i64
        - (coeff_a.decimal_digits() as i64 - coeff_b.decimal_digits() as i64);
    let (quotient, remainder) = if shift >= 0 {
        coeff_a.mul_pow10(shift as u32).divmod(coeff_b)
    } else {
        coeff_a.divmod(&coeff_b.mul_pow10((-shift) as u32))
    };
    let sticky = !remainder.is_zero();
    let base_exp = exp_a - exp_b - shift;
    let text = quotient.to_decimal_string();
    if text.len() <= CONTEXT_DIGITS {
        return DecimalValue::Finite {
            negative,
            coeff: quotient,
            exp10: base_exp,
        };
    }
    let (kept_value, drop) = round_digit_text(&text, sticky);
    DecimalValue::Finite {
        negative,
        coeff: kept_value,
        exp10: base_exp + drop,
    }
}

/// The outcome of the six place quantize step.
pub enum Quantized {
    /// The value rounded at six decimal places, as a float.
    Value(f64),
    /// The rounded coefficient outgrows the context digit count,
    /// the step the Python rounding raises an uncaught error on.
    Overflow,
}

fn quantize(coeff: &BigUint, exp10: i64, negative: bool, capped: bool) -> Quantized {
    if coeff.is_zero() {
        return Quantized::Value(if negative { -0.0 } else { 0.0 });
    }
    if exp10 > 400 {
        // The value is far past the context digit count, so the
        // capped step overflows, and the full value overflows the
        // float conversion the Python pair ends in.
        if capped {
            return Quantized::Overflow;
        }
        return Quantized::Value(if negative {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        });
    }
    let rounded: BigUint = if exp10 >= -6 {
        coeff.mul_pow10((exp10 + 6) as u32)
    } else {
        let drop = (-6 - exp10) as usize;
        let text = coeff.to_decimal_string();
        if drop >= text.len() {
            let padded = format!("{}{}", "0".repeat(drop - text.len()), text);
            let half = format!("5{}", "0".repeat(drop - 1));
            if padded.as_str() >= half.as_str() {
                BigUint::from_u64(1)
            } else {
                BigUint::zero()
            }
        } else {
            let kept = &text[..text.len() - drop];
            let dropped = &text[text.len() - drop..];
            let half = format!("5{}", "0".repeat(drop - 1));
            let mut kept_value = BigUint::from_decimal_digits(kept);
            if dropped >= half.as_str() {
                kept_value = kept_value.add_u64(1);
            }
            kept_value
        }
    };
    if capped && rounded.decimal_digits() > CONTEXT_DIGITS {
        return Quantized::Overflow;
    }
    let sign = if negative { "-" } else { "" };
    let value = format!("{}{}e-6", sign, rounded.to_decimal_string())
        .parse::<f64>()
        .unwrap_or(f64::INFINITY);
    Quantized::Value(value)
}

/// Close a finite decimal at six decimal places, half up, under the
/// context digit cap the Python rounding step enforces.
pub fn quantize_six(value: &DecimalValue) -> Quantized {
    let (negative, coeff, exp10) = value.parts();
    quantize(coeff, exp10, negative, true)
}

/// Close a finite decimal at six decimal places, half up, in full.
///
/// This is the value the port builds where the capped step raises
/// on the Python side, the side the contract states.
pub fn quantize_six_full(value: &DecimalValue) -> f64 {
    let (negative, coeff, exp10) = value.parts();
    match quantize(coeff, exp10, negative, false) {
        Quantized::Value(value) => value,
        Quantized::Overflow => unreachable!("the uncapped quantize cannot overflow"),
    }
}

/// Close a finite decimal at six decimal places, half up,
/// taking the capped value where the capped step stands
/// Value and the full build where it stands Overflow, the
/// closing the money seats of the ledger and the token
/// join read, spelled once here beside the quantize pair
/// it joins.
pub fn close_six(value: &DecimalValue) -> f64 {
    match quantize_six(value) {
        Quantized::Value(closed) => closed,
        Quantized::Overflow => quantize_six_full(value),
    }
}
