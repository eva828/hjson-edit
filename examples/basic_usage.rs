//! 値の読み書きと配列への追加の基本。

use hjson_edit::{HjsonDocument, HjsonDocumentEdit, HjsonObjectPath};
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let input = r#"{
  name: example
  dependencies: {
    clap: 4.5
  }
  features: [
    default
  ]
}"#;

    let document = HjsonDocument::parse(input)?;

    // ドット記法と配列の index で値を読む
    let clap = document.get(HjsonObjectPath::parse("dependencies.clap")?)?;
    println!("clap version: {clap:?}");

    // 編集はどれも新しい文書を返す（元の文書は変わらない）
    let updated = document
        .set(HjsonObjectPath::parse("version")?, "0.1.0")? // 無いキーは追加
        .set(HjsonObjectPath::parse("dependencies.clap")?, "4.6")? // 既存の値は差し替え
        .set(HjsonObjectPath::parse("features[-]")?, "extra")? // 配列の末尾へ追加
        .remove(HjsonObjectPath::parse("name")?)?; // キーを削除

    println!("\n=== Modified ===");
    println!("{updated}");

    Ok(())
}
