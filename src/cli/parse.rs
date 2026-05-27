use std::fmt::Write as FmtWrite;
use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};

use hjson_edit::{
    ast::{
        ArrayItem, CommentKind, DelimiterStyle, Document, DocumentItem, QuoteStyle, ScalarValue,
        Trivia, ValueContent,
    },
    high_level::HjsonDocument,
    RedNode, RedToken, SyntaxKind,
};

pub fn run_parse(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let show_ast = *matches.get_one::<bool>("ast").unwrap_or(&false);
    let show_trivia = *matches.get_one::<bool>("show-trivia").unwrap_or(&false);
    let color_mode = matches
        .get_one::<String>("color")
        .map(|s| s.as_str())
        .unwrap_or("auto");
    let simple = *matches.get_one::<bool>("simple").unwrap_or(&false);
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    match HjsonDocument::parse(&content) {
        Ok(document) => {
            if show_ast {
                let ast = document.ast()?;
                let console_config = ConsoleConfig::new(simple, color_mode);
                let mut tree_view = AstTreeViewBuilder::new(console_config, show_trivia);
                tree_view.dump_document(&ast, true)?;
                print!("{}", tree_view.buf);
            } else {
                let red = document.syntax_root();

                let console_config = ConsoleConfig::new(simple, color_mode);
                let mut tree_view = TreeViewBuilder::new(console_config, show_trivia);
                tree_view.dump_node(red, true)?;
                print!("{}", tree_view.buf);
            }
            Ok(())
        }
        Err(error) => anyhow::bail!("Parse errors:\n  {}", error),
    }
}

struct ConsoleConfig {
    colored: bool,
    pretty: bool,
}

impl ConsoleConfig {
    fn new(simple: bool, color_mode: &str) -> Self {
        let no_color = std::env::var("NO_COLOR").is_ok();
        let is_tty = std::io::stdout().is_terminal();

        let colored = if simple {
            false
        } else if no_color {
            false
        } else {
            match color_mode {
                "always" => true,
                "never" => false,
                "auto" => is_tty,
                _ => is_tty,
            }
        };
        let pretty = !simple;
        Self { colored, pretty }
    }
}

enum Indent {
    Last,
    NotLast,
}

impl Indent {
    fn as_str(&self, pretty: bool) -> &'static str {
        if pretty {
            match self {
                Indent::Last => "    ",
                Indent::NotLast => "│   ",
            }
        } else {
            ""
        }
    }
}

struct TreeViewBuilder {
    config: ConsoleConfig,
    show_trivia: bool,
    buf: String,
    indent_stack: Vec<Indent>,
}

impl TreeViewBuilder {
    fn new(config: ConsoleConfig, show_trivia: bool) -> Self {
        Self {
            config,
            show_trivia,
            buf: String::new(),
            indent_stack: Vec::new(),
        }
    }

    fn connector(&self, is_root: bool, is_last: bool) -> &'static str {
        if self.config.pretty && !is_root {
            if is_last {
                "└── "
            } else {
                "├── "
            }
        } else {
            ""
        }
    }

    fn colorize(&self, text: &str, kind: SyntaxKind) -> String {
        if self.config.colored {
            kind.colorize(text)
        } else {
            text.to_string()
        }
    }

    fn push_indent(&mut self, is_last: bool) {
        self.indent_stack.push(if is_last {
            Indent::Last
        } else {
            Indent::NotLast
        });
    }

    fn pop_indent(&mut self) {
        self.indent_stack.pop();
    }

    fn indent(&self) -> String {
        self.indent_stack
            .iter()
            .map(|indent| indent.as_str(self.config.pretty))
            .collect()
    }

    fn dump_node(&mut self, node: &RedNode, is_last: bool) -> Result<()> {
        let kind = self.colorize(node.kind().to_string().as_str(), node.kind());
        let is_root = node.kind() == SyntaxKind::Root;
        writeln!(
            self.buf,
            "{}{}{}{:?}",
            self.indent(),
            self.connector(is_root, is_last),
            &kind,
            node.text_range()
        )?;

        if !is_root {
            self.push_indent(is_last);
        }

        let mut children = FilterPeekable::new(node.children_with_tokens(), self.show_trivia);
        while let Some(item) = children.next() {
            let is_last = children.peek().is_none();
            match item {
                rowan::NodeOrToken::Node(n) => {
                    self.dump_node(&n, is_last)?;
                }
                rowan::NodeOrToken::Token(t) => {
                    self.dump_token(&t, is_last)?;
                }
            }
        }

        if !is_root {
            self.pop_indent();
        }

        Ok(())
    }

    fn dump_token(&mut self, token: &RedToken, is_last: bool) -> Result<()> {
        let kind = token.kind();

        let token_str = format!("{:?}: {:?}", kind, token.text());

        writeln!(
            self.buf,
            "{}{}{}{:?}",
            self.indent(),
            self.connector(false, is_last),
            self.colorize(&token_str, kind),
            token.text_range()
        )?;

        Ok(())
    }
}

