use crate::syntax::{NewlineKind, SyntaxKind};
use crate::RedNode;
use rowan::NodeOrToken;

/// 改行コードを検出可能な対象
///
/// テキスト、構文要素、ドキュメントなどから改行コードを検出するためのtrait。
/// 検出できない場合はデフォルト値を返す。
pub trait DetectNewline {
    /// 改行コードを検出する。検出できない場合はデフォルト値を返す。
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

impl DetectNewline for String {
    fn detect_newline(&self) -> NewlineKind {
        self.as_str().detect_newline()
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
                if let Some(kind) = NewlineKind::from_text(token.text()) {
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
