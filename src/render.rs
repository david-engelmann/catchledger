//! Deterministic JSON rendering, byte for byte the Python rendering.
//!
//! Keys are sorted, indentation is two spaces, non ASCII text stands
//! raw, and the document ends with exactly one trailing newline.

use crate::jsonvalue::JsonValue;

pub(crate) fn escape_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

/// Spell a float the way the Python JSON encoder spells it.
///
/// The shortest digit string that round trips is read off the Rust
/// scientific spelling, then laid out under the Python rule, fixed
/// notation for a leading digit exponent in the range -4 to 15 and
/// scientific notation outside it.
pub fn json_float_text(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    let magnitude = value.abs();
    if magnitude == 0.0 {
        return format!("{sign}0.0");
    }
    let scientific = format!("{magnitude:e}");
    let (mantissa, exponent_text) = scientific
        .split_once('e')
        .expect("scientific spelling always carries an exponent");
    let exponent: i32 = exponent_text
        .parse()
        .expect("scientific exponent always parses");
    let mut digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    // The Python float spelling chooses between the two shortest
    // candidates by closeness to the exact value, and where both
    // candidates read back to the same value on an exact tie, by
    // the even last digit. The Rust spelling rounds a decimal tie
    // up even where the floor candidate also reads back and is the
    // even one. The exact decimal expansion of the value shows a
    // decimal tie as a five standing alone at the first digit past
    // the shortest length, with nothing but zeros after it. A
    // decimal tie is a true tie only when the other candidate also
    // reads back, because the reading interval of a value standing
    // at a binade edge is not symmetric, so the other candidate is
    // parsed and compared before the even rule is applied.
    let exact = format!("{magnitude:.767e}");
    let exact_digits: String = exact
        .split_once('e')
        .expect("scientific spelling always carries an exponent")
        .0
        .chars()
        .filter(|ch| *ch != '.')
        .collect();
    if exact_digits.len() > digits.len() {
        let rest = &exact_digits[digits.len()..];
        if rest.starts_with('5') && rest[1..].chars().all(|ch| ch == '0') {
            let floor = &exact_digits[..digits.len()];
            let other: String = if floor == digits {
                let mut raised: Vec<u8> = digits.bytes().collect();
                let mut at = raised.len() - 1;
                loop {
                    if raised[at] == b'9' {
                        raised[at] = b'0';
                        if at == 0 {
                            raised.insert(0, b'1');
                            break;
                        }
                        at -= 1;
                    } else {
                        raised[at] += 1;
                        break;
                    }
                }
                String::from_utf8(raised).expect("digits stay ASCII")
            } else {
                floor.to_string()
            };
            let unit = exponent - digits.len() as i32 + 1;
            if let Ok(parsed) = format!("{other}e{unit}").parse::<f64>() {
                if parsed.to_bits() == magnitude.to_bits()
                    && other.len() == digits.len()
                    && digits.as_bytes()[digits.len() - 1] % 2 == 1
                    && other.as_bytes()[other.len() - 1] % 2 == 0
                {
                    digits = other;
                }
            }
        }
    }
    if (-4..16).contains(&exponent) {
        // Fixed notation. The point stands after exponent + 1 digits.
        let point = exponent + 1;
        let body = if point <= 0 {
            let mut text = String::from("0.");
            text.push_str(&"0".repeat((-point) as usize));
            text.push_str(&digits);
            text
        } else if (point as usize) >= digits.len() {
            let mut text = digits.clone();
            text.push_str(&"0".repeat(point as usize - digits.len()));
            text.push_str(".0");
            text
        } else {
            let mut text = String::from(&digits[..point as usize]);
            text.push('.');
            text.push_str(&digits[point as usize..]);
            text
        };
        format!("{sign}{body}")
    } else {
        let mut mantissa_text = String::from(&digits[..1]);
        if digits.len() > 1 {
            mantissa_text.push('.');
            mantissa_text.push_str(&digits[1..]);
        }
        let exponent_sign = if exponent < 0 { "-" } else { "+" };
        format!("{sign}{mantissa_text}e{exponent_sign}{:02}", exponent.abs())
    }
}

fn write_value(value: &JsonValue, indent: usize, out: &mut String) {
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        JsonValue::Int(integer) => out.push_str(&integer.to_string()),
        JsonValue::BigInt(text) => out.push_str(text),
        JsonValue::Float(number) => out.push_str(&json_float_text(*number)),
        JsonValue::Text(text) => escape_string(text, out),
        JsonValue::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (index, item) in items.iter().enumerate() {
                out.push_str(&"  ".repeat(indent + 1));
                write_value(item, indent + 1, out);
                if index + 1 < items.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(indent));
            out.push(']');
        }
        JsonValue::Object(pairs) => {
            if pairs.is_empty() {
                out.push_str("{}");
                return;
            }
            let mut sorted: Vec<&(String, JsonValue)> = pairs.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            out.push_str("{\n");
            for (index, (key, item)) in sorted.iter().enumerate() {
                out.push_str(&"  ".repeat(indent + 1));
                escape_string(key, out);
                out.push_str(": ");
                write_value(item, indent + 1, out);
                if index + 1 < sorted.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&"  ".repeat(indent));
            out.push('}');
        }
    }
}

/// Render a document exactly as the Python `render_json` renders it.
pub fn render_json(document: &JsonValue) -> String {
    let mut out = String::new();
    write_value(document, 0, &mut out);
    out.push('\n');
    out
}

fn write_value_compact(value: &JsonValue, out: &mut String) {
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(flag) => out.push_str(if *flag { "true" } else { "false" }),
        JsonValue::Int(integer) => out.push_str(&integer.to_string()),
        JsonValue::BigInt(text) => out.push_str(text),
        JsonValue::Float(number) => out.push_str(&json_float_text(*number)),
        JsonValue::Text(text) => escape_string(text, out),
        JsonValue::List(items) => {
            out.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_value_compact(item, out);
            }
            out.push(']');
        }
        JsonValue::Object(pairs) => {
            let mut sorted: Vec<&(String, JsonValue)> = pairs.iter().collect();
            sorted.sort_by(|a, b| a.0.cmp(&b.0));
            out.push('{');
            for (index, (key, item)) in sorted.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                escape_string(key, out);
                out.push(':');
                write_value_compact(item, out);
            }
            out.push('}');
        }
    }
}

/// Render a document in the canonical rendering the digest helpers
/// hash, sorted keys and compact separators and no trailing newline,
/// byte for byte the rendering the Python canonical digest renders.
pub fn render_canonical(document: &JsonValue) -> String {
    let mut out = String::new();
    write_value_compact(document, &mut out);
    out
}
