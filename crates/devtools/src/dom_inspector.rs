//! DOM tree serialization and computed style inspector for Developer Tools.

use dom::{Document, NodeData, NodeId};
use serde_json::json;

/// Inspector utility traversing the DOM arena into serialized CDP nodes.
pub struct DomInspector;

impl DomInspector {
    /// Serializes a subtree starting at `root_id` into a JSON representation for `DevTools`.
    #[must_use]
    pub fn serialize_subtree(doc: &Document, root_id: NodeId) -> serde_json::Value {
        let Some(node) = doc.get_node(root_id) else {
            return json!({ "error": "node not found" });
        };

        let (node_name, node_type, attributes) = match &node.data {
            NodeData::Element(elem) => {
                let attrs: Vec<String> = elem
                    .attributes
                    .iter()
                    .flat_map(|(k, v)| vec![k.clone(), v.clone()])
                    .collect();
                (elem.tag_name.clone(), "ELEMENT_NODE", attrs)
            }
            NodeData::Text(txt) => ("#text".to_string(), "TEXT_NODE", vec![txt.clone()]),
            NodeData::Comment(cmt) => ("#comment".to_string(), "COMMENT_NODE", vec![cmt.clone()]),
            NodeData::Document => ("#document".to_string(), "DOCUMENT_NODE", Vec::new()),
            NodeData::DocumentType(dt) => (
                format!("<!DOCTYPE {}>", dt.name),
                "DOCUMENT_TYPE_NODE",
                Vec::new(),
            ),
        };

        let child_ids = doc.children(root_id);
        let children_json: Vec<serde_json::Value> = child_ids
            .iter()
            .map(|cid| Self::serialize_subtree(doc, *cid))
            .collect();

        json!({
            "nodeId": root_id.0,
            "nodeName": node_name,
            "nodeType": node_type,
            "attributes": attributes,
            "childNodeCount": children_json.len(),
            "children": children_json,
        })
    }
}
