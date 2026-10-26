//! `check`: 2 つの文書が意味的に同じかを調べる。
//!
//! レイアウト（空白・空行・字下げ）やコメントは値ではないため、既定では値だけを比べる。
//! レビューで「整形しただけか、値を変えたか」を機械的に判定するために使う。

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, Command};
use hjson_edit::high_level::ops::object::HjsonDocumentEdit;
use hjson_edit::high_level::path::HjsonObjectPath;
use hjson_edit::{HjsonDocument, HjsonValue};
use std::path::PathBuf;

/// 差分として表示するパスの上限。
const MAX_REPORTED_PATHS: usize = 20;

pub fn check_command() -> Command {
    Command::new("check")
        .about("Check that two HJSON files have the same values (layout and comments are ignored)")
        .arg(
            Arg::new("file")
                .required(true)
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("HJSON file to read"),
        )
        .arg(
            Arg::new("other")
                .value_parser(clap::value_parser!(std::path::PathBuf))
                .help("Second HJSON file (omit to only validate the first)"),
        )
        .arg(
            Arg::new("comments")
                .long("comments")
                .action(ArgAction::SetTrue)
                .help("Also require the comments (their text and order) to match"),
        )
}

pub fn run_check(matches: &clap::ArgMatches) -> Result<()> {
    let path: &PathBuf = matches.get_one("file").context("missing file argument")?;
    let document = parse_file(path)?;

    let Some(other_path) = matches.get_one::<PathBuf>("other") else {
        println!("ok: {} parses", path.display());
        return Ok(());
    };

    let other = parse_file(other_path)?;
    let empty = HjsonObjectPath::from_segments(Vec::new());
    let left = document.get(empty.clone())?;
    let right = other.get(empty.clone())?;

    let mut differences = Vec::new();
    collect_differences(&empty, &left, &right, &mut differences);

    if !differences.is_empty() {
        for difference in differences.iter().take(MAX_REPORTED_PATHS) {
            eprintln!("{difference}");
        }
        if differences.len() > MAX_REPORTED_PATHS {
            eprintln!("… and {} more", differences.len() - MAX_REPORTED_PATHS);
        }
        anyhow::bail!(
            "values differ: {}",
            format_args!("{} path(s)", differences.len())
        );
    }

    if matches.get_flag("comments") && document.all_comments() != other.all_comments() {
        anyhow::bail!("comments differ");
    }

    println!(
        "values are equal: {} and {}",
        path.display(),
        other_path.display()
    );
    Ok(())
}

fn parse_file(path: &std::path::Path) -> Result<HjsonDocument> {
    let content = super::read_input(path)?;
    let document = HjsonDocument::parse(&content)
        .with_context(|| format!("Invalid HJSON: {}", path.display()))?;
    super::warn_skipped_input(&document);

    Ok(document)
}

/// 値を比べて、違うパスを集める。
fn collect_differences(
    path: &HjsonObjectPath,
    left: &HjsonValue,
    right: &HjsonValue,
    out: &mut Vec<String>,
) {
    match (left, right) {
        (HjsonValue::Object(left), HjsonValue::Object(right)) => {
            for (key, value) in left {
                let child = child_path(path, hjson_edit::PathSegment::Key(key.clone()));
                match right.iter().find(|(other, _)| other == key) {
                    Some((_, other)) => collect_differences(&child, value, other, out),
                    None => out.push(format!("{child}: only in the first document")),
                }
            }
            for (key, _) in right {
                if !left.iter().any(|(other, _)| other == key) {
                    let child = child_path(path, hjson_edit::PathSegment::Key(key.clone()));
                    out.push(format!("{child}: only in the second document"));
                }
            }
        }
        (HjsonValue::Array(left), HjsonValue::Array(right)) => {
            for (index, value) in left.iter().enumerate() {
                let child = child_path(
                    path,
                    hjson_edit::PathSegment::Array(hjson_edit::ArrayPathSegment::Index(index)),
                );
                match right.get(index) {
                    Some(other) => collect_differences(&child, value, other, out),
                    None => out.push(format!("{child}: only in the first document")),
                }
            }
            for index in left.len()..right.len() {
                let child = child_path(
                    path,
                    hjson_edit::PathSegment::Array(hjson_edit::ArrayPathSegment::Index(index)),
                );
                out.push(format!("{child}: only in the second document"));
            }
        }
        _ => {
            if !left.same_value(right) {
                out.push(format!("{path}: {left:?} -> {right:?}"));
            }
        }
    }
}

fn child_path(path: &HjsonObjectPath, segment: hjson_edit::PathSegment) -> HjsonObjectPath {
    let mut segments = path.segments().to_vec();
    segments.push(segment);
    HjsonObjectPath::from_segments(segments)
}
