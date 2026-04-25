use crate::{HjsonDocument, RedNode};

pub mod comment;
pub mod object;

use rowan::GreenNode;

pub(crate) fn finish_document(doc: HjsonDocument, root: GreenNode) -> HjsonDocument {
    HjsonDocument {
        root: RedNode::new_root(root),
        string_style: doc.current_string_style(),
        comment_style: doc.current_comment_style(),
        append_mode: doc.current_append_mode(),
        newline: doc.current_newline().to_string(),
    }
}
