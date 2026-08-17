//! Navigation state machine, URL resolution, and session history management.

mod controller;
mod history;
mod state;

pub use controller::NavigationController;
pub use history::{HistoryEntry, NavigationHistory};
pub use state::{NavigationError, NavigationId, NavigationState};
