//! A minimal unsigned big integer for exact decimal arithmetic.
//!
//! Limbs are base 1_000_000_000, little endian, with no trailing zero
//! limbs. The empty vector is zero. Only the operations the ledger
//! arithmetic and the exact integer storage need are implemented.

use std::cmp::Ordering;

const BASE: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigUint {
    limbs: Vec<u32>,
}

impl BigUint {
    pub fn zero() -> Self {
        BigUint { limbs: Vec::new() }
    }

    pub fn from_u64(mut value: u64) -> Self {
        let mut limbs = Vec::new();
        while value > 0 {
            limbs.push((value % BASE) as u32);
            value /= BASE;
        }
        BigUint { limbs }
    }

    /// Build from a run of ASCII decimal digits. Leading zeros are fine.
    pub fn from_decimal_digits(digits: &str) -> Self {
        let mut result = BigUint::zero();
        let bytes = digits.as_bytes();
        let mut index = 0;
        // Consume in chunks of nine digits from the left.
        let first = bytes.len() % 9;
        if first > 0 {
            let mut chunk: u32 = 0;
            for &byte in &bytes[..first] {
                chunk = chunk * 10 + u32::from(byte - b'0');
            }
            result = BigUint::from_u64(u64::from(chunk));
            index = first;
        }
        while index < bytes.len() {
            let mut chunk: u64 = 0;
            for &byte in &bytes[index..index + 9] {
                chunk = chunk * 10 + u64::from(byte - b'0');
            }
            result = result.mul_u64(BASE).add_u64(chunk);
            index += 9;
        }
        result
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    /// Number of decimal digits, with zero counting as one.
    pub fn decimal_digits(&self) -> usize {
        if self.limbs.is_empty() {
            return 1;
        }
        let top = self.limbs[self.limbs.len() - 1];
        let mut digits = top.to_string().len();
        digits += (self.limbs.len() - 1) * 9;
        digits
    }

    pub fn to_decimal_string(&self) -> String {
        if self.limbs.is_empty() {
            return "0".to_string();
        }
        let mut text = self.limbs[self.limbs.len() - 1].to_string();
        for limb in self.limbs.iter().rev().skip(1) {
            text.push_str(&format!("{limb:09}"));
        }
        text
    }

    pub fn cmp_big(&self, other: &BigUint) -> Ordering {
        if self.limbs.len() != other.limbs.len() {
            return self.limbs.len().cmp(&other.limbs.len());
        }
        for index in (0..self.limbs.len()).rev() {
            if self.limbs[index] != other.limbs[index] {
                return self.limbs[index].cmp(&other.limbs[index]);
            }
        }
        Ordering::Equal
    }

    pub fn add_u64(&self, value: u64) -> BigUint {
        self.add_big(&BigUint::from_u64(value))
    }

    pub fn add_big(&self, other: &BigUint) -> BigUint {
        let mut limbs = Vec::with_capacity(self.limbs.len().max(other.limbs.len()) + 1);
        let mut carry: u64 = 0;
        for index in 0..self.limbs.len().max(other.limbs.len()) {
            let a = u64::from(*self.limbs.get(index).unwrap_or(&0));
            let b = u64::from(*other.limbs.get(index).unwrap_or(&0));
            let sum = a + b + carry;
            limbs.push((sum % BASE) as u32);
            carry = sum / BASE;
        }
        if carry > 0 {
            limbs.push(carry as u32);
        }
        BigUint { limbs }.normalized()
    }

    /// Subtract, assuming self is not smaller than other.
    pub fn sub_big(&self, other: &BigUint) -> BigUint {
        debug_assert!(self.cmp_big(other) != Ordering::Less);
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut borrow: i64 = 0;
        for index in 0..self.limbs.len() {
            let a = i64::from(self.limbs[index]);
            let b = i64::from(*other.limbs.get(index).unwrap_or(&0));
            let mut diff = a - b - borrow;
            if diff < 0 {
                diff += BASE as i64;
                borrow = 1;
            } else {
                borrow = 0;
            }
            limbs.push(diff as u32);
        }
        BigUint { limbs }.normalized()
    }

    pub fn mul_u64(&self, value: u64) -> BigUint {
        if value == 0 || self.is_zero() {
            return BigUint::zero();
        }
        if value < BASE {
            let mut limbs = Vec::with_capacity(self.limbs.len() + 1);
            let mut carry: u64 = 0;
            for &limb in &self.limbs {
                let product = u64::from(limb) * value + carry;
                limbs.push((product % BASE) as u32);
                carry = product / BASE;
            }
            while carry > 0 {
                limbs.push((carry % BASE) as u32);
                carry /= BASE;
            }
            return BigUint { limbs }.normalized();
        }
        self.mul_big(&BigUint::from_u64(value))
    }

    pub fn mul_big(&self, other: &BigUint) -> BigUint {
        if self.is_zero() || other.is_zero() {
            return BigUint::zero();
        }
        let mut limbs = vec![0u32; self.limbs.len() + other.limbs.len()];
        for (i, &a) in self.limbs.iter().enumerate() {
            let mut carry: u64 = 0;
            for (j, &b) in other.limbs.iter().enumerate() {
                let current = u64::from(limbs[i + j]) + u64::from(a) * u64::from(b) + carry;
                limbs[i + j] = (current % BASE) as u32;
                carry = current / BASE;
            }
            let mut index = i + other.limbs.len();
            while carry > 0 {
                let current = u64::from(limbs[index]) + carry;
                limbs[index] = (current % BASE) as u32;
                carry = current / BASE;
                index += 1;
            }
        }
        BigUint { limbs }.normalized()
    }

    /// Multiply by ten raised to the given power.
    pub fn mul_pow10(&self, mut power: u32) -> BigUint {
        let mut result = self.clone();
        while power >= 9 {
            result = result.mul_u64(BASE);
            power -= 9;
        }
        if power > 0 {
            result = result.mul_u64(10u64.pow(power));
        }
        result
    }

    /// Divide by another big integer, returning quotient and remainder.
    ///
    /// Implemented as binary long division over a scratch binary form.
    /// The values in this crate stay small (hundreds of digits at the
    /// outside), so the simple algorithm is fast enough.
    pub fn divmod(&self, other: &BigUint) -> (BigUint, BigUint) {
        assert!(!other.is_zero(), "division by zero");
        if self.cmp_big(other) == Ordering::Less {
            return (BigUint::zero(), self.clone());
        }
        let dividend = BinaryUint::from_big(self);
        let divisor = BinaryUint::from_big(other);
        let mut quotient = BinaryUint::zero();
        let mut remainder = BinaryUint::zero();
        let top = dividend.bit_len();
        for index in (0..top).rev() {
            remainder.shl1();
            if dividend.get_bit(index) {
                remainder.set_bit0();
            }
            if remainder.cmp_binary(&divisor) != Ordering::Less {
                remainder = remainder.sub_binary(&divisor);
                quotient.set_bit(index);
            }
        }
        (quotient.to_big(), remainder.to_big())
    }

    fn normalized(mut self) -> BigUint {
        while let Some(&0) = self.limbs.last() {
            self.limbs.pop();
        }
        self
    }
}

/// A scratch binary (base 2^32) form used only inside division.
struct BinaryUint {
    limbs: Vec<u32>,
}

impl BinaryUint {
    fn zero() -> Self {
        BinaryUint { limbs: Vec::new() }
    }

