#![cfg(feature = "net-corpus")]

//! 外部コーパスを取得してまとめてパースする統合テスト。
//!
//! ネットワークと GitHub API へのアクセスが必須のため、既定では実行しない。
//! `cargo test --features net-corpus --test tmlhonyaku_parse -- --ignored`

use hjson_edit::HjsonDocument;

/// 取得先のリポジトリ。
const CORPUS_REPO: &str = "ExternalLocalizer/TMLHonyaku";
/// 取得先を固定するコミット。コーパスを更新する場合はこの定数を書き換える。
const CORPUS_COMMIT: &str = "26f3d22fc0df2815a81b9537d259c79f5d512bb9";

#[test]
#[ignore = "ネットワーク必須"]
fn parse_all_tmlhonyaku_hjson_files() {
    let client = reqwest::blocking::Client::builder()
        .user_agent("hjson-edit-test")
        .build()
        .expect("Failed to build HTTP client");

    let tree_url =
        format!("https://api.github.com/repos/{CORPUS_REPO}/git/trees/{CORPUS_COMMIT}?recursive=1");
    let resp = client
        .get(tree_url)
        .send()
        .expect("Failed to fetch git tree from GitHub API");
    assert!(
        resp.status().is_success(),
        "GitHub API returned {}",
        resp.status()
    );

    let tree: serde_json::Value = resp.json().expect("Invalid JSON in tree response");
    let entries = tree["tree"]
        .as_array()
        .expect("Missing 'tree' array in response");

    let hjson_files: Vec<&str> = entries
        .iter()
        .filter_map(|entry| {
            let path = entry["path"].as_str()?;
            (entry["type"].as_str() == Some("blob") && path.ends_with(".hjson")).then_some(path)
        })
        .collect();

    let total = hjson_files.len();
    assert!(
        total > 0,
        "No .hjson files found in {CORPUS_REPO} at {CORPUS_COMMIT}"
    );
    eprintln!("Found {} .hjson files. Starting parse...", total);

    let mut failures: Vec<(String, String)> = Vec::new();

    for (i, path) in hjson_files.iter().enumerate() {
        let raw_url = format!(
            "https://raw.githubusercontent.com/{CORPUS_REPO}/{CORPUS_COMMIT}/{}",
            path
        );
        let content = match client.get(&raw_url).send() {
            Ok(r) if r.status().is_success() => match r.text() {
                Ok(text) => text,
                Err(e) => {
                    failures.push((path.to_string(), format!("body read error: {}", e)));
                    continue;
                }
            },
            Ok(r) => {
                failures.push((path.to_string(), format!("HTTP {}", r.status())));
                continue;
            }
            Err(e) => {
                failures.push((path.to_string(), format!("network error: {}", e)));
                continue;
            }
        };

        if let Err(e) = HjsonDocument::parse(&content) {
            failures.push((path.to_string(), e.to_string()));
        }

        if (i + 1) % 100 == 0 {
            eprintln!("  Progress: {}/{} parsed", i + 1, total);
        }
    }

    let passed = total - failures.len();
    eprintln!(
        "\nResults: {}/{} passed, {} failed",
        passed,
        total,
        failures.len()
    );

    // 宣言済みの逸脱（tests/fixtures/hjson-cs/corpus_deviations.tsv）だけを許す。
    // 新しい失敗も、修正済みなのに残っている項目も検出する。
    let declared = declared_corpus_deviations();
    let failed: std::collections::BTreeSet<String> =
        failures.iter().map(|(path, _)| path.clone()).collect();

    if !failures.is_empty() {
        eprintln!("\nFailed files:");
        for (path, err) in &failures {
            eprintln!("  {}: {}", path, err);
        }
    }

    let undeclared: Vec<&String> = failed.difference(&declared).collect();
    let stale: Vec<&String> = declared.difference(&failed).collect();
    assert!(
        undeclared.is_empty(),
        "宣言されていない失敗があります: {undeclared:?}",
    );
    assert!(
        stale.is_empty(),
        "逸脱リストの項目が解消されています（削除してください）: {stale:?}",
    );
}

/// コーパスの逸脱リスト（パスのみ）を読む。
fn declared_corpus_deviations() -> std::collections::BTreeSet<String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/hjson-cs/corpus_deviations.tsv");
    let Ok(text) = std::fs::read_to_string(path) else {
        return std::collections::BTreeSet::new();
    };
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split('\t').next().map(str::to_string))
        .collect()
}
