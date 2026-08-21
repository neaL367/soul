//! Box tree construction, block and inline layout formatting, and geometry types.

pub mod a11y;
pub mod block;
pub mod box_tree;
pub mod calc;
pub mod flex;
pub mod geometry;
pub mod grid;
pub mod inline;

pub use a11y::{A11yNode, A11yRole};
pub use block::{MAX_LAYOUT_DEPTH, layout_block};
pub use box_tree::{BoxType, LayoutBox, build_box_tree, build_box_tree_with_intrinsics};
pub use calc::{LengthContext, evaluate_calc, resolve_length};
pub use flex::{FlexContainerResult, FlexResult, layout_flex};
pub use geometry::{Dimensions, EdgeSizes, IntrinsicSize, Rect};
pub use grid::{GridContainerResult, GridResult, layout_grid};
pub use inline::{InlineFragment, LineBox, layout_inline_context};
