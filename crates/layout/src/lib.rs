//! Box generation, normal flow block layout, and CSS box model geometry.

pub mod block;
pub mod box_tree;
pub mod geometry;
pub mod inline;

pub use block::layout_block;
pub use box_tree::{BoxType, LayoutBox, build_box_tree};
pub use geometry::{Dimensions, EdgeSizes, Rect};
pub use inline::{InlineFragment, LineBox, layout_inline_context};
