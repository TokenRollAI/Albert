use albert_core::{AppBootstrapSummary, HttpMethod};
use albert_gateway::GatewayConfig;

#[tauri::command]
pub fn bootstrap_summary() -> AppBootstrapSummary {
    AppBootstrapSummary {
        project_name: "Albert".to_string(),
        current_phase: "Phase 3 - Static Mock Runtime".to_string(),
        ui_surfaces: vec![
            "Overview".to_string(),
            "Import".to_string(),
            "Endpoints".to_string(),
            "Providers".to_string(),
            "Mock Server".to_string(),
        ],
        parser_capabilities: albert_parser::planned_capabilities(),
        storage_capabilities: albert_storage::planned_capabilities(),
        provider_capabilities: albert_openai::planned_capabilities(),
        gateway_capabilities: albert_gateway::planned_capabilities(),
    }
}

#[tauri::command]
pub fn default_gateway_config() -> GatewayConfig {
    GatewayConfig::default()
}

#[tauri::command]
pub fn supported_http_methods() -> Vec<&'static str> {
    vec![
        HttpMethod::Get.as_str(),
        HttpMethod::Post.as_str(),
        HttpMethod::Put.as_str(),
        HttpMethod::Patch.as_str(),
        HttpMethod::Delete.as_str(),
        HttpMethod::Options.as_str(),
        HttpMethod::Head.as_str(),
    ]
}
