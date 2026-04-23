use albert_core::AppBootstrapSummary;

#[tauri::command]
fn bootstrap_summary() -> AppBootstrapSummary {
    AppBootstrapSummary {
        project_name: "Albert".to_string(),
        current_phase: "Phase 1 - Foundation".to_string(),
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![bootstrap_summary])
        .run(tauri::generate_context!())
        .expect("failed to run Albert desktop app");
}
