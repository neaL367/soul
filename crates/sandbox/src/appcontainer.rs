//! Windows `AppContainer` isolation profile and capability management.

use crate::error::SandboxError;

/// Standard capabilities grantable to an `AppContainer` sandbox profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppContainerCapability {
    /// Allows outbound TCP/IP socket connections (`internetClient`).
    InternetClient,
    /// Allows inbound and outbound network server connections (`internetClientServer`).
    InternetClientServer,
    /// Allows access to private network resources (`privateNetworkClientServer`).
    PrivateNetwork,
}

/// `AppContainer` security profile managing isolated SID derivation and capabilities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppContainerProfile {
    /// Unique package family name for the container (e.g. `"SoulBrowser.Renderer"`).
    pub package_name: String,
    /// Human-readable display name.
    pub display_name: String,
    /// List of enabled capabilities.
    pub capabilities: Vec<AppContainerCapability>,
}

impl AppContainerProfile {
    /// Creates a new `AppContainer` profile with the specified package name and display name.
    #[must_use]
    pub fn new(package_name: &str, display_name: &str) -> Self {
        Self {
            package_name: package_name.to_string(),
            display_name: display_name.to_string(),
            capabilities: Vec::new(),
        }
    }

    /// Creates a standard `AppContainer` profile tailored for untrusted renderer processes.
    #[must_use]
    pub fn for_renderer(instance_id: u64) -> Self {
        Self {
            package_name: format!("SoulBrowser.Renderer.{instance_id}"),
            display_name: format!("Soul Renderer Sandbox {instance_id}"),
            capabilities: Vec::new(), // Renderer has 0 network capabilities
        }
    }

    /// Creates a standard `AppContainer` profile tailored for network processes.
    #[must_use]
    pub fn for_network() -> Self {
        Self {
            package_name: "SoulBrowser.Network".to_string(),
            display_name: "Soul Network Sandbox".to_string(),
            capabilities: vec![AppContainerCapability::InternetClient],
        }
    }

    /// Adds a capability to this `AppContainer` profile.
    #[must_use]
    pub fn with_capability(mut self, capability: AppContainerCapability) -> Self {
        if !self.capabilities.contains(&capability) {
            self.capabilities.push(capability);
        }
        self
    }

    /// Computes the deterministic `AppContainer` SID string identifier.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn derive_sid_string(&self) -> String {
        // Deterministic AppContainer pseudo-SID format: S-1-15-2-...
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        self.package_name.hash(&mut hasher);
        let h = hasher.finish();

        let p1 = (h & 0xFFFF_FFFF) as u32;
        let p2 = ((h >> 32) & 0xFFFF_FFFF) as u32;
        format!("S-1-15-2-{p1}-{p2}")
    }

    /// Validates that the profile meets minimum integrity requirements.
    ///
    /// # Errors
    ///
    /// Returns `SandboxError::InvalidLimit` if the package name is empty.
    pub fn validate(&self) -> Result<(), SandboxError> {
        if self.package_name.trim().is_empty() {
            return Err(SandboxError::InvalidLimit(
                "AppContainer package name cannot be empty".to_string(),
            ));
        }
        Ok(())
    }
}
