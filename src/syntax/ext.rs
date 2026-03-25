// use super::{RedElement, RedNode, RedToken, SyntaxKind};
// use crate::high_level::error::HjsonEditError;
// use crate::high_level::path::{ArrayPathSegment, HjsonPath, ObjectPathSegment, PathSegment};
// use crate::high_level::value::HjsonValue;
// use crate::syntax::HjsonLanguage;
// use crate::GreenElement;
// use rowan::{GreenNode, GreenNodeBuilder, GreenToken, Language, NodeOrToken};

// /// GreenNodeおよびNodeOrTokenのユーティリティ拡張
// pub trait GreenNodeExt {
//     /// Pairノードが指定したキー名を持っているか判定する（ゼロアロケーション）
//     fn has_pair(&self, expected_key: &str) -> bool;
// }

// impl GreenNodeExt for GreenNode {
//     fn has_pair(&self, key: &str) -> bool {
//         self.children()
//             .filter(|c| c.kind() == SyntaxKind::Key.into())
//             .filter_map(|c| c.into_node())
//             .filter(|c| {
//                 matches!(
//                     HjsonLanguage::kind_from_raw(c.kind()),
//                     SyntaxKind::String | SyntaxKind::UnquotedString
//                 )
//             })
//             .any(|c| c.to_string().trim_matches('"') == key)
//     }
// }

// // pub trait NodeOrTokenExt {
// //     /// 参照から所有権を持つ GreenElement へ変換する
// //     fn to_green_element(&self) -> GreenElement;
// // }

// // impl NodeOrTokenExt for NodeOrToken<RedNode, RedToken> {
// //     fn to_green_element(&self) -> GreenElement {
// //         match self {
// //             NodeOrToken::Node(node_data) => {
// //                 NodeOrToken::Node(GreenNode::from(std::borrow::Cow::Borrowed(*node_data)))
// //             }
// //             NodeOrToken::Token(token_data) => {
// //                 NodeOrToken::Token(GreenToken::new(token_data.kind(), token_data.text()))
// //             }
// //         }
// //     }
// // }

// // pub trait GreenNodeBuilderExt {
// //     fn push_element(&mut self, element: &GreenElement) -> Result<(), HjsonEditError>;
// //     fn push_raw(
// //         &mut self,
// //         child: &NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>,
// //     ) -> Result<(), HjsonEditError>;
// //     fn push_node(&mut self, node: GreenNode) -> Result<(), HjsonEditError>;
// // }

// // impl GreenNodeBuilderExt for GreenNodeBuilder<'static> {
// //     fn push_element(&mut self, element: &GreenElement) -> Result<(), HjsonEditError> {
// //         match element {
// //             NodeOrToken::Node(node) => {
// //                 self.start_node(node.kind());
// //                 for child in node.children() {
// //                     self.push_element(&child.to_green_element())?;
// //                 }
// //                 self.finish_node();
// //             }
// //             NodeOrToken::Token(token) => {
// //                 self.token(token.kind(), token.text());
// //             }
// //         }
// //         Ok(())
// //     }

// //     fn push_raw(
// //         &mut self,
// //         child: &NodeOrToken<&rowan::GreenNodeData, &rowan::GreenTokenData>,
// //     ) -> Result<(), HjsonEditError> {
// //         self.push_element(&child.to_green_element())
// //     }

// //     fn push_node(&mut self, node: GreenNode) -> Result<(), HjsonEditError> {
// //         self.push_element(&NodeOrToken::Node(node))
// //     }
// // }

// /// RedNodeの基本ユーティリティ拡張
// pub trait RedNodeExt {
//     fn find_pair_by_key(&self, key: &str) -> Option<RedNode>;
//     fn get_array_element(&self, op: &ArrayPathSegment) -> Result<Option<RedNode>, HjsonEditError>;
//     fn get_value_at_path(&self, path: &HjsonPath) -> Result<HjsonValue, HjsonEditError>;
//     fn navigate_path(
//         &self,
//         path: &HjsonPath,
//         idx: usize,
//         create_missing: bool,
//     ) -> Result<Option<RedNode>, HjsonEditError>;
//     fn to_hjson_value(&self) -> Result<HjsonValue, HjsonEditError>;
// }

