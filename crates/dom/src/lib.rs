//! Arena-based DOM tree, `NodeId` handle system, element attributes, and mutation APIs.
//!
//! Also implements the WHATWG DOM `EventTarget` interface (\u00a72.7) via [`events`].

pub mod document;
pub mod events;
pub mod node;
pub mod traversal;

pub use document::{Document, MAX_DOM_DEPTH, MAX_NODES};
pub use events::{
    AddEventListenerOptions, DispatchResult, Event, EventPhase, add_event_listener, dispatch_event,
    remove_event_listener,
};
pub use node::{
    DocumentTypeData, ElementData, EventListener, InvalidationFlags, Node, NodeData, NodeId,
    ShadowRootMode,
};
