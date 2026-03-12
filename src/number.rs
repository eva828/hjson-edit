#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberKind {
    Integer,
    Float,
}

pub struct HjsonNumber;

impl HjsonNumber {
    pub fn classify(text: &str) -> Option<NumberKind> {
        if text.is_empty() {
            return None;
        }

        let bytes = text.as_bytes();
        let mut idx = 0usize;
        let mut has_fraction_or_exp = false;

        if bytes[idx] == b'-' {
            idx += 1;
            if idx == bytes.len() {
                return None;
            }
        }

        if bytes[idx] == b'0' {
            idx += 1;
            if idx < bytes.len() && bytes[idx].is_ascii_digit() {
                return None;
            }
        } else if bytes[idx].is_ascii_digit() {
            idx += 1;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
        } else {
            return None;
        }

        if idx < bytes.len() && bytes[idx] == b'.' {
            has_fraction_or_exp = true;
            idx += 1;
            let frac_start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if frac_start == idx {
                return None;
            }
        }

        if idx < bytes.len() && matches!(bytes[idx], b'e' | b'E') {
            has_fraction_or_exp = true;
            idx += 1;
            if idx < bytes.len() && matches!(bytes[idx], b'+' | b'-') {
                idx += 1;
            }
            let exp_start = idx;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
            }
            if exp_start == idx {
                return None;
            }
        }

        if idx != bytes.len() {
            return None;
        }

        Some(if has_fraction_or_exp {
            NumberKind::Float
        } else {
            NumberKind::Integer
        })
    }

    pub fn is_valid(text: &str) -> bool {
        Self::classify(text).is_some()
    }
}
