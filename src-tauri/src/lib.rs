mod binman;
mod brand;
mod cloudflare;
mod commands;
mod diagnostics;
mod engine;
mod forwarder;
mod frp_deploy;
mod frp_import;
mod models;
mod providers;
mod secrets_store;
mod servers_store;
mod ssh;
mod store;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch (another copy of the app
        // from a different path, double-open, etc.) exits immediately and
        // brings the existing instance's window to the front — without this,
        // two tray icons and two processes can coexist.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tray::show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            #[cfg(desktop)]
            tray::setup(app.handle())?;
            commands::init_runtime(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tunnels,
            commands::create_tunnel,
            commands::update_tunnel,
            commands::delete_tunnel,
            commands::start_tunnel,
            commands::stop_tunnel,
            commands::get_state,
            commands::list_states,
            commands::read_binary_status,
            commands::install_binary,
            commands::list_servers,
            commands::add_server,
            commands::remove_server,
            commands::test_server,
            commands::deploy_server,
            commands::undeploy_server,
            commands::get_server_status,
            commands::import_frpc_config,
            commands::diagnose_tunnel,
            commands::set_tunnel_auth,
            commands::cf_verify_token,
            commands::cf_list_zones,
            commands::cf_provision,
            commands::cf_deprovision,
            commands::cf_update_hostname,
            commands::cf_get_api_token,
            commands::cf_list_zones_stored,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                commands::shutdown(app_handle);
            }
        });
}
