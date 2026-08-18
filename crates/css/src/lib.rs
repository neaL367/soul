//! CSS parser, selector matching, specificity, cascade resolution, and computed styles.

pub mod cascade;
pub mod parser;
pub mod properties;
pub mod rule;
pub mod ua;

pub use cascade::CascadeResolver;
pub use parser::parse_stylesheet;
pub use properties::{
    AlignItems, AlignSelf, BoxSizing, Color, ComputedStyle, Display, FlexDirection, FlexWrap,
    FontStyle, FontWeight, JustifyContent, Length, Position, TextAlign, TextDecoration,
};
pub use rule::{
    Combinator, Declaration, Origin, Rule, Selector, SimpleSelector, Specificity, StyleSheet,
};
pub use ua::user_agent_stylesheet;
