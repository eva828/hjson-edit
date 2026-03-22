#[derive(Debug, Clone, PartialEq)]
pub enum HjsonValue {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
    Null,
    Array(Vec<HjsonValue>),
    Object(Vec<(String, HjsonValue)>),
}

impl From<&str> for HjsonValue {
    fn from(s: &str) -> Self {
        HjsonValue::String(s.to_string())
    }
}

impl From<String> for HjsonValue {
    fn from(s: String) -> Self {
        HjsonValue::String(s)
    }
}

impl From<i64> for HjsonValue {
    fn from(n: i64) -> Self {
        HjsonValue::Integer(n)
    }
}

impl From<i32> for HjsonValue {
    fn from(n: i32) -> Self {
        HjsonValue::Integer(n as i64)
    }
}

impl From<f64> for HjsonValue {
    fn from(n: f64) -> Self {
        HjsonValue::Float(n)
    }
}

impl From<f32> for HjsonValue {
    fn from(n: f32) -> Self {
        HjsonValue::Float(n as f64)
    }
}

impl From<bool> for HjsonValue {
    fn from(b: bool) -> Self {
        HjsonValue::Bool(b)
    }
}

impl From<Vec<HjsonValue>> for HjsonValue {
    fn from(v: Vec<HjsonValue>) -> Self {
        HjsonValue::Array(v)
    }
}

impl From<Vec<(String, HjsonValue)>> for HjsonValue {
    fn from(v: Vec<(String, HjsonValue)>) -> Self {
        HjsonValue::Object(v)
    }
}
