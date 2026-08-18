//! Windows UI Automation accessibility bridge mapping engine semantic trees to Windows accessibility representations.

/// Windows UI Automation Control Type Identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum UiaControlType {
    /// Button control (`UIA_ButtonControlTypeId = 50000`).
    Button = 50000,
    /// Hyperlink control (`UIA_HyperlinkControlTypeId = 50005`).
    Hyperlink = 50005,
    /// Image control (`UIA_ImageControlTypeId = 50006`).
    Image = 50006,
    /// Text / paragraph control (`UIA_TextControlTypeId = 50020`).
    Text = 50020,
    /// Group / generic container control (`UIA_GroupControlTypeId = 50026`).
    Group = 50026,
    /// Document control (`UIA_DocumentControlTypeId = 50030`).
    Document = 50030,
    /// Heading control (`UIA_HeadingControlTypeId = 50036`).
    Heading = 50036,
}

/// Lightweight UI Automation accessibility element representing an accessible node in the visual tree.
#[derive(Debug, Clone, PartialEq)]
pub struct UiaElement {
    /// Node identifier.
    pub id: u64,
    /// UI Automation control type.
    pub control_type: UiaControlType,
    /// Accessible name / label.
    pub name: String,
    /// Bounding rectangle in layout coordinates `(x, y, width, height)`.
    pub bounds: (f32, f32, f32, f32),
    /// Whether the element is directly interactive (e.g. Button or Hyperlink).
    pub is_interactive: bool,
    /// Child accessible elements.
    pub children: Vec<Self>,
}

impl UiaElement {
    /// Creates a new `UiaElement`.
    #[must_use]
    pub fn new(
        id: u64,
        control_type: UiaControlType,
        name: impl Into<String>,
        bounds: (f32, f32, f32, f32),
    ) -> Self {
        let is_interactive = matches!(
            control_type,
            UiaControlType::Button | UiaControlType::Hyperlink
        );
        Self {
            id,
            control_type,
            name: name.into(),
            bounds,
            is_interactive,
            children: Vec::new(),
        }
    }

    /// Recursively performs hit-testing against this element and its descendants.
    #[must_use]
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&Self> {
        let (bx, by, bw, bh) = self.bounds;
        if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
            for child in &self.children {
                if let Some(hit) = child.hit_test(x, y) {
                    return Some(hit);
                }
            }
            return Some(self);
        }
        None
    }

    /// Recursively searches for an element with the given ID.
    #[must_use]
    pub fn find_by_id(&self, target_id: u64) -> Option<&Self> {
        if self.id == target_id {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_by_id(target_id) {
                return Some(found);
            }
        }
        None
    }
}

/// Accessibility tree bridge for Windows accessibility dispatch.
#[derive(Debug, Clone, Default)]
pub struct UiaBridge {
    root: Option<UiaElement>,
}

impl UiaBridge {
    /// Creates a new empty `UiaBridge`.
    #[must_use]
    pub const fn new() -> Self {
        Self { root: None }
    }

    /// Sets the root element of the accessibility tree.
    pub fn set_root(&mut self, root: UiaElement) {
        self.root = Some(root);
    }

    /// Returns a reference to the root accessibility element.
    #[must_use]
    pub const fn root(&self) -> Option<&UiaElement> {
        self.root.as_ref()
    }

    /// Hit-tests the accessibility tree at coordinates `(x, y)`.
    #[must_use]
    pub fn hit_test(&self, x: f32, y: f32) -> Option<&UiaElement> {
        self.root.as_ref().and_then(|r| r.hit_test(x, y))
    }

    /// Finds an accessible element by its node ID.
    #[must_use]
    pub fn find_element(&self, id: u64) -> Option<&UiaElement> {
        self.root.as_ref().and_then(|r| r.find_by_id(id))
    }
}
