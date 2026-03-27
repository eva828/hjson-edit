use std::fmt::Write as FmtWrite;
use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};

use hjson_edit::{high_level::HjsonDocument, RedNode, RedToken, SyntaxKind};

pub fn run_parse(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
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
            let red = document.root();

            let console_config = ConsoleConfig::new(simple, color_mode);
            let mut tree_view = TreeViewBuilder::new(console_config, show_trivia);
            tree_view.dump_node(red, true)?;
            print!("{}", tree_view.buf);
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
            | SyntaxKind::Pair
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
                _ => {
                    self.peeked = Some(item);
                    return self.peeked.as_ref();
                }
            }
        }

        None
    }
}
