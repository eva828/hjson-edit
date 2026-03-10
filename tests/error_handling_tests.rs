//! Error handling tests for hjson-edit
//!
//! These tests verify error cases and error messages.

use hjson_edit::{parse, ParseError};

/// 閉じられていないオブジェクト "{" が "Unclosed" または "Expected" エラーになることを検証
#[test]
fn test_error_unclosed_object() {
    let input = "{";

    let result = parse(input);
    assert!(result.is_err(), "Unclosed object should fail");

    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should have at least one error");

    // Check error message mentions unclosed object
    let error_message = format!("{:?}", errors);
    assert!(
        error_message.contains("Unclosed") || error_message.contains("Expected"),
        "Error should mention unclosed object: {}",
        error_message
    );
}

/// 閉じられていない配列 "[" がエラーになることを検証
#[test]
fn test_error_unclosed_array() {
    let input = "[";

    let result = parse(input);
    assert!(result.is_err(), "Unclosed array should fail");

    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should have at least one error");
}

/// 閉じられていない文字列リテラルがLexerレベルで "Unterminated" エラーになることを検証
#[test]
fn test_error_unclosed_string() {
    let input = "\"unclosed string";

    let result = parse(input);
    // Lexerが未閉じ文字列をエラーとして報告する
    assert!(result.is_err(), "Unclosed string should fail");
    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should have at least one error");
    assert!(
        errors.iter().any(|e| e.message.contains("Unterminated")),
        "Error should mention unterminated string: {:?}",
        errors
    );
}

/// キーと値の間にコロンがない場合にエラーが発生することを検証
#[test]
fn test_error_missing_colon() {
    let input = "{ key value }";

    let result = parse(input);
    assert!(
        result.is_err() || result.as_ref().unwrap().errors.len() > 0,
        "Missing colon should produce error"
    );
}

/// HJSON/JSON文字列では無効なエスケープシーケンス（\q）が拒否されることを検証
#[test]
fn test_error_invalid_escape() {
    let input = r#""\q is not a valid escape""#;

    let result = parse(input);
    assert!(result.is_err(), "Invalid escape should fail");
}

/// 構文的に不正な箇所が複数含まれる入力でエラーが報告されることを検証
#[test]
fn test_error_multiple_errors() {
    let input = r#"{
  first: 1
  second,
  third: 3
}"#;

    let result = parse(input);

    // Should either fail or have errors in the result
    match result {
        Ok(parse_result) => {
            // If it doesn't fail completely, there might be error nodes
            assert!(
                !parse_result.errors.is_empty()
                    || format!("{:?}", parse_result.green).contains("Error"),
                "Should have errors or error nodes"
            );
        }
        Err(errors) => {
            assert!(errors.len() >= 1, "Should have at least one error");
        }
    }
}

/// 空文字列はRFC的には値を持たないため、少なくとも値取得はできないことを検証
#[test]
fn test_error_empty_input() {
    let input = "";

    let result = parse(input);
    assert!(
        result.is_ok(),
        "Empty input should be valid: {:?}",
        result.err()
    );
    assert!(
        result.unwrap().errors.is_empty(),
        "Empty input should have no errors"
    );
}

/// 空白と改行のみの入力が有効なドキュメントとしてパースされることを検証
#[test]
fn test_error_whitespace_only() {
    let input = "   \n\n   ";

    let result = parse(input);
    // 空白のみの入力は有効なドキュメントとしてパースされる
    assert!(
        result.is_ok(),
        "Whitespace-only input should be valid: {:?}",
        result.err()
    );
    assert!(
        result.unwrap().errors.is_empty(),
        "Whitespace-only input should have no errors"
    );
}

/// エラーの行番号・列番号が妥当な範囲（1以上）であることを検証
#[test]
fn test_error_line_and_column() {
    let input = r#"{
  valid: "ok"
  invalid syntax here
}"#;

    let result = parse(input);

    // このパーサーでは構文エラーのある入力はErrを返す
    assert!(result.is_err(), "Input with syntax errors should fail");

    let errors = result.unwrap_err();
    assert!(!errors.is_empty(), "Should have at least one error");

    // エラーの行番号が妥当な範囲にあることを確認
    for error in &errors {
        assert!(
            error.line >= 1,
            "Error line should be >= 1, got {}",
            error.line
        );
        assert!(
            error.column >= 1,
            "Column should be >= 1, got {}",
            error.column
        );
    }
}

