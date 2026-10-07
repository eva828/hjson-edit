use hjson_edit::ast::{AstNode, Document};
use hjson_edit::high_level::{
    ops::comment::HjsonDocumentCommentEdit, path::HjsonCommentPath, HjsonDocument,
};
use rowan::NodeOrToken;
use std::convert::TryFrom;

fn print_tree(node: &hjson_edit::RedNode, indent: usize) {
    let prefix = "  ".repeat(indent);
    println!("{}NODE {:?} (index={})", prefix, node.kind(), node.index());
    for child in node.children_with_tokens() {
        match child {
            NodeOrToken::Node(child_node) => {
                print_tree(&child_node, indent + 1);
            }
            NodeOrToken::Token(token) => {
                let text: String = token.text().chars().take(50).collect();
                let p = "  ".repeat(indent + 1);
                println!("{}TOKEN {:?} text={:?}", p, token.kind(), text);
            }
        }
    }
}

fn inspect(input: &str, label: &str) {
    println!("\n=== {} ===", label);
    let doc = HjsonDocument::parse(input).unwrap();
    let root = doc.syntax_root().clone();

    print_tree(&root, 0);

    // Use the Document API to navigate
    let ast = Document::cast(root.clone()).unwrap();
    println!("\nDocument leading_trivia: {:?}", ast.leading_trivia);
    println!("Document trailing_trivia: {:?}", ast.trailing_trivia);

    // Get the object
    let outer_obj = ast.object().unwrap();
    println!(
        "\nOuter object members: {:?}",
        outer_obj
            .members()
            .iter()
            .map(|m| m.member_key())
            .collect::<Vec<_>>()
    );
    let a_member = outer_obj.member_by_key("a").unwrap();
    println!("Member 'a' trailing_trivia: {:?}", a_member.trailing_trivia);
    println!("Member 'a' leading_trivia: {:?}", a_member.leading_trivia);

    // Navigate deeper
    let a_val = a_member.value().unwrap();
    let middle_obj = match a_val.content().unwrap() {
        hjson_edit::ast::ValueContent::Object(obj) => obj,
        _ => panic!("expected object"),
    };
    let b_member = middle_obj.member_by_key("b").unwrap();
    let b_val = b_member.value().unwrap();
    let inner_obj = match b_val.content().unwrap() {
        hjson_edit::ast::ValueContent::Object(obj) => obj,
        _ => panic!("expected object"),
    };
    let c_member = inner_obj.member_by_key("c").unwrap();
    println!(
        "\nMember 'c' trailing_trivia: {:?}",
        c_member.trailing_trivia
    );
    println!("Member 'c' leading_trivia: {:?}", c_member.leading_trivia);

    // Value of c
    let c_val = c_member.value().unwrap();
    println!(
        "Value of c raw text: {:?}",
        c_val
            .content()
            .ok()
            .and_then(|c| c.raw_text().map(|s| s.to_string()))
    );

    // Get comment via API
    let comment = doc
        .get_comment(HjsonCommentPath::try_from("a.b.c:inline").unwrap())
        .unwrap();
    println!("get_comment('a.b.c:inline') = {:?}", comment);
}

fn main() {
    inspect(
        "{\n  a: {\n    b: {\n      c: value // note\n    }\n  }\n}\n",
        "Unquoted string 'value // note'",
    );

    inspect(
        "{\n  a: {\n    b: {\n      c: \"value\" // note\n    }\n  }\n}\n",
        "Quoted string '\"value\" // note'",
    );

    inspect(
        "{\n  a: {\n    b: {\n      c: 42 // note\n    }\n  }\n}\n",
        "Number '42 // note'",
    );

    inspect(
        "{\n  a: {\n    b: {\n      c: true // note\n    }\n  }\n}\n",
        "Boolean 'true // note'",
    );
}
