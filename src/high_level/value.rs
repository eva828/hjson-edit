use rowan::{GreenNode, GreenToken, Language, NodeOrToken};

use crate::{syntax::HjsonLanguage, RedNode, SyntaxKind};

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

impl HjsonValue {
    pub fn from_rednode(v: RedNode) -> Self {
        match v.kind() {
            SyntaxKind::Value => v
                .children()
                .next()
                .map(|child| Self::from_rednode(child))
                .unwrap_or(HjsonValue::Null),
            SyntaxKind::Integer => {
                HjsonValue::from(v.text().to_string().parse::<i64>().unwrap_or_default())
            }
            SyntaxKind::Float => {
                HjsonValue::from(v.text().to_string().parse::<f64>().unwrap_or_default())
            }
            SyntaxKind::True => HjsonValue::Bool(true),
            SyntaxKind::False => HjsonValue::Bool(false),
            SyntaxKind::String => HjsonValue::String(v.text().to_string()),
            SyntaxKind::UnquotedString => HjsonValue::String(v.text().to_string()),
            SyntaxKind::BlockString => HjsonValue::String(v.text().to_string()),
            SyntaxKind::Object => {
                let pairs = v
                    .children()
                    .filter(|child| {
                        HjsonLanguage::kind_from_raw(child.kind().into()) == SyntaxKind::Pair
                    })
                    .map(|child| {
                        let key = child
                            .children()
                            .find(|child| {
                                HjsonLanguage::kind_from_raw(child.kind().into()) == SyntaxKind::Key
                            })
                            .map(|child| child.text().to_string())
                            .unwrap_or_default();
                        let value = child
                            .children()
                            .find(|child| {
                                HjsonLanguage::kind_from_raw(child.kind().into())
                                    == SyntaxKind::Value
                            })
                            .map(|child| Self::from_rednode(child))
                            .unwrap_or(HjsonValue::Null);
                        (key, value)
                    });
                HjsonValue::Object(pairs.collect())
            }
            SyntaxKind::Array => HjsonValue::Array(
                v.children()
                    .filter(|child| {
                        HjsonLanguage::kind_from_raw(child.kind().into()) == SyntaxKind::Value
                    })
                    .map(|child| Self::from_rednode(child))
                    .collect(),
            ),
            _ => {
                unimplemented!("Unsupported syntax kind: {:?}", v.kind())
            }
        }
    }

    fn into_greennode_or_token(self) -> NodeOrToken<GreenNode, GreenToken> {
        match self {
            HjsonValue::String(s) => GreenToken::new(SyntaxKind::String.into(), &s).into(),
            HjsonValue::Integer(n) => {
                GreenToken::new(SyntaxKind::Integer.into(), &n.to_string()).into()
            }
            HjsonValue::Float(f) => {
                GreenToken::new(SyntaxKind::Float.into(), &f.to_string()).into()
            }
            HjsonValue::Bool(b) => {
                let kind = if b {
                    SyntaxKind::True
                } else {
                    SyntaxKind::False
                };
                GreenToken::new(kind.into(), &b.to_string()).into()
            }
            HjsonValue::Null => GreenToken::new(SyntaxKind::Null.into(), "null").into(),
            HjsonValue::Array(arr) => {
                let children: Vec<NodeOrToken<GreenNode, GreenToken>> = arr
                    .into_iter()
                    .map(|v| v.into_greennode_or_token())
                    .collect();
                GreenNode::new(SyntaxKind::Array.into(), children).into()
            }
            HjsonValue::Object(obj) => {
                let children: Vec<NodeOrToken<GreenNode, GreenToken>> = obj
                    .into_iter()
                    .map(|(k, v)| {
                        let key_token = GreenToken::new(SyntaxKind::Key.into(), &k);
                        let value_node: NodeOrToken<GreenNode, GreenToken> =
                            v.into_greennode_or_token();
                        GreenNode::new(SyntaxKind::Pair.into(), vec![key_token.into(), value_node])
                            .into()
                    })
                    .collect();
                GreenNode::new(SyntaxKind::Object.into(), children).into()
            }
        }
    }

    pub fn into_greennode(self) -> GreenNode {
        let value = self.into_greennode_or_token();
        GreenNode::new(SyntaxKind::Value.into(), vec![value])
    }
}
