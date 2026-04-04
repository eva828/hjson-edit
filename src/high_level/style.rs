use crate::syntax::GreenElement;
use crate::syntax::{HjsonLanguage, SyntaxKind};
use crate::RedNode;
use rowan::Language;
use rowan::NodeOrToken;

/// 改行コードの種類
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum NewlineKind {
    /// Line Feed (`\n`)
    #[default]
    Lf,
    /// Carriage Return + Line Feed (`\r\n`)
    CrLf,
}

impl NewlineKind {
    /// 改行コードを文字列として取得
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
    }

    /// 改行文字列から改行コード種別を解析
    pub fn from_newline_text(text: &str) -> Option<Self> {
        match text {
            "\n" => Some(Self::Lf),
            "\r\n" => Some(Self::CrLf),
            _ => None,
        }
    }

    pub fn normalize(self, text: &str) -> String {
        let (from, to) = match self {
            Self::Lf => (Self::CrLf, Self::Lf),
            Self::CrLf => (Self::Lf, Self::CrLf),
        };
        text.replace(from.as_str(), to.as_str())
    }
}

/// 改行コードを検出可能な対象
///
/// テキスト、構文要素、ドキュメントなどから改行コードを検出するためのtrait。
/// 検出できない場合は `None` を返す。
pub trait DetectNewline {
    /// 改行コードを検出する。検出できない場合は `None` を返す。
    fn detect_newline(&self) -> NewlineKind;
}

impl DetectNewline for &str {
    fn detect_newline(&self) -> NewlineKind {
        if self.contains("\r\n") {
            NewlineKind::CrLf
        } else if self.contains('\n') {
            NewlineKind::Lf
        } else {
            NewlineKind::default()
        }
    }
}

impl DetectNewline for &[GreenElement] {
    fn detect_newline(&self) -> NewlineKind {
        let newlines = self
            .iter()
            .flat_map(|e| e.as_token())
            .filter(|e| HjsonLanguage::kind_from_raw(e.kind()) != SyntaxKind::Newline);

        newlines
            .clone()
            .flat_map(|token| NewlineKind::from_newline_text(token.text()))
            .find(|kind| *kind != NewlineKind::default())
            .unwrap_or(NewlineKind::default())
    }
}

impl DetectNewline for String {
    fn detect_newline(&self) -> NewlineKind {
        self.as_str().detect_newline()
    }
}

impl DetectNewline for Vec<GreenElement> {
    fn detect_newline(&self) -> NewlineKind {
        self.as_slice().detect_newline()
    }
}

impl DetectNewline for RedNode {
    fn detect_newline(&self) -> NewlineKind {
        detect_newline_in_node(self).unwrap_or_default()
    }
}

fn detect_newline_in_node(node: &RedNode) -> Option<NewlineKind> {
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Token(token) if token.kind() == SyntaxKind::Newline => {
                if let Some(kind) = NewlineKind::from_newline_text(token.text()) {
                    return Some(kind);
                }
            }
            NodeOrToken::Node(child) => {
                if let Some(kind) = detect_newline_in_node(&child) {
                    return Some(kind);
                }
            }
            _ => {}
        }
    }

    None
}
