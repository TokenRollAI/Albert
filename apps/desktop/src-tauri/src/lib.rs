pub mod commands;
pub mod services;

use crate::services::AppServices;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppServices::new())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap::bootstrap_summary,
            commands::bootstrap::default_gateway_config,
            commands::bootstrap::supported_http_methods,
            commands::parser::parse_api_description,
            commands::parser::import_api_description,
            commands::parser::list_imported_collections,
            commands::parser::list_imported_endpoints,
            commands::parser::load_collection_snapshot,
            commands::parser::export_collection_json,
            commands::gateway::start_mock_server,
            commands::gateway::stop_mock_server,
            commands::gateway::mock_server_status,
            commands::gateway::mock_server_requests,
            commands::gateway::update_mock_server,
            commands::openai::generate_mock_example,
            commands::openai::preview_generation_prompt,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Albert desktop app");
}
