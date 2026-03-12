#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NewlineKind {
    Lf,
    CrLf,
}

impl NewlineKind {
    pub fn from_text(text: &str) -> Self {
        if text.contains("\r\n") {
            Self::CrLf
        } else {
            Self::Lf
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::CrLf => "\r\n",
        }
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
        NewlineKind::from_text(self.source)
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