    fn from_big(value: &BigUint) -> Self {
        // Convert base 1e9 limbs to base 2^32 by repeated multiply-add.
        let mut result = BinaryUint::zero();
        for &limb in value.limbs.iter().rev() {
            result.mul_add_small(1_000_000_000, limb);
        }
        result
    }

    fn to_big(&self) -> BigUint {
        // Convert back by repeated division of a copy by 1e9.
        let mut copy = BinaryUint {
            limbs: self.limbs.clone(),
        };
        let mut out = Vec::new();
        while !copy.is_zero() {
            let remainder = copy.divmod_small(1_000_000_000);
            out.push(remainder);
        }
        BigUint { limbs: out }.normalized()
    }

    fn is_zero(&self) -> bool {
        self.limbs.iter().all(|&limb| limb == 0)
    }

    fn mul_add_small(&mut self, multiplier: u32, addend: u32) {
        let mut carry = u64::from(addend);
        for limb in &mut self.limbs {
            let current = u64::from(*limb) * u64::from(multiplier) + carry;
            *limb = current as u32;
            carry = current >> 32;
        }
        while carry > 0 {
            self.limbs.push(carry as u32);
            carry >>= 32;
        }
    }

    fn divmod_small(&mut self, divisor: u32) -> u32 {
        let mut remainder: u64 = 0;
        for limb in self.limbs.iter_mut().rev() {
            let current = (remainder << 32) | u64::from(*limb);
            *limb = (current / u64::from(divisor)) as u32;
            remainder = current % u64::from(divisor);
        }
        while let Some(&0) = self.limbs.last() {
            self.limbs.pop();
        }
        remainder as u32
    }

