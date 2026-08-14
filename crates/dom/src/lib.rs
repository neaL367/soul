//! Arena-based DOM tree, `NodeId` handle system, element attributes, and mutation APIs.

pub mod document;
pub mod node;

pub use document::Document;
pub use node::{DocumentTypeData, ElementData, InvalidationFlags, Node, NodeData, NodeId};
