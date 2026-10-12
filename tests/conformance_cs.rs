//! hjson-cs 準拠テスト（レイヤ B: cs 固有ケース）。
//!
//! `tests/fixtures/hjson-cs/inputs.tsv` の入力に対する期待値は、hjson-cs を実行して
//! 生成した `cases.tsv` と突き合わせる。差分は `deviations.tsv` に明示されたものだけを
//! 許し、**実装の実差分集合とリストが完全一致**しなければ失敗する（新しい逸脱も、
//! 修正済みなのにリストに残っている項目も検出する）。
//!
//! - 期待値の再生成: `tools/gen_fixtures.sh`
//! - 逸脱リストの更新: `CONFORMANCE_BLESS=1 cargo test --test conformance_cs`

use hjson_edit::conformance::{evaluate, unescape};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hjson-cs")
}

/// TSV（`#` 行と空行は無視）を読み、(id, 残りの列) を返す。
fn read_tsv(path: &Path) -> Vec<(String, Vec<String>)> {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} を読めない: {e}", path.display()));
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let mut parts = line.split('\t');
            let id = parts.next().unwrap().to_string();
            let rest: Vec<String> = parts.map(|column| column.to_string()).collect();
            assert!(!rest.is_empty(), "列が足りない: {line}");
            (id, rest)
        })
        .collect()
}

fn inputs() -> BTreeMap<String, String> {
    read_tsv(&fixtures_dir().join("inputs.tsv"))
        .into_iter()
        .map(|(id, columns)| {
            assert_eq!(columns.len(), 2, "inputs.tsv は id/入力/note の 3 列: {id}");
            (id, columns[0].clone())
        })
        .collect()
}

/// cases.tsv の期待値（id -> (状態, 値)）。
fn expected() -> BTreeMap<String, (String, String)> {
    read_tsv(&fixtures_dir().join("cases.tsv"))
        .into_iter()
        .map(|(id, columns)| {
            let status = columns[0].clone();
            let value = columns.get(1).cloned().unwrap_or_default();
            (id, (status, value))
        })
        .collect()
}

/// deviations.tsv（id -> (判断, 理由)）。
fn deviations() -> BTreeMap<String, (String, String)> {
    let path = fixtures_dir().join("deviations.tsv");
    if !path.exists() {
        return BTreeMap::new();
    }
    read_tsv(&path)
        .into_iter()
        .map(|(id, columns)| {
            let judgment = columns.first().cloned().unwrap_or_default();
            let reason = columns.get(1).cloned().unwrap_or_default();
            (id, (judgment, reason))
        })
        .collect()
}

fn write_deviations(actual: &BTreeSet<String>, declared: &BTreeMap<String, (String, String)>) {
    let mut out = String::from(
        "# hjson-cs 準拠テストの逸脱リスト（実装の実差分集合と完全一致させる）\n#\n\
         # 判断: 修正予定 / 許容\n\
         # 更新: CONFORMANCE_BLESS=1 cargo test --test conformance_cs\n\
         #\n# id<TAB>判断<TAB>理由\n",
    );
    for id in actual {
        let (judgment, reason) = declared
            .get(id)
            .cloned()
            .unwrap_or_else(|| ("修正予定".to_string(), String::new()));
        out.push_str(&format!("{id}\t{judgment}\t{reason}\n"));
    }
    std::fs::write(fixtures_dir().join("deviations.tsv"), out).expect("deviations.tsv を書けない");
}

#[test]
fn cases_match_reference_or_are_declared() {
    let inputs = inputs();
    let expected = expected();
    let declared = deviations();

    for id in expected.keys() {
        assert!(inputs.contains_key(id), "inputs.tsv に入力がない: {id}");
    }

    let mut actual: BTreeSet<String> = BTreeSet::new();
    let mut report = Vec::new();
    for (id, (exp_status, exp_value)) in &expected {
        let got = evaluate(&unescape(&inputs[id]));
        let mut parts = got.splitn(2, '\t');
        let status = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        let matched = status == exp_status && (exp_status != "OK" || value == exp_value);
        if !matched {
            actual.insert(id.clone());
            report.push(format!(
                "  {id}: 基準={exp_status} {exp_value} / 実装={status} {value}"
            ));
        }
    }

    if std::env::var("CONFORMANCE_BLESS").is_ok() {
        write_deviations(&actual, &declared);
        eprintln!("deviations.tsv を更新: {} 件", actual.len());
        return;
    }

    let declared_ids: BTreeSet<String> = declared.keys().cloned().collect();
    let undeclared: Vec<&String> = actual.difference(&declared_ids).collect();
    let stale: Vec<&String> = declared_ids.difference(&actual).collect();

    assert!(
        undeclared.is_empty(),
        "宣言されていない逸脱が {} 件あります（deviations.tsv に追記するか修正してください）:\n{}",
        undeclared.len(),
        report.join("\n")
    );
    assert!(
        stale.is_empty(),
        "逸脱リストの項目が実装と一致しません（修正済みなら削除してください）: {stale:?}"
    );

    println!(
        "ケース {} 件: 基準と一致 {} 件 / 宣言済みの逸脱 {} 件",
        expected.len(),
        expected.len() - actual.len(),
        actual.len()
    );
}
