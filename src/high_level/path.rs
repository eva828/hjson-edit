use std::fmt::Debug;

use crate::high_level::error::HjsonEditError;

/// 配列を指すセグメント。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArrayPathSegment {
    /// 位置 `n` の要素。
    Index(usize), // [n]
    /// 位置 `n` へ挿入。
    InsertAt(usize), // [+n]
    /// 末尾の要素（書き込みは末尾への追加）。
    Push, // [-]
}

impl ArrayPathSegment {
    /// 位置を指定するセグメントの index。末尾を指す `[-]` は `None`。
    pub fn position(&self) -> Option<usize> {
        match self {
            ArrayPathSegment::Index(index) | ArrayPathSegment::InsertAt(index) => Some(*index),
            ArrayPathSegment::Push => None,
        }
    }
}

/// パスの 1 セグメント。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathSegment {
    /// オブジェクトのキー。
    Key(String),
    /// 配列の要素。
    Array(ArrayPathSegment),
}

/// 値の位置を指すパス（ドットで区切ったキーと `[n]` の並び）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HjsonObjectPath {
    segments: Vec<PathSegment>,
}

/// コメントを付ける位置。
///
/// `<path>` に `.` を書くと文書直下のコンテナを指す（`.:footer`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HjsonCommentPath {
    /// 文書全体の先頭（`:header`）。
    Header,
    /// 文書全体の末尾（`:footer`）。
    Footer,
    /// 要素の前の行（`<path>:before`。接尾辞を省略した場合も同じ）。
    Before(HjsonObjectPath),
    /// 要素の値の後ろ、同じ行の末尾（`<path>:inline`）。
    Inline(HjsonObjectPath),
    /// コンテナの閉じ区切りの直前（`<path>:footer`）。要素の後の行に置く。
    ContainerFooter(HjsonObjectPath),
}

impl TryFrom<&str> for HjsonCommentPath {
    type Error = HjsonEditError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        if raw == ":header" {
            Ok(Self::Header)
        } else if raw == ":footer" {
            Ok(Self::Footer)
        } else if let Some(stripped) = raw.strip_suffix(":inline") {
            Ok(Self::Inline(parse_element_path(stripped)?))
        } else if let Some(stripped) = raw.strip_suffix(":before") {
            Ok(Self::Before(parse_element_path(stripped)?))
        } else if let Some(stripped) = raw.strip_suffix(":footer") {
            Ok(Self::ContainerFooter(parse_element_path(stripped)?))
        } else if raw.contains(':') {
            Err(HjsonEditError::InvalidPath(format!(
                "{raw}: unknown comment suffix in a comment path"
            )))
        } else {
            Ok(Self::Before(parse_element_path(raw)?))
        }
    }
}

/// コメントの対象になるパス。`.` は文書直下を指す。
fn parse_element_path(raw: &str) -> Result<HjsonObjectPath, HjsonEditError> {
    if raw == "." {
        return Ok(HjsonObjectPath::from_segments(Vec::new()));
    }

    HjsonObjectPath::parse(raw.to_string())
}

/// パスを書く。文書直下（空のパス）は `.` と書く。
fn write_element_path(f: &mut std::fmt::Formatter<'_>, path: &HjsonObjectPath) -> std::fmt::Result {
    if path.depth() == 0 {
        write!(f, ".")
    } else {
        write!(f, "{path}")
    }
}

impl std::fmt::Display for ArrayPathSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArrayPathSegment::Index(index) => write!(f, "[{index}]"),
            ArrayPathSegment::InsertAt(index) => write!(f, "[+{index}]"),
            ArrayPathSegment::Push => write!(f, "[-]"),
        }
    }
}

impl std::fmt::Display for PathSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathSegment::Key(key) => write!(f, "{key}"),
            PathSegment::Array(segment) => write!(f, "{segment}"),
        }
    }
}

impl std::fmt::Display for HjsonObjectPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.segments.is_empty() {
            return write!(f, "(document)");
        }

        let mut first = true;
        for segment in &self.segments {
            match segment {
                PathSegment::Key(key) => {
                    if !first {
                        write!(f, ".")?;
                    }
                    write!(f, "{key}")?;
                }
                PathSegment::Array(array) => write!(f, "{array}")?,
            }
            first = false;
        }
        Ok(())
    }
}