struct AstTreeViewBuilder {
    config: ConsoleConfig,
    show_trivia: bool,
    buf: String,
    indent_stack: Vec<Indent>,
}

impl AstTreeViewBuilder {
    fn new(config: ConsoleConfig, show_trivia: bool) -> Self {
        Self {
            config,
            show_trivia,
            buf: String::new(),
            indent_stack: Vec::new(),
        }
    }

    fn connector(&self, is_root: bool, is_last: bool) -> &'static str {
        if self.config.pretty && !is_root {
            if is_last {
                "└── "
            } else {
                "├── "
            }
        } else {
            ""
        }
    }

    fn colorize(&self, text: &str, color: i32) -> String {
        if self.config.colored {
            format!("\x1b[{}m{}\x1b[0m", color, text)
        } else {
            text.to_string()
        }
    }

    fn push_indent(&mut self, is_last: bool) {
        self.indent_stack.push(if is_last {
            Indent::Last
        } else {
            Indent::NotLast
        });
    }

    fn pop_indent(&mut self) {
        self.indent_stack.pop();
    }

    fn indent(&self) -> String {
        self.indent_stack
            .iter()
            .map(|indent| indent.as_str(self.config.pretty))
            .collect()
    }

    fn begin_node(
        &mut self,
        label: impl Into<String>,
        color: i32,
        is_root: bool,
        is_last: bool,
    ) -> Result<()> {
        let label = self.colorize(&label.into(), color);
        writeln!(
            self.buf,
            "{}{}{}",
            self.indent(),
            self.connector(is_root, is_last),
            label
        )?;

        if !is_root {
            self.push_indent(is_last);
        }

        Ok(())
    }

    fn end_node(&mut self, is_root: bool) {
        if !is_root {
            self.pop_indent();
        }
    }

    fn leaf(&mut self, label: impl Into<String>, color: i32, is_last: bool) -> Result<()> {
        let label = self.colorize(&label.into(), color);
        writeln!(
            self.buf,
            "{}{}{}",
            self.indent(),
            self.connector(false, is_last),
            label
        )?;
        Ok(())
    }

    fn dump_document(&mut self, document: &Document, is_last: bool) -> Result<()> {
        self.begin_node("Document", 37, true, is_last)?;

        let item = document.item();
        let show_leading = self.show_trivia && !document.leading_trivia.is_empty();
        let show_trailing = self.show_trivia && !document.trailing_trivia.is_empty();
        let child_count = show_leading as usize + item.is_some() as usize + show_trailing as usize;
        let mut index = 0;

        if show_leading {
            self.dump_trivia_group(
                "LeadingTrivia",
                &document.leading_trivia,
                index == child_count - 1,
            )?;
            index += 1;
        }

        if let Some(item) = item {
            self.dump_document_item(&item, index == child_count - 1)?;
            index += 1;
        }

        if show_trailing {
            self.dump_trivia_group(
                "TrailingTrivia",
                &document.trailing_trivia,
                index == child_count - 1,
            )?;
        }

        self.end_node(true);
        Ok(())
    }

    fn dump_document_item(&mut self, item: &DocumentItem, is_last: bool) -> Result<()> {
        match item {
            DocumentItem::Object(object) => self.dump_object(object, is_last),
            DocumentItem::Array(array) => self.dump_array(array, is_last),
            DocumentItem::Value(value) => self.dump_value(value, is_last),
        }
    }

    fn dump_object(&mut self, object: &hjson_edit::ast::Object, is_last: bool) -> Result<()> {
        let label = match object.delimiter_style {
            DelimiterStyle::Braced => "Object".to_string(),
            DelimiterStyle::Braceless => "Object (braceless)".to_string(),
        };
        self.begin_node(label, 36, false, is_last)?;

        let members = object.members();
        let show_leading = self.show_trivia && !object.leading_trivia.is_empty();
        let show_trailing = self.show_trivia && !object.trailing_trivia.is_empty();
        let child_count = show_leading as usize + members.len() + show_trailing as usize;
        let mut index = 0;

        if show_leading {
            self.dump_trivia_group(
                "LeadingTrivia",
                &object.leading_trivia,
                index == child_count - 1,
            )?;
            index += 1;
        }

        for member in &members {
            self.dump_member(member, index == child_count - 1)?;
            index += 1;
        }

        if show_trailing {
            self.dump_trivia_group(
                "TrailingTrivia",
                &object.trailing_trivia,
                index == child_count - 1,
            )?;
        }

        self.end_node(false);
        Ok(())
    }

    fn dump_array(&mut self, array: &hjson_edit::ast::Array, is_last: bool) -> Result<()> {
        let label = match array.delimiter_style {
            DelimiterStyle::Braced => "Array".to_string(),
            DelimiterStyle::Braceless => "Array (braceless)".to_string(),
        };
        self.begin_node(label, 35, false, is_last)?;

        let items = array.items();
        let show_leading = self.show_trivia && !array.leading_trivia.is_empty();
        let show_trailing = self.show_trivia && !array.trailing_trivia.is_empty();
        let child_count = show_leading as usize + items.len() + show_trailing as usize;
        let mut index = 0;

        if show_leading {
            self.dump_trivia_group(
                "LeadingTrivia",
                &array.leading_trivia,
                index == child_count - 1,
            )?;
            index += 1;
        }

        for item in &items {
            self.dump_array_item(item, index == child_count - 1)?;
            index += 1;
        }

        if show_trailing {
            self.dump_trivia_group(
                "TrailingTrivia",
                &array.trailing_trivia,
                index == child_count - 1,
            )?;
        }

        self.end_node(false);
        Ok(())
    }

    fn dump_array_item(&mut self, item: &ArrayItem, is_last: bool) -> Result<()> {
        self.begin_node("ArrayItem", 93, false, is_last)?;

        let value = item.value()?;
        let show_leading = self.show_trivia && !item.leading_trivia.is_empty();
        let show_trailing = self.show_trivia && !item.trailing_trivia.is_empty();
        let child_count = show_leading as usize + 1 + show_trailing as usize;
        let mut index = 0;

        if show_leading {
            self.dump_trivia_group(
                "LeadingTrivia",
                &item.leading_trivia,
                index == child_count - 1,
            )?;
            index += 1;
        }

        self.dump_value(&value, index == child_count - 1)?;
        index += 1;

        if show_trailing {
            self.dump_trivia_group(
                "TrailingTrivia",
                &item.trailing_trivia,
                index == child_count - 1,
            )?;
        }

        self.end_node(false);
        Ok(())
    }

    fn dump_member(&mut self, member: &hjson_edit::ast::Member, is_last: bool) -> Result<()> {
        self.begin_node("Member", 93, false, is_last)?;

        let key = member.key();
        let value = member.value();
        let show_leading = self.show_trivia && !member.leading_trivia.is_empty();
        let show_before = self.show_trivia && !member.before_colon_trivia.is_empty();
        let show_after = self.show_trivia && !member.after_colon_trivia.is_empty();
        let show_trailing = self.show_trivia && !member.trailing_trivia.is_empty();
        let child_count = show_leading as usize
            + key.is_some() as usize
            + show_before as usize
            + value.is_some() as usize
            + show_after as usize
            + show_trailing as usize;
        let mut index = 0;

        if show_leading {
            self.dump_trivia_group(
                "LeadingTrivia",
                &member.leading_trivia,
                index == child_count - 1,
            )?;
            index += 1;
        }

        if let Some(key) = key.as_ref() {
            self.dump_key(key, index == child_count - 1)?;
            index += 1;
        }

        if show_before {
            self.dump_trivia_group(
                "BeforeColonTrivia",
                &member.before_colon_trivia,
                index == child_count - 1,
            )?;
            index += 1;
        }

        if let Some(value) = value.as_ref() {
            self.dump_value(value, index == child_count - 1)?;
            index += 1;
        }

        if show_after {
            self.dump_trivia_group(
                "AfterColonTrivia",
                &member.after_colon_trivia,
                index == child_count - 1,
            )?;
            index += 1;
        }

        if show_trailing {
            self.dump_trivia_group(
                "TrailingTrivia",
                &member.trailing_trivia,
                index == child_count - 1,
            )?;
        }

        self.end_node(false);
        Ok(())
    }

    fn dump_key(&mut self, key: &hjson_edit::ast::Key, is_last: bool) -> Result<()> {
        let label = key
            .body_text()
            .map(|text| format!("Key {:?}", text))
            .unwrap_or_else(|| "Key".to_string());
        self.leaf(label, 93, is_last)
    }

    fn dump_value(&mut self, value: &hjson_edit::ast::Value, is_last: bool) -> Result<()> {
        self.begin_node("Value", 36, false, is_last)?;

        let content = value.content()?;
        self.dump_value_content(&content, true)?;

        self.end_node(false);
        Ok(())
    }

    fn dump_value_content(&mut self, content: &ValueContent, is_last: bool) -> Result<()> {
        match content {
            ValueContent::Object(object) => self.dump_object(object, is_last),
            ValueContent::Array(array) => self.dump_array(array, is_last),
            ValueContent::Scalar(scalar) => self.dump_scalar(scalar, is_last),
        }
    }

    fn dump_scalar(&mut self, scalar: &ScalarValue, is_last: bool) -> Result<()> {
        match scalar {
            ScalarValue::String(string) => {
                let label = match string.quote_style() {
                    QuoteStyle::DoubleQuoted => format!("String {:?}", string.body_text()),
                    QuoteStyle::Block => format!("BlockString {:?}", string.body_text()),
                    QuoteStyle::Unquoted => format!("UnquotedString {:?}", string.body_text()),
                };
                self.leaf(label, 32, is_last)
            }
            ScalarValue::Integer { raw, .. } => self.leaf(format!("Integer {}", raw), 35, is_last),
            ScalarValue::Float { raw, .. } => self.leaf(format!("Float {}", raw), 35, is_last),
            ScalarValue::Bool(true) => self.leaf("True".to_string(), 34, is_last),
            ScalarValue::Bool(false) => self.leaf("False".to_string(), 34, is_last),
            ScalarValue::Null => self.leaf("Null".to_string(), 34, is_last),
        }
    }

    fn dump_trivia_group(&mut self, label: &str, trivia: &[Trivia], is_last: bool) -> Result<()> {
        if !self.show_trivia || trivia.is_empty() {
            return Ok(());
        }

        self.begin_node(label.to_string(), 90, false, is_last)?;
        for (index, item) in trivia.iter().enumerate() {
            self.dump_trivia(item, index == trivia.len() - 1)?;
        }
        self.end_node(false);
        Ok(())
    }

    fn dump_trivia(&mut self, trivia: &Trivia, is_last: bool) -> Result<()> {
        match trivia {
            Trivia::Whitespace(text) => self.leaf(format!("Whitespace {:?}", text), 90, is_last),
            Trivia::Newline { kind, .. } => self.leaf(format!("Newline {:?}", kind), 90, is_last),
            Trivia::Comment(comment) => match comment.kind {
                CommentKind::Line => {
                    self.leaf(format!("LineComment {:?}", comment.body), 90, is_last)
                }
                CommentKind::Hash => {
                    self.leaf(format!("HashComment {:?}", comment.body), 90, is_last)
                }
                CommentKind::Block => {
                    self.leaf(format!("BlockComment {:?}", comment.body), 90, is_last)
                }
            },
            Trivia::Comma(text) => self.leaf(format!("Comma {:?}", text), 90, is_last),
        }
    }
}

