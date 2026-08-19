mod agent;
mod browser;
mod commands;
mod prompt;
mod providers;
mod storage;
mod tools;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(agent::state::RunState::default())
        .manage(browser::state::BrowserState::default())
        .setup(|app| {
            commands::chat_repository(&app.handle())?.initialize()?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_prompt_settings,
            commands::save_prompt_settings,
            commands::reset_prompt_settings,
            commands::list_prompt_resources,
            commands::list_providers,
            commands::save_provider,
            commands::delete_provider,
            commands::test_provider,
            commands::list_conversations,
            commands::list_messages,
            commands::list_tool_runs,
            commands::list_sources,
            commands::rename_conversation,
            commands::delete_conversation,
            commands::send_message,
            commands::cancel_run,
            browser::commands::list_browser_tabs,
            browser::commands::open_browser_tab,
            browser::commands::close_browser_tab,
            browser::commands::select_browser_tab,
            browser::commands::set_browser_visibility,
            browser::commands::set_browser_bounds,
            browser::commands::navigate_browser_tab,
            browser::commands::reload_browser_tab,
            browser::commands::browser_history,
            browser::commands::extract_browser_page,
            browser::commands::scroll_browser_page,
            browser::commands::click_browser_target,
            browser::commands::browser_profile_status
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Paimon Desktop");
}
