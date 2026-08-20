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
        if rect.left().is_finite()
            && rect.top().is_finite()
            && rect.right().is_finite()
            && rect.bottom().is_finite()
            && rect.width() > 0.0
            && rect.height() > 0.0
        {
            self.dirty_rects.push(rect);
        }
    }

    /// Marks multiple rectangular regions as damaged/dirty in batch.
    pub fn add_damage_rects(&mut self, rects: &[Rect]) {
        for &rect in rects {
            self.add_damage(rect);
        }
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

        if left < right && top < bottom {
            Rect::from_ltrb(left, top, right, bottom)
        } else {
            None
        }
    }

    /// Checks if any accumulated dirty rectangle intersects `target`.
    #[must_use]
    pub fn intersects_any(&self, target: Rect) -> bool {
        self.dirty_rects.iter().any(|r| {
            r.left() < target.right()
                && r.right() > target.left()
                && r.top() < target.bottom()
                && r.bottom() > target.top()
        })
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
