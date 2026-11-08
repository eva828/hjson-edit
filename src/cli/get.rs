use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};
use hjson_edit::high_level::{
    ops::object::HjsonDocumentEdit, path::HjsonObjectPath, value::HjsonValue,
};
use hjson_edit::{HjsonDocument, HjsonEditError};

/// JSON の 1 段分の字下げ。
const INDENT: &str = "  ";

pub fn get_command() -> Command {
    Command::new("get")
        .about("Print the value at a path as JSON")
        .arg(
            Arg::new("file")
                .required(true)
                .value_parser(clap::value_parser!(PathBuf))
                .help("HJSON file to read (use - for standard input)"),
        )
        .arg(
            Arg::new("path")
                .required(true)
                .value_name("PATH")
                .help("Path of the value (an empty string for the document)"),
        )
        .arg(
            Arg::new("raw")
                .long("raw")
                .action(ArgAction::SetTrue)
                .help("Print a string value without quotes"),
        )
}

pub fn run_get(matches: &clap::ArgMatches) -> Result<()> {
    let file: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let content = super::read_input(file)?;
    let doc = match HjsonDocument::parse(&content) {
        Ok(document) => {
            super::warn_skipped_input(&document);
            document
        }
        Err(error) => anyhow::bail!("{error}"),
    };

    let raw_path = matches
        .get_one::<String>("path")
        .context("missing path argument")?;
    let path = HjsonObjectPath::parse(raw_path.clone())?;
    let value = doc.get(path.clone())?;

    if matches.get_flag("raw") {
        return match value {
            HjsonValue::String(text) => {
                println!("{text}");
                Ok(())
            }
            HjsonValue::Null
            | HjsonValue::Bool(_)
            | HjsonValue::Integer(_)
            | HjsonValue::Float(_)
            | HjsonValue::Array(_)
            | HjsonValue::Object(_) => Err(HjsonEditError::InvalidTarget {
                path,
                reason: "--raw needs a string value".to_string(),
            }
            .into()),
        };
    }

    println!("{}", to_json(&value, 0)?);
    Ok(())
}

fn indent(depth: usize) -> String {
    INDENT.repeat(depth)
}

/// 値を正規化した JSON にする。コンテナは複数行、スカラーは 1 行。
fn to_json(value: &HjsonValue, depth: usize) -> Result<String> {
    Ok(match value {
        HjsonValue::Null => "null".to_string(),
        HjsonValue::Bool(value) => value.to_string(),
        HjsonValue::Integer(value) => value.to_string(),
        HjsonValue::Float(value) if value.is_finite() => format!("{value:?}"),
        HjsonValue::Float(_) => {
            return Err(HjsonEditError::ValueNotRepresentable {
                reason: "a non-finite number has no JSON representation".to_string(),
            }
            .into())
        }
        HjsonValue::String(text) => json_string(text),
        HjsonValue::Array(items) => {
            if items.is_empty() {
                return Ok("[]".to_string());
            }
            let inner = items
                .iter()
                .map(|item| {
                    Ok(format!(
                        "{}{}",
                        indent(depth + 1),
                        to_json(item, depth + 1)?
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            format!("[\n{}\n{}]", inner.join(",\n"), indent(depth))
        }
        HjsonValue::Object(entries) => {
            if entries.is_empty() {
                return Ok("{}".to_string());
            }
            let inner = entries
                .iter()
                .map(|(key, value)| {
                    Ok(format!(
                        "{}{}: {}",
                        indent(depth + 1),
                        json_string(key),
                        to_json(value, depth + 1)?
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            format!("{{\n{}\n{}}}", inner.join(",\n"), indent(depth))
        }
    })
}

fn json_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_string_escapes_control_characters() {
        assert_eq!(
            json_string("a\"b\\c\nd\te\u{1}"),
            "\"a\\\"b\\\\c\\nd\\te\\u0001\""
        );
    }

    #[test]
    fn json_string_keeps_unicode_as_is() {
        assert_eq!(json_string("日本語"), "\"日本語\"");
    }
}
