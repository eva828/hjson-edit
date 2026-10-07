use hjson_edit::HjsonDocument;

/// ネットワークと GitHub API へのアクセスが必須のため、既定では実行しない。
/// `cargo test --all-features --test tmlhonyaku_parse -- --ignored`
#[test]
#[ignore = "ネットワーク必須（ExternalLocalizer/TMLHonyaku を参照）"]
fn parse_all_tmlhonyaku_hjson_files() {
    let client = reqwest::blocking::Client::builder()
        .user_agent("hjson-edit-test")
        .build()
        .expect("Failed to build HTTP client");

    let tree_url =
        "https://api.github.com/repos/ExternalLocalizer/TMLHonyaku/git/trees/main?recursive=1";
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
    assert!(total > 0, "No .hjson files found in TMLHonyaku repository");
    eprintln!("Found {} .hjson files. Starting parse...", total);

    let mut failures: Vec<(String, String)> = Vec::new();

    for (i, path) in hjson_files.iter().enumerate() {
        let raw_url = format!(
            "https://raw.githubusercontent.com/ExternalLocalizer/TMLHonyaku/main/{}",
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

    if !failures.is_empty() {
        eprintln!("\nFailed files:");
        for (path, err) in &failures {
            eprintln!("  {}: {}", path, err);
        }
        panic!("{} of {} files failed to parse", failures.len(), total);
    }
}
