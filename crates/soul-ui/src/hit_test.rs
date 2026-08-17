//! Backend-neutral page hit-test regions produced by layout.

/// Page target activated by a pointer click.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HitTestTarget {
    /// Hyperlink destination.
    Link(String),
}

/// Rectangular page region associated with an interactive target.
#[derive(Debug, Clone, PartialEq)]
pub struct HitTestRegion {
    /// Region origin x in page CSS pixels.
    pub x: f32,
    /// Region origin y in page CSS pixels.
    pub y: f32,
    /// Region width in page CSS pixels.
    pub width: f32,
    /// Region height in page CSS pixels.
    pub height: f32,
    /// Target activated by the region.
    pub target: HitTestTarget,
}

impl HitTestRegion {
    /// Returns true when page coordinates fall inside the region.
    #[must_use]
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.x + self.width && y <= self.y + self.height
    }
}

/// Ordered page hit-test regions. Later regions win, matching paint order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HitTestMap {
    /// Interactive regions in layout/paint order.
    pub regions: Vec<HitTestRegion>,
}

impl HitTestMap {
    /// Finds the topmost target at page coordinates.
    #[must_use]
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&HitTestTarget> {
        self.regions
            .iter()
            .rev()
            .find(|region| region.contains(x, y))
            .map(|region| &region.target)
    }
}