trait SyntaxKindDebugExt {
    fn color(&self) -> i32;
    fn colorize(&self, text: &str) -> String;
}

impl SyntaxKindDebugExt for SyntaxKind {
    fn color(&self) -> i32 {
        match self {
            // emphasize errors
            SyntaxKind::Error => 91, // bright red

            // distinguish key/value
            SyntaxKind::Key => 93,   // bright yellow
            SyntaxKind::Value => 36, // cyan

            // literals
            SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString => 32, // green
            SyntaxKind::Integer | SyntaxKind::Float => 35, // magenta
            SyntaxKind::True | SyntaxKind::False | SyntaxKind::Null => 34, // blue

            // comments & whitespace
            SyntaxKind::LineComment | SyntaxKind::HashComment | SyntaxKind::BlockComment => 90,
            SyntaxKind::Whitespace | SyntaxKind::Newline => 90,

            // punctuation and structural nodes
            SyntaxKind::LBrace
            | SyntaxKind::RBrace
            | SyntaxKind::LBracket
            | SyntaxKind::RBracket
            | SyntaxKind::Colon
            | SyntaxKind::Comma
            | SyntaxKind::Root
            | SyntaxKind::Object
            | SyntaxKind::Array
            | SyntaxKind::Member
            | _ => 37,
        }
    }

