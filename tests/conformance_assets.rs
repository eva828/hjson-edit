#![cfg(feature = "net-conformance")]

//! hjson-cs 準拠テスト（レイヤ A: 公式テスト資産）。
//!
//! hjson-cs `test/assets` の `*_test.{hjson,json}` を固定コミットから取得し、
//! hjson-cs を実行して生成した期待値（`assets_cs.tsv`）と突き合わせる。
//! ネットワークが必須のため既定では実行しない。
//!
//! ```text
//! cargo test --features net-conformance --test conformance_assets -- --ignored
//! CONFORMANCE_BLESS=1 cargo test --features net-conformance --test conformance_assets -- --ignored   # 逸脱リスト更新
//! ```

use hjson_edit::conformance::evaluate;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// 取得先のリポジトリとコミット（tools/cs-oracle/reference.env と同じ値）。
const REPO: &str = "hjson/hjson-cs";
const COMMIT: &str = "777ec1a554aabb43c68950d52b6183547b815281";

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hjson-cs")
}

fn read_tsv(path: &Path) -> Vec<(String, Vec<String>)> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} を読めない: {e}", path.display()));
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split('\t');
            let id = parts.next().unwrap().to_string();
            let rest: Vec<String> = parts.map(|column| column.to_string()).collect();
            (id, rest)
        })
        .collect()
}

fn expected() -> BTreeMap<String, (String, String)> {
    read_tsv(&fixtures_dir().join("assets_cs.tsv"))
        .into_iter()
        .map(|(id, columns)| {
            let status = columns.first().cloned().unwrap_or_default();
            let value = columns.get(1).cloned().unwrap_or_default();
            (id, (status, value))
        })
        .collect()
}

fn deviations() -> BTreeMap<String, (String, String)> {
    let path = fixtures_dir().join("asset_deviations.tsv");
    if !path.exists() {
        return BTreeMap::new();
    }
    read_tsv(&path)
        .into_iter()
        .map(|(id, columns)| {
            (
                id,
                (
                    columns.first().cloned().unwrap_or_default(),
                    columns.get(1).cloned().unwrap_or_default(),
                ),
            )
        })
        .collect()
}

fn write_deviations(actual: &BTreeSet<String>, declared: &BTreeMap<String, (String, String)>) {
    let mut out = String::from(
        "# 公式テスト資産（レイヤ A）の逸脱リスト\n#\n\
         # 更新: CONFORMANCE_BLESS=1 cargo test --features net-conformance --test conformance_assets -- --ignored\n\
         #\n# ファイル名<TAB>判断<TAB>理由\n",
    );
    for id in actual {
        let (judgment, reason) = declared
            .get(id)
            .cloned()
            .unwrap_or_else(|| ("修正予定".to_string(), String::new()));
        out.push_str(&format!("{id}\t{judgment}\t{reason}\n"));
    }
    std::fs::write(fixtures_dir().join("asset_deviations.tsv"), out)
        .expect("asset_deviations.tsv を書けない");
}

#[test]
#[ignore = "ネットワーク必須"]
fn official_assets_match_reference_or_are_declared() {
    let expected = expected();
    let declared = deviations();

    let client = reqwest::blocking::Client::builder()
        .user_agent("hjson-edit-test")
        .build()
        .expect("HTTP クライアントを作れない");

    let mut actual: BTreeSet<String> = BTreeSet::new();
    let mut report = Vec::new();
    for (name, (exp_status, exp_value)) in &expected {
        let url = format!("https://raw.githubusercontent.com/{REPO}/{COMMIT}/test/assets/{name}");
        let response = client.get(&url).send().expect("取得に失敗");
        assert!(
            response.status().is_success(),
            "{url}: {}",
            response.status()
        );
        let text = response.text().expect("本文を読めない");

        let got = evaluate(&text);
        let mut parts = got.splitn(2, '\t');
        let status = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        if status != exp_status || (exp_status == "OK" && value != exp_value) {
            actual.insert(name.clone());
            report.push(format!(
                "  {name}: 基準={exp_status} {exp_value} / 実装={status} {value}"
            ));
        }
    }

    if std::env::var("CONFORMANCE_BLESS").is_ok() {
        write_deviations(&actual, &declared);
        eprintln!("asset_deviations.tsv を更新: {} 件", actual.len());
        return;
    }

    let declared_ids: BTreeSet<String> = declared.keys().cloned().collect();
    let undeclared: Vec<&String> = actual.difference(&declared_ids).collect();
    let stale: Vec<&String> = declared_ids.difference(&actual).collect();
    assert!(
        undeclared.is_empty(),
        "宣言されていない逸脱が {} 件あります:\n{}",
        undeclared.len(),
        report.join("\n")
    );
    assert!(
        stale.is_empty(),
        "逸脱リストの項目が実装と一致しません（修正済みなら削除してください）: {stale:?}"
    );

    println!(
        "公式資産 {} 件: 基準と一致 {} 件 / 宣言済みの逸脱 {} 件",
        expected.len(),
        expected.len() - actual.len(),
        actual.len()
    );
}