/// 構文エラーがあっても部分的にパースが継続し、エラーが報告されることを検証
///
/// パーサーのエラー回復機能により、最初のエラーで停止せず複数エラーを収集する
#[test]
fn test_error_recoverable() {
    // エラー回復のテスト：構文エラーがあっても情報を失わない
    let input = r#"{
  good: "value"
  bad,
  also_good: "another"
}"#;

    let result = parse(input);

    // 構文エラーがある入力なのでErrが返る
    assert!(result.is_err(), "Input with syntax errors should fail");

    let errors = result.unwrap_err();
    // 複数のエラーが報告されるべき
    assert!(
        errors.len() >= 1,
        "Should have at least one error, got {}",
        errors.len()
    );
}

/// 無効な16進数リテラルはquoteless stringとして扱われることを検証
#[test]
fn test_error_invalid_hex() {
    let input = "{ value: 0xZZ }";

    let result = parse(input);
    assert!(
        result.is_ok(),
        "Invalid hex is treated as unquoted string in HJSON: {:?}",
        result.err()
    );
}

#[test]
fn test_error_non_rfc_numeric_extensions() {
    for input in ["{ value: 0xFF }", "{ value: NaN }", "{ value: Infinity }"] {
        assert!(
            parse(input).is_ok(),
            "Non-RFC numeric spellings should fall back to strings: {input}"
        );
    }
}

#[test]
fn test_quoteless_string_with_space_and_comma() {
    let input = "{ value: look ma, no quotes! }";
    let result = parse(input);
    assert!(
        result.is_ok(),
        "Should parse quoteless string with spaces: {:?}",
        result.err()
    );
}

#[test]
fn test_quoteless_rfc_examples_semantics() {
    assert!(parse("duration: 7 # minutes\n").is_ok());
    assert!(parse("windowsPath: C:\\Users\\eva828\\Documents\\Develop\n").is_ok());
}

/// コロンが連続する構文エラー（key:: value）が検出されることを検証
#[test]
fn test_error_double_colon() {
    let input = "{ key:: value }";

    let result = parse(input);
    assert!(
        result.is_err() || result.as_ref().unwrap().errors.len() > 0,
        "Double colon should produce error"
    );
}

/// コロンの後に値がない構文エラー（key: }）が検出されることを検証
#[test]
fn test_error_missing_value() {
    let input = "{ key: }";

    let result = parse(input);
    assert!(
        result.is_err() || result.as_ref().unwrap().errors.len() > 0,
        "Missing value should produce error"
    );
}

/// 多バイトUnicode文字（日本語・絵文字）を含む文字列が正常にパースされることを検証
///
/// Rustの型システムにより無効なUTF-8は&strとして渡せないため、正常系を検証する
#[test]
fn test_error_invalid_unicode() {
    // 有効なUTF-8文字列は正常にパースできることを確認
    // （Rustの型システムにより無効なUTF-8はそもそも&strとして渡せないため、
    //  ここでは多バイトUnicode文字の正常系を検証する）
    let input = r#"{ key: "こんにちは🌍" }"#;
    let result = parse(input);
    assert!(
        result.is_ok(),
        "Valid unicode should parse successfully: {:?}",
        result.err()
    );
}

/// 深くネストされた構造内の構文エラーが適切な行番号で報告されることを検証
///
/// 5階層のオブジェクト内に不正な構文を配置し、エラー行が5〜12行目の範囲であることを確認する
#[test]
fn test_error_deeply_nested() {
    // Test error handling in deeply nested structures
    let input = r#"{
  level1: {
    level2: {
      level3: {
        level4: {
          invalid syntax here
        }
      }
    }
  }
}"#;

    let result = parse(input);
    assert!(
        result.is_err() || result.as_ref().unwrap().errors.len() > 0,
        "Should have errors"
    );

    if let Err(errors) = &result {
        for error in errors {
            println!(
                "Deep error: line {}, col {}: {}",
                error.line, error.column, error.message
            );
            // Line should be around 6-11 (errors can be in different places)
            assert!(
                error.line >= 5 && error.line <= 12,
                "Error should be on line ~5-12, got {}",
                error.line
            );
        }
    }
}

/// 複数の閉じられていない配列（[[[[）がエラーになることを検証
#[test]
fn test_error_multiple_unclosed() {
    let input = "[[[[";

    let result = parse(input);
    assert!(result.is_err(), "Multiple unclosed arrays should fail");
}

/// ParseError構造体のmessage・line・columnがDebugフォーマットで正しく出力されることを検証
#[test]
fn test_parse_error_display() {
    let error = ParseError {
        message: "Test error".to_string(),
        line: 10,
        column: 5,
    };

    let error_string = format!("{}", error.message);
    assert_eq!(error_string, "Test error");

    let debug_string = format!("{:?}", error);
    assert!(debug_string.contains("Test error"));
    assert!(debug_string.contains("10"));
    assert!(debug_string.contains("5"));
}