    fn colorize(&self, text: &str) -> String {
        let color_code = self.color();
        format!("\x1b[{}m{}\x1b[0m", color_code, text)
    }
}

struct FilterPeekable<I>
where
    I: Iterator<Item = rowan::NodeOrToken<RedNode, RedToken>>,
{
    iter: I,
    show_trivia: bool,
    peeked: Option<rowan::NodeOrToken<RedNode, RedToken>>,
}

impl<I> FilterPeekable<I>
where
    I: Iterator<Item = rowan::NodeOrToken<RedNode, RedToken>>,
{
    fn new(iter: I, show_trivia: bool) -> Self {
        Self {
            iter,
            show_trivia,
            peeked: None,
        }
    }

    fn next(&mut self) -> Option<rowan::NodeOrToken<RedNode, RedToken>> {
        if let Some(item) = self.peeked.take() {
            return Some(item);
        }

        while let Some(item) = self.iter.next() {
            match &item {
                rowan::NodeOrToken::Token(t) if t.kind().is_trivia() && !self.show_trivia => {
                    continue
                }
                rowan::NodeOrToken::Node(n)
                    if n.kind() == SyntaxKind::Comment && !self.show_trivia =>
                {
                    continue
                }
                _ => return Some(item),
            }
        }

        None
    }

    fn peek(&mut self) -> Option<&rowan::NodeOrToken<RedNode, RedToken>> {
        if self.peeked.is_some() {
            return self.peeked.as_ref();
        }

        while let Some(item) = self.iter.next() {
            match &item {
                rowan::NodeOrToken::Token(t) if t.kind().is_trivia() && !self.show_trivia => {
                    continue
                }
                rowan::NodeOrToken::Node(n)
                    if n.kind() == SyntaxKind::Comment && !self.show_trivia =>
                {
                    continue
                }
                _ => {
                    self.peeked = Some(item);
                    return self.peeked.as_ref();
                }
            }
        }

        None
    }
}