    fn bit_len(&self) -> usize {
        for (index, &limb) in self.limbs.iter().enumerate().rev() {
            if limb != 0 {
                return index * 32 + (32 - limb.leading_zeros() as usize);
            }
        }
        0
    }

    fn get_bit(&self, bit: usize) -> bool {
        let limb = bit / 32;
        let offset = bit % 32;
        if limb >= self.limbs.len() {
            return false;
        }
        (self.limbs[limb] >> offset) & 1 == 1
    }

    fn set_bit(&mut self, bit: usize) {
        let limb = bit / 32;
        let offset = bit % 32;
        while self.limbs.len() <= limb {
            self.limbs.push(0);
        }
        self.limbs[limb] |= 1 << offset;
    }

    fn set_bit0(&mut self) {
        if self.limbs.is_empty() {
            self.limbs.push(0);
        }
        self.limbs[0] |= 1;
    }

    fn shl1(&mut self) {
        let mut carry = 0u32;
        for limb in &mut self.limbs {
            let next_carry = *limb >> 31;
            *limb = (*limb << 1) | carry;
            carry = next_carry;
        }
        if carry > 0 {
            self.limbs.push(carry);
        }
    }

    fn cmp_binary(&self, other: &BinaryUint) -> Ordering {
        let a_len = self.effective_len();
        let b_len = other.effective_len();
        if a_len != b_len {
            return a_len.cmp(&b_len);
        }
        for index in (0..a_len).rev() {
            if self.limbs[index] != other.limbs[index] {
                return self.limbs[index].cmp(&other.limbs[index]);
            }
        }
        Ordering::Equal
    }

    fn effective_len(&self) -> usize {
        let mut len = self.limbs.len();
        while len > 0 && self.limbs[len - 1] == 0 {
            len -= 1;
        }
        len
    }

    fn sub_binary(&self, other: &BinaryUint) -> BinaryUint {
        let mut limbs = Vec::with_capacity(self.limbs.len());
        let mut borrow: i64 = 0;
        for index in 0..self.limbs.len() {
            let a = i64::from(self.limbs[index]);
            let b = i64::from(*other.limbs.get(index).unwrap_or(&0));
            let mut diff = a - b - borrow;
            if diff < 0 {
                diff += 1 << 32;
                borrow = 1;
            } else {
                borrow = 0;
            }
            limbs.push(diff as u32);
        }
        BinaryUint { limbs }
    }
}

impl PartialOrd for BigUint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(std::cmp::Ord::cmp(self, other))
    }
}

impl Ord for BigUint {
    fn cmp(&self, other: &Self) -> Ordering {
        self.cmp_big(other)
    }
}
