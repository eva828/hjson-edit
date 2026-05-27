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
            idx += 1;
            has_fraction_or_exp = true;
            while idx < bytes.len() && bytes[idx].is_ascii_digit() {
                idx += 1;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_valid_integer_returns_some_integer() {
        assert_eq!(HjsonNumber::classify("0"), Some(NumberKind::Integer));
        assert_eq!(HjsonNumber::classify("123"), Some(NumberKind::Integer));
        assert_eq!(HjsonNumber::classify("-456"), Some(NumberKind::Integer));
        assert_eq!(HjsonNumber::classify("-0"), Some(NumberKind::Integer));
    }

    #[test]
    fn classify_valid_float_returns_some_float() {
        assert_eq!(HjsonNumber::classify("1.5"), Some(NumberKind::Float));
        assert_eq!(HjsonNumber::classify("-3.14"), Some(NumberKind::Float));
        assert_eq!(HjsonNumber::classify("1e10"), Some(NumberKind::Float));
        assert_eq!(HjsonNumber::classify("2.5E-3"), Some(NumberKind::Float));
        assert_eq!(HjsonNumber::classify("6.022e+23"), Some(NumberKind::Float));
    }

    #[test]
    fn classify_leading_zero_with_digit_returns_none() {
        // RFC: leading zeros not allowed; try page: treated as string
        assert_eq!(HjsonNumber::classify("01"), None);
        assert_eq!(HjsonNumber::classify("007"), None);
    }

    #[test]
    fn classify_trailing_decimal_returns_float() {
        // "1." has a decimal point, should be treated as Float.
        assert_eq!(HjsonNumber::classify("1."), Some(NumberKind::Float));
        assert_eq!(HjsonNumber::classify("123."), Some(NumberKind::Float));
    }

    #[test]
    fn classify_incomplete_exponent_returns_none() {
        // RFC: exp needs digits after e/E; try page: treated as string
        assert_eq!(HjsonNumber::classify("1e"), None);
        assert_eq!(HjsonNumber::classify("1E"), None);
        assert_eq!(HjsonNumber::classify("1e+"), None);
        assert_eq!(HjsonNumber::classify("1e-"), None);
    }

    #[test]
    fn classify_plus_sign_returns_none() {
        assert_eq!(HjsonNumber::classify("+1"), None);
    }

    #[test]
    fn classify_hex_returns_none() {
        assert_eq!(HjsonNumber::classify("0x1"), None);
        assert_eq!(HjsonNumber::classify("0x1A"), None);
    }

    #[test]
    fn classify_bare_decimal_returns_none() {
        assert_eq!(HjsonNumber::classify(".5"), None);
        assert_eq!(HjsonNumber::classify("."), None);
    }

    #[test]
    fn classify_multiple_decimals_returns_none() {
        assert_eq!(HjsonNumber::classify("1.2.3"), None);
    }

    #[test]
    fn classify_non_numeric_returns_none() {
        assert_eq!(HjsonNumber::classify(""), None);
        assert_eq!(HjsonNumber::classify("-"), None);
        assert_eq!(HjsonNumber::classify("NaN"), None);
        assert_eq!(HjsonNumber::classify("Infinity"), None);
        assert_eq!(HjsonNumber::classify("12abc"), None);
    }

    #[test]
    fn is_valid_returns_true_for_valid_numbers() {
        assert!(HjsonNumber::is_valid("42"));
        assert!(HjsonNumber::is_valid("-1.5"));
        assert!(HjsonNumber::is_valid("1e10"));
        assert!(HjsonNumber::is_valid("1."));
        assert!(HjsonNumber::is_valid("123."));
    }

    #[test]
    fn is_valid_returns_false_for_invalid_numbers() {
        assert!(!HjsonNumber::is_valid("01"));
        assert!(!HjsonNumber::is_valid("1e"));
        assert!(!HjsonNumber::is_valid("abc"));
    }
}
