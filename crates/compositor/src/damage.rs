//! Damage tracking and dirty rectangle union calculations for efficient partial redraws.

use tiny_skia::Rect;

/// Manages dirty rectangle regions to minimize repainting and texture upload overhead.
#[derive(Debug, Default, Clone)]
pub struct DamageTracker {
    dirty_rects: Vec<Rect>,
}

impl DamageTracker {
    /// Creates a new empty `DamageTracker`.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dirty_rects: Vec::new(),
        }
    }

    /// Marks a rectangular region as damaged/dirty.
    pub fn add_damage(&mut self, rect: Rect) {
        self.dirty_rects.push(rect);
    }

    /// Computes the minimal bounding box enclosing all accumulated damage rectangles.
    #[must_use]
    pub fn union_bounds(&self) -> Option<Rect> {
        if self.dirty_rects.is_empty() {
            return None;
        }

        let mut left = f32::MAX;
        let mut top = f32::MAX;
        let mut right = f32::MIN;
        let mut bottom = f32::MIN;

        for r in &self.dirty_rects {
            left = left.min(r.left());
            top = top.min(r.top());
            right = right.max(r.right());
            bottom = bottom.max(r.bottom());
        }

        Rect::from_ltrb(left, top, right, bottom)
    }

    /// Clears all accumulated damage.
    pub fn clear(&mut self) {
        self.dirty_rects.clear();
    }

    /// Returns `true` if there is no damaged region.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.dirty_rects.is_empty()
    }

    /// Returns all individual dirty rectangles.
    #[must_use]
    pub fn rects(&self) -> &[Rect] {
        &self.dirty_rects
    }
}