// impl RedNodeExt for RedNode {
//     fn find_pair_by_key(&self, key: &str) -> Option<RedNode> {
//         if self.kind() != SyntaxKind::Object {
//             return None;
//         }

//         for pair in self.children() {
//             if pair.kind() != SyntaxKind::Pair {
//                 continue;
//             }

//             let Some(key_node) = pair.children().find(|c| c.kind() == SyntaxKind::Key) else {
//                 continue;
//             };
//             let Some(token) = key_node.children_with_tokens().find_map(|e| e.into_token()) else {
//                 continue;
//             };

//             if !matches!(
//                 token.kind(),
//                 SyntaxKind::String | SyntaxKind::UnquotedString | SyntaxKind::BlockString
//             ) {
//                 continue;
//             }

//             if token.text().trim_matches('"') == key {
//                 return Some(pair);
//             }
//         }

//         None
//     }

//     fn get_array_element(&self, op: &ArrayPathSegment) -> Result<Option<RedNode>, HjsonEditError> {
//         if self.kind() != SyntaxKind::Array {
//             return Ok(None);
//         }

//         // Vecにcollectせず、イテレータのまま処理する
//         let values_iter = || {
//             self.children()
//                 .filter(|child| child.kind() == SyntaxKind::Value)
//         };

//         match op {
//             ArrayPathSegment::Index(idx) => values_iter()
//                 .nth(*idx)
//                 .map(Some)
//                 .ok_or_else(|| HjsonEditError::ChildNotFound(vec![])),
//             ArrayPathSegment::InsertAt(idx) => {
//                 let count = values_iter().count();
//                 if *idx > count {
//                     return Err(HjsonEditError::ChildNotFound(vec![]));
//                 }
//                 if *idx == count {
//                     return Ok(None);
//                 }
//                 Ok(values_iter().nth(*idx))
//             }
//             ArrayPathSegment::Push => Ok(None),
//         }
//     }

//     fn get_value_at_path(&self, path: &HjsonPath) -> Result<HjsonValue, HjsonEditError> {
//         if path.segments.is_empty() {
//             return Err(HjsonEditError::ChildNotFound(vec![]));
//         }

//         // navigate_segments から navigate_path へ変更し、path をそのまま渡す
//         let Some(target_node) = self.navigate_path(path, 0, false)? else {
//             return Err(HjsonEditError::ChildNotFound(path.segments.clone()));
//         };

//         target_node.to_hjson_value()
//     }

//     fn navigate_path(
//         &self,
//         path: &HjsonPath,
//         idx: usize,
//         create_missing: bool,
//     ) -> Result<Option<RedNode>, HjsonEditError> {
//         if idx >= path.segments.len() {
//             return Ok(Some(self.clone()));
//         }

//         let kind = self.kind();

//         // Root または Value の場合は、その中身（Object, Array, Pair等）に潜る
//         if kind == SyntaxKind::Root || kind == SyntaxKind::Value {
//             if let Some(n) = self.children().find(|c| {
//                 matches!(
//                     c.kind(),
//                     SyntaxKind::Object | SyntaxKind::Array | SyntaxKind::Pair
//                 )
//             }) {
//                 return n.navigate_path(path, idx, create_missing);
//             }
//             if let Some(value_node) = self.children().find(|c| c.kind() == SyntaxKind::Value) {
//                 return value_node.navigate_path(path, idx, create_missing);
//             }
//             return Err(HjsonEditError::ChildNotFound(
//                 path.segments[..idx + 1].to_vec(),
//             ));
//         }

//         match &path.segments[idx] {
//             PathSegment::Object(ObjectPathSegment::Key(key)) => {
//                 if kind != SyntaxKind::Object {
//                     return Err(HjsonEditError::ChildNotFound(
//                         path.segments[..idx + 1].to_vec(),
//                     ));
//                 }

//                 let Some(pair) = self.find_pair_by_key(key) else {
//                     return if create_missing {
//                         Ok(None)
//                     } else {
//                         Err(HjsonEditError::ChildNotFound(
//                             path.segments[..idx + 1].to_vec(),
//                         ))
//                     };
//                 };

//                 let Some(value_node) = pair.children().find(|c| c.kind() == SyntaxKind::Value)
//                 else {
//                     return if create_missing {
//                         Ok(None)
//                     } else {
//                         Err(HjsonEditError::ChildNotFound(
//                             path.segments[..idx + 1].to_vec(),
//                         ))
//                     };
//                 };

