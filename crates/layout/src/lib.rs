//! Box tree construction, block and inline layout formatting, and geometry types.

pub mod a11y;
pub mod block;
pub mod box_tree;
pub mod geometry;
pub mod inline;

pub use a11y::{A11yNode, A11yRole};
pub use block::layout_block;
pub use box_tree::{BoxType, LayoutBox, build_box_tree};
pub use geometry::{Dimensions, EdgeSizes, Rect};
pub use inline::{InlineFragment, LineBox, layout_inline_context};
