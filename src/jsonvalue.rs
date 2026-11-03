//! The JSON document model and its decoder.
//!
//! The decoder reads the grammar the Python `json` decoder reads, with
//! the duplicate key refusal of obligation R2-1, a nesting bound of
//! 4000 levels, and exact storage for integer tokens that do not fit
//! in a signed 64 bit integer.

/// The deepest nesting the decoder accepts.
pub const MAX_NESTING_DEPTH: usize = 4000;

/// A parsed JSON document, preserving the written order of fields.
///
/// Integer tokens that fit in an `i64` are stored as `Int`. Every
/// other integer token is stored as `BigInt` holding its exact digit
/// text, sign included, so it renders back digit for digit.
#[derive(Debug, Clone)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Int(i64),
    BigInt(String),
    Float(f64),
    Text(String),
    List(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            JsonValue::Object(pairs) => pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn as_object(&self) -> Option<&Vec<(String, JsonValue)>> {
        match self {
            JsonValue::Object(pairs) => Some(pairs),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            JsonValue::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn object(pairs: Vec<(&str, JsonValue)>) -> JsonValue {
        JsonValue::Object(
            pairs
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }
}

fn float_integer_text(value: f64) -> Option<String> {
    if !value.is_finite() || value.fract() != 0.0 {
        return None;
    }
    if value == 0.0 {
        return Some("0".to_string());
    }
    Some(format!("{value:.0}"))
}

fn integer_text(value: &JsonValue) -> Option<String> {
    match value {
        JsonValue::Int(integer) => Some(integer.to_string()),
        JsonValue::BigInt(text) => Some(text.clone()),
        JsonValue::Bool(flag) => Some(if *flag { "1" } else { "0" }.to_string()),
        _ => None,
    }
}

fn numbers_equal(a: &JsonValue, b: &JsonValue) -> Option<bool> {
    let a_numeric = matches!(
        a,
        JsonValue::Int(_) | JsonValue::BigInt(_) | JsonValue::Float(_) | JsonValue::Bool(_)
    );
    let b_numeric = matches!(
        b,
        JsonValue::Int(_) | JsonValue::BigInt(_) | JsonValue::Float(_) | JsonValue::Bool(_)
    );
    if !a_numeric || !b_numeric {
        return None;
    }
    if let (JsonValue::Float(x), JsonValue::Float(y)) = (a, b) {
        return Some(x == y || (x.is_nan() && y.is_nan()));
    }
    // Mixed kinds compare exactly, the way Python compares int and
    // float and bool across kinds.
    let a_float = match a {
        JsonValue::Float(value) => Some(*value),
        _ => None,
    };
    let b_float = match b {
        JsonValue::Float(value) => Some(*value),
        _ => None,
    };
    match (a_float, b_float) {
        (Some(float), None) => Some(match integer_text(b) {
            Some(text) => float_integer_text(float).is_some_and(|exact| exact == text),
            None => false,
        }),
        (None, Some(float)) => Some(match integer_text(a) {
            Some(text) => float_integer_text(float).is_some_and(|exact| exact == text),
            None => false,
        }),
        _ => Some(integer_text(a) == integer_text(b)),
    }
}

impl PartialEq for JsonValue {
    fn eq(&self, other: &JsonValue) -> bool {
        if let Some(equal) = numbers_equal(self, other) {
            return equal;
        }
        match (self, other) {
            (JsonValue::Null, JsonValue::Null) => true,
            (JsonValue::Text(a), JsonValue::Text(b)) => a == b,
            (JsonValue::List(a), JsonValue::List(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x == y)
            }
            (JsonValue::Object(a), JsonValue::Object(b)) => {
                a.len() == b.len()
                    && a.iter().all(|(key, value)| match other.get(key) {
                        Some(other_value) => value == other_value,
                        None => false,
                    })
            }
            _ => false,
        }
    }
}

/// Why a document did not decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseFailure {
    /// The document is not valid JSON under the ported grammar. A
    /// document nested beyond the bound and a document carrying a
    /// lone surrogate escape also land here, as the contract states.
    Invalid,
    /// One object carries the named key more than once.
    DuplicateKey(String),
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn skip_whitespace(&mut self) {
        while let Some(byte) = self.peek() {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<JsonValue, ParseFailure> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_list(depth),
            Some(b'"') => Ok(JsonValue::Text(self.parse_string()?)),
            Some(b't') => self.parse_literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal("false", JsonValue::Bool(false)),
            Some(b'n') => self.parse_literal("null", JsonValue::Null),
            Some(b'N') => self.parse_literal("NaN", JsonValue::Float(f64::NAN)),
            Some(b'I') => self.parse_literal("Infinity", JsonValue::Float(f64::INFINITY)),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            _ => Err(ParseFailure::Invalid),
        }
    }

    fn parse_literal(&mut self, word: &str, value: JsonValue) -> Result<JsonValue, ParseFailure> {
        if self.bytes[self.pos..].starts_with(word.as_bytes()) {
            self.pos += word.len();
            Ok(value)
        } else {
            Err(ParseFailure::Invalid)
        }
    }

    fn parse_list(&mut self, depth: usize) -> Result<JsonValue, ParseFailure> {
        if depth >= MAX_NESTING_DEPTH {
            return Err(ParseFailure::Invalid);
        }
        self.pos += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::List(items));
        }
        loop {
            let item = self.parse_value(depth + 1)?;
            items.push(item);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::List(items));
                }
                _ => return Err(ParseFailure::Invalid),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<JsonValue, ParseFailure> {
        if depth >= MAX_NESTING_DEPTH {
            return Err(ParseFailure::Invalid);
        }
        self.pos += 1;
        let mut pairs: Vec<(String, JsonValue)> = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(pairs));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(ParseFailure::Invalid);
            }
            let key = self.parse_string()?;
            self.skip_whitespace();
            if self.peek() != Some(b':') {
                return Err(ParseFailure::Invalid);
            }
            self.pos += 1;
            let value = self.parse_value(depth + 1)?;
            pairs.push((key, value));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => return Err(ParseFailure::Invalid),
            }
        }
        // The duplicate check stands at the close of the object, so an
        // inner object is checked before the object that carries it,
        // and the first key that stands twice in written order is the
        // key named. This matches the pair preserving hook exactly.
        let mut seen: Vec<&str> = Vec::with_capacity(pairs.len());
        for (key, _) in &pairs {
            if seen.contains(&key.as_str()) {
                return Err(ParseFailure::DuplicateKey(key.clone()));
            }
            seen.push(key.as_str());
        }
        Ok(JsonValue::Object(pairs))
    }

    fn parse_string(&mut self) -> Result<String, ParseFailure> {
        // The caller has seen the opening quote.
        self.pos += 1;
        let mut out = String::new();
        loop {
            let byte = self.peek().ok_or(ParseFailure::Invalid)?;
            match byte {
                b'"' => {
                    self.pos += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.pos += 1;
                    let escape = self.peek().ok_or(ParseFailure::Invalid)?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let unit = self.parse_hex4()?;
                            if (0xD800..0xDC00).contains(&unit) {
                                // A high surrogate must pair with a low
                                // surrogate escape. A lone surrogate is
                                // refused, as the contract states.
                                if self.peek() == Some(b'\\')
                                    && self.bytes.get(self.pos + 1) == Some(&b'u')
                                {
                                    self.pos += 2;
                                    let low = self.parse_hex4()?;
                                    if !(0xDC00..0xE000).contains(&low) {
                                        return Err(ParseFailure::Invalid);
                                    }
                                    let code = 0x1_0000
                                        + (((unit as u32) - 0xD800) << 10)
                                        + ((low as u32) - 0xDC00);
                                    out.push(char::from_u32(code).ok_or(ParseFailure::Invalid)?);
                                } else {
                                    return Err(ParseFailure::Invalid);
                                }
                            } else if (0xDC00..0xE000).contains(&unit) {
                                return Err(ParseFailure::Invalid);
                            } else {
                                out.push(char::from_u32(unit as u32).ok_or(ParseFailure::Invalid)?);
                            }
                        }
                        _ => return Err(ParseFailure::Invalid),
                    }
                }
                0x00..=0x1F => return Err(ParseFailure::Invalid),
                _ => {
                    // Copy one whole UTF-8 character. The input is a
                    // string slice, so the bytes are valid UTF-8 and
                    // the cursor stands on a character boundary.
                    let width = if byte < 0x80 {
                        1
                    } else if byte >> 5 == 0b110 {
                        2
                    } else if byte >> 4 == 0b1110 {
                        3
                    } else {
                        4
                    };
                    let text = std::str::from_utf8(&self.bytes[self.pos..self.pos + width])
                        .map_err(|_| ParseFailure::Invalid)?;
                    let ch = text.chars().next().ok_or(ParseFailure::Invalid)?;
                    out.push(ch);
                    self.pos += width;
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u16, ParseFailure> {
        let mut value: u16 = 0;
        for _ in 0..4 {
            let byte = self.peek().ok_or(ParseFailure::Invalid)?;
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a') + 10,
                b'A'..=b'F' => u16::from(byte - b'A') + 10,
                _ => return Err(ParseFailure::Invalid),
            };
            value = value * 16 + digit;
            self.pos += 1;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<JsonValue, ParseFailure> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
            if self.bytes[self.pos..].starts_with(b"Infinity") {
                self.pos += "Infinity".len();
                return Ok(JsonValue::Float(f64::NEG_INFINITY));
            }
            if self.bytes[self.pos..].starts_with(b"NaN") {
                self.pos += "NaN".len();
                return Ok(JsonValue::Float(f64::NAN));
            }
        }
        // Integer part: a single zero or a nonzero digit run.
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(ParseFailure::Invalid),
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(ParseFailure::Invalid);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(ParseFailure::Invalid);
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        let token =
            std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|_| ParseFailure::Invalid)?;
        if is_float {
            token
                .parse::<f64>()
                .map(JsonValue::Float)
                .map_err(|_| ParseFailure::Invalid)
        } else {
            match token.parse::<i64>() {
                Ok(integer) => Ok(JsonValue::Int(integer)),
                Err(_) => Ok(JsonValue::BigInt(token.to_string())),
            }
        }
    }
}

/// Decode a JSON document from text.
pub fn parse_json(text: &str) -> Result<JsonValue, ParseFailure> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        pos: 0,
    };
    let value = parser.parse_value(0)?;
    parser.skip_whitespace();
    if parser.pos != parser.bytes.len() {
        return Err(ParseFailure::Invalid);
    }
    Ok(value)
}
