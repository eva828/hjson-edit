//! 開発用ツール: 解析結果を正準形（型付き値）で TSV 出力する。
//!
//! hjson-cs のオラクル（`tools/cs-oracle`）と突き合わせるために使う。
//!
//! ```text
//! cargo run --release --example canon -- cases <入力TSV>
//! cargo run --release --example canon -- dir <ディレクトリ>
//! ```

use hjson_edit::conformance::{evaluate, unescape};
use std::path::{Path, PathBuf};

fn run_cases(file: &Path) {
    let text = std::fs::read_to_string(file).unwrap();
    for line in text.lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let mut parts = line.split('\t');
        let id = parts.next().unwrap();
        let input = unescape(parts.next().unwrap());
        println!("{id}\t{}", evaluate(&input));
    }
}

fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, PathBuf)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(&path, root, out);
        } else if path.extension().is_some_and(|e| e == "hjson") {
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            out.push((name, path));
        }
    }
}

fn run_dir(dir: &Path) {
    let mut files = Vec::new();
    walk(dir, dir, &mut files);
    files.sort();
    for (name, path) in files {
        let text = std::fs::read_to_string(&path).unwrap();
        println!("{name}\t{}", evaluate(&text));
    }
}

fn main() {
    let mode = std::env::args().nth(1).expect("mode required");
    let path = std::env::args().nth(2).expect("path required");
    match mode.as_str() {
        "cases" => run_cases(Path::new(&path)),
        "dir" => run_dir(Path::new(&path)),
        other => panic!("unknown mode: {other}"),
    }
}
