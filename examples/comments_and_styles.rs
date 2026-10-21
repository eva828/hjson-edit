//! コメントを保ったまま値を差し替え、変更したコンテナだけを整形する例。

use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonObjectPath, HjsonValue};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let input = r#"{
  // アプリケーションの設定
  name: my-app
  version: 1.0.0
}"#;

    let document = HjsonDocument::parse(input)?;

    // 編集はテキストを最小限だけ変更する。既存のコメントと体裁はそのまま残る。
    let edited = document
        .set(
            HjsonObjectPath::parse("description")?,
            "A multi-line\ndescription goes here.",
        )?
        .set(HjsonObjectPath::parse("type")?, "library")?
        .set(
            HjsonObjectPath::parse("keywords")?,
            HjsonValue::Array(vec![
                HjsonValue::String("hjson".to_string()),
                HjsonValue::String("parser".to_string()),
            ]),
        )?;

    // 値の種類は保たれる: 文字列 "true" は真偽値にならない。
    let edited = edited.set(HjsonObjectPath::parse("enabled")?, "true")?;
    assert_eq!(
        edited.get(HjsonObjectPath::parse("enabled")?)?,
        HjsonValue::String("true".to_string())
    );

    println!("=== Edited (minimal text changes) ===");
    println!("{edited}");

    // 体裁を整えたいときは、変更したコンテナだけを整形する。
    let config = edited.current_format_config();
    let mut formatted = edited.clone();
    for container in edited.touched_containers().to_vec() {
        formatted = formatted.format_container(&container, &config)?;
    }

    println!("=== Formatted (only the changed containers) ===");
    println!("{formatted}");

    Ok(())
}
