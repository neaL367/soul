//! Arena-based DOM tree, `NodeId` handle system, element attributes, and mutation APIs.

pub mod document;
pub mod node;
pub mod traversal;

pub use document::{Document, MAX_DOM_DEPTH, MAX_NODES};
pub use node::{DocumentTypeData, ElementData, InvalidationFlags, Node, NodeData, NodeId};