//                 value_node.navigate_path(path, idx + 1, create_missing)
//             }
//             PathSegment::Array(op) => {
//                 if kind != SyntaxKind::Array {
//                     return Err(HjsonEditError::ChildNotFound(
//                         path.segments[..idx + 1].to_vec(),
//                     ));
//                 }

//                 let Some(element) = self.get_array_element(op)? else {
//                     if create_missing
//                         || matches!(op, ArrayPathSegment::Push | ArrayPathSegment::InsertAt(_))
//                     {
//                         return Ok(None);
//                     }
//                     return Err(HjsonEditError::ChildNotFound(
//                         path.segments[..idx + 1].to_vec(),
//                     ));
//                 };

//                 element.navigate_path(path, idx + 1, create_missing)
//             }
//         }
//     }

//     fn to_hjson_value(&self) -> Result<HjsonValue, HjsonEditError> {
//         match self.kind() {
//             SyntaxKind::Value => {
//                 if let Some(child) = self.children().next() {
//                     return child.to_hjson_value();
//                 }
//                 if let Some(token) = self.children_with_tokens().find_map(|e| e.into_token()) {
//                     return token.to_hjson_value();
//                 }
//                 Ok(HjsonValue::Null)
//             }
//             SyntaxKind::Object => {
//                 let mut pairs = Vec::new();
//                 for pair in self.children() {
//                     if pair.kind() != SyntaxKind::Pair {
//                         continue;
//                     }

//                     let Some(key_node) = pair.children().find(|c| c.kind() == SyntaxKind::Key)
//                     else {
//                         continue;
//                     };

//                     let key = key_node
//                         .children_with_tokens()
//                         .find_map(|e| {
//                             let t = e.into_token()?;
//                             if matches!(
//                                 t.kind(),
//                                 SyntaxKind::String
//                                     | SyntaxKind::UnquotedString
//                                     | SyntaxKind::BlockString
//                             ) {
//                                 Some(t.text().trim_matches('"').to_string())
//                             } else {
//                                 None
//                             }
//                         })
//                         .unwrap_or_default();

//                     let Some(value_node) = pair.children().find(|c| c.kind() == SyntaxKind::Value)
//                     else {
//                         continue;
//                     };

//                     pairs.push((key, value_node.to_hjson_value()?));
//                 }
//                 Ok(HjsonValue::Object(pairs))
//             }
//             SyntaxKind::Array => {
//                 let mut items = Vec::new();
//                 for value_node in self.children() {
//                     if value_node.kind() != SyntaxKind::Value {
//                         continue;
//                     }
//                     items.push(value_node.to_hjson_value()?);
//                 }
//                 Ok(HjsonValue::Array(items))
//             }
//             _ => {
//                 if let Some(token) = self.children_with_tokens().find_map(|e| e.into_token()) {
//                     return token.to_hjson_value();
//                 }
//                 Ok(HjsonValue::Null)
//             }
//         }
//     }
// }

// pub trait RedTokenExt {
//     fn to_hjson_value(&self) -> Result<HjsonValue, HjsonEditError>;
// }

// impl RedTokenExt for RedToken {
//     fn to_hjson_value(&self) -> Result<HjsonValue, HjsonEditError> {
//         let text = self.text();
//         match self.kind() {
//             SyntaxKind::String | SyntaxKind::BlockString | SyntaxKind::UnquotedString => {
//                 Ok(HjsonValue::String(text.trim_matches('"').to_string()))
//             }
//             SyntaxKind::Integer => text
//                 .parse::<i64>()
//                 .map(HjsonValue::Integer)
//                 .map_err(|_| HjsonEditError::ChildNotFound(vec![])),
//             SyntaxKind::Float => text
//                 .parse::<f64>()
//                 .map(HjsonValue::Float)
//                 .map_err(|_| HjsonEditError::ChildNotFound(vec![])),
//             SyntaxKind::True => Ok(HjsonValue::Bool(true)),
//             SyntaxKind::False => Ok(HjsonValue::Bool(false)),
//             SyntaxKind::Null => Ok(HjsonValue::Null),
//             _ => Ok(HjsonValue::String(text.to_string())),
//         }
//     }
// }
