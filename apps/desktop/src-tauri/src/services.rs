//! Long-lived services shared across Tauri commands.

use std::sync::Arc;

use albert_gateway::MockGateway;

pub struct AppServices {
    pub gateway: Arc<MockGateway>,
}

impl AppServices {
    pub fn new() -> Self {
        Self {
            gateway: Arc::new(MockGateway::new()),
        }
    }
}

impl Default for AppServices {
    fn default() -> Self {
        Self::new()
    }
}

pub fn default_database_url() -> String {
    "albert.db".to_string()
}
