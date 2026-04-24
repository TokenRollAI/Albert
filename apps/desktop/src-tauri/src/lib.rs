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
            commands::parser::import_bundle,
            commands::parser::list_imported_collections,
            commands::parser::list_imported_endpoints,
            commands::parser::load_collection_snapshot,
            commands::parser::export_collection_json,
            commands::parser::export_all_collections_json,
            commands::parser::delete_collection,
            commands::parser::rename_collection,
            commands::parser::save_mock_example,
            commands::parser::synthesize_request_body,
            commands::fetch::fetch_remote_source,
            commands::gateway::start_mock_server,
            commands::gateway::stop_mock_server,
            commands::gateway::mock_server_status,
            commands::gateway::mock_server_requests,
            commands::gateway::mock_server_metrics,
            commands::gateway::mock_server_clear_log,
            commands::gateway::export_gateway_config,
            commands::gateway::import_gateway_config,
            commands::gateway::load_gateway_preferences,
            commands::gateway::save_gateway_preferences,
            commands::gateway::update_mock_server,
            commands::gateway::list_gateway_scenarios,
            commands::gateway::save_gateway_scenario,
            commands::gateway::load_gateway_scenario,
            commands::gateway::delete_gateway_scenario,
            commands::gateway::rename_gateway_scenario,
            commands::openai::generate_mock_example,
            commands::openai::preview_generation_prompt,
            commands::openai::test_provider_connection,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Albert desktop app");
}
