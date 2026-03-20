use crate::syntax::GreenElement;
use crate::syntax::{HjsonLanguage, SyntaxKind};
use rowan::{Language, NodeOrToken};

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
}

/// 改行コードを検出可能な対象
///
/// テキスト、構文要素、ドキュメントなどから改行コードを検出するためのtrait。
/// 検出できない場合は `None` を返す。
pub trait DetectNewline {
    /// 改行コードを検出する。検出できない場合は `None` を返す。
    fn try_detect_newline(&self) -> Option<NewlineKind>;
}

impl DetectNewline for &str {
    fn try_detect_newline(&self) -> Option<NewlineKind> {
        if self.contains("\r\n") {
            Some(NewlineKind::CrLf)
        } else if self.contains('\n') {
            Some(NewlineKind::Lf)
        } else {
            None
        }
    }
}

impl DetectNewline for &[GreenElement] {
    fn try_detect_newline(&self) -> Option<NewlineKind> {
        let mut saw_lf = false;

        for element in *self {
            let NodeOrToken::Token(token) = element else {
                continue;
            };

            if HjsonLanguage::kind_from_raw(token.kind()) != SyntaxKind::Newline {
                continue;
            }

            match NewlineKind::from_newline_text(token.text()) {
                Some(NewlineKind::CrLf) => return Some(NewlineKind::CrLf),
                Some(NewlineKind::Lf) => saw_lf = true,
                None => {}
            }
        }

        if saw_lf {
            Some(NewlineKind::Lf)
        } else {
            None
        }
    }
}

impl DetectNewline for String {
    fn try_detect_newline(&self) -> Option<NewlineKind> {
        self.as_str().try_detect_newline()
    }
}

impl DetectNewline for Vec<GreenElement> {
    fn try_detect_newline(&self) -> Option<NewlineKind> {
        self.as_slice().try_detect_newline()
    }
}

pub struct DocumentStyle<'a> {
    source: &'a str,
}

impl<'a> DocumentStyle<'a> {
    pub fn new(source: &'a str) -> Self {
        Self { source }
    }

    pub fn newline_kind(&self) -> NewlineKind {
        self.source.try_detect_newline().unwrap_or_default()
    }

    pub fn newline(&self) -> &'static str {
        self.newline_kind().as_str()
    }

    pub fn indent_unit(&self) -> String {
        let mut space_indents = Vec::new();
        let mut tab_count = 0usize;

        for line in self.source.lines() {
            if line.trim().is_empty() {
                continue;
            }

            let indent: String = line
                .chars()
                .take_while(|ch| *ch == ' ' || *ch == '\t')
                .collect();
            if indent.is_empty() {
                continue;
            }
            if indent.contains('\t') {
                tab_count += 1;
            } else {
                space_indents.push(indent.len());
            }
        }

        if tab_count > 0 {
            return "\t".to_string();
        }

        let width = space_indents
            .into_iter()
            .filter(|len| *len > 0)
            .min()
            .unwrap_or(2);
        " ".repeat(width)
    }

    pub fn line_indent_for_offset(&self, offset: usize) -> String {
        let clamped = offset.min(self.source.len());
        let start = self.source[..clamped]
            .rfind('\n')
            .map(|idx| idx + 1)
            .unwrap_or(0);
        self.source[start..clamped]
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect()
    }

    pub fn normalize_text(&self, text: &str) -> String {
        TextNormalizer::normalize_newlines(text, self.newline_kind())
    }
}

pub struct TextNormalizer;

impl TextNormalizer {
    pub fn normalize_newlines(text: &str, style: NewlineKind) -> String {
        let normalized = text.replace("\r\n", "\n");
        match style {
            NewlineKind::Lf => normalized,
            NewlineKind::CrLf => normalized.replace('\n', "\r\n"),
        }
    }
}