impl std::fmt::Display for HjsonCommentPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HjsonCommentPath::Header => write!(f, ":header"),
            HjsonCommentPath::Footer => write!(f, ":footer"),
            HjsonCommentPath::Before(path) => {
                write_element_path(f, path)?;
                write!(f, ":before")
            }
            HjsonCommentPath::Inline(path) => {
                write_element_path(f, path)?;
                write!(f, ":inline")
            }
            HjsonCommentPath::ContainerFooter(path) => {
                write_element_path(f, path)?;
                write!(f, ":footer")
            }
        }
    }
}

impl HjsonObjectPath {
    /// セグメントの並びからパスを作る。
    pub fn from_segments(segments: Vec<PathSegment>) -> Self {
        Self { segments }
    }

    /// セグメントの並び。
    pub fn segments(&self) -> &[PathSegment] {
        &self.segments
    }

    /// `index` 番目のセグメント。
    pub fn segment(&self, index: usize) -> Option<&PathSegment> {
        self.segments.get(index)
    }

    /// パスの文字列を解釈する。
    pub fn parse(path: impl Into<String>) -> Result<Self, HjsonEditError> {
        let path = path.into();

        if path.starts_with('.') {
            return Err(HjsonEditError::InvalidPath(format!(
                "{path}: a path must not start with '.'"
            )));
        }
        if path.ends_with('.') {
            return Err(HjsonEditError::InvalidPath(format!(
                "{path}: a path must not end with '.'"
            )));
        }
        if path.contains("..") {
            return Err(HjsonEditError::InvalidPath(format!(
                "{path}: a path must not contain consecutive dots"
            )));
        }

        let mut segments = Vec::new();
        let mut chars = path.chars().peekable();

        while chars.peek().is_some() {
            // Parse key part
            let mut key = String::new();
            let mut in_brackets = false;

            while let Some(&c) = chars.peek() {
                if c == '.' {
                    chars.next();
                    break;
                } else if c == '[' {
                    in_brackets = true;
                    break;
                } else {
                    key.push(c);
                    chars.next();
                }
            }

            if !key.is_empty() {
                segments.push(PathSegment::Key(key));
            }

            // Parse array index if present
            if in_brackets {
                chars.next(); // consume '['

                // Check for special operators
                let mut op_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c == ']' {
                        break;
                    } else {
                        op_str.push(c);
                        chars.next();
                    }
                }

                // Expect closing bracket
                if chars.peek() != Some(&']') {
                    return Err(HjsonEditError::InvalidPath(format!(
                        "{path}: expected ']' in an array segment"
                    )));
                }
                chars.next(); // consume ']'

                // Parse the operation
                if op_str == "-" || op_str == "+" {
                    // `[-]` と `[+]` はどちらも末尾を指す
                    segments.push(PathSegment::Array(ArrayPathSegment::Push));
                } else if let Some(stripped) = op_str.strip_prefix('+') {
                    let idx: usize = stripped.parse().map_err(|_| {
                        HjsonEditError::InvalidPath(format!(
                            "{path}: an array index must be a number"
                        ))
                    })?;
                    segments.push(PathSegment::Array(ArrayPathSegment::InsertAt(idx)));
                } else {
                    let idx: usize = op_str.parse().map_err(|_| {
                        HjsonEditError::InvalidPath(format!(
                            "{path}: an array index must be a number"
                        ))
                    })?;
                    segments.push(PathSegment::Array(ArrayPathSegment::Index(idx)));
                }
            }
        }

        Ok(HjsonObjectPath { segments })
    }

    /// 先頭のセグメントと残りのパスに分ける。
    pub fn split_first(&self) -> Option<(PathSegment, Self)> {
        let (left, right) = self.split_at(1)?;
        let first = left.segments.into_iter().next()?;
        Some((first, right))
    }

    /// 末尾のセグメントと残りのパスに分ける。
    pub fn split_last(&self) -> Option<(Self, PathSegment)> {
        if self.segments.is_empty() {
            return None;
        }
        let (left, mut right) = self.split_at(self.segments.len() - 1)?;
        let last = right.segments.pop()?;
        Some((left, last))
    }

    /// `index` で 2 つに分ける。
    pub fn split_at(&self, index: usize) -> Option<(Self, Self)> {
        if index > self.segments.len() {
            return None;
        }
        let (left, right) = self.segments.split_at(index);
        Some((
            Self::from_segments(left.to_vec()),
            Self::from_segments(right.to_vec()),
        ))
    }

    /// セグメント数。
    pub fn depth(&self) -> usize {
        self.segments.len()
    }
}
