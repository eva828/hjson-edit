use hjson_edit::high_level::{
    ops::object::HjsonDocumentEdit, path::HjsonObjectPath, value::HjsonValue, HjsonDocument,
};

// ------------------------------------------------------------------
// BOM はパーサーでは通常の文字として扱う（基準実装 hjson-cs と同じ）。
// ファイル先頭の BOM 除去は CLI（ファイル読み込み層）の役割。
// 参考: RFC 8259 9.1（BOM は無視してもよい）
// ------------------------------------------------------------------

#[test]
fn bom_at_start_is_an_ordinary_character() {
    // `\uFEFF{a: 1}` はキーとして解釈できないため、行全体が quoteless 文字列になる。
    let doc = HjsonDocument::parse("\u{FEFF}{a: 1}").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    assert_eq!(value, HjsonValue::String("\u{FEFF}{a: 1}".to_string()));
}

#[test]
fn bom_only_input_is_a_single_value() {
    let doc = HjsonDocument::parse("\u{FEFF}").unwrap();
    let value = doc.get(HjsonObjectPath::parse("").unwrap()).unwrap();
    assert_eq!(value, HjsonValue::String("\u{FEFF}".to_string()));
}
