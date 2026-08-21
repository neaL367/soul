//! CSS parser, selector matching, specificity, cascade resolution, and computed styles.

pub mod cascade;
pub mod media;
pub mod parser;
pub mod properties;
pub mod rule;
pub mod selector_impl;
pub mod ua;

pub use cascade::{CascadeResolver, apply_declaration, resolve_var_references};
pub use media::{ColorScheme, MediaCondition};
pub use parser::parse_stylesheet;
pub use properties::{
    AlignItems, AlignSelf, BoxShadow, BoxSizing, Color, ColorStop, ComputedStyle, Display,
    FlexDirection, FlexWrap, FontStyle, FontWeight, Gradient, GridTrack, JustifyContent, Length,
    Position, TextAlign, TextDecoration, TimingFunction, Transform2D, TransformOp, Transition,
};
pub use rule::{Declaration, Origin, Rule, Selector, Specificity, StyleSheet};
pub use selector_impl::{DomElement, SoulParser, SoulSelectorImpl};
pub use ua::user_agent_stylesheet;
