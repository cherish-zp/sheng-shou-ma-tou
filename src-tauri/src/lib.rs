mod binman;
mod commands;
mod diagnostics;
mod engine;
mod forwarder;
mod frp_deploy;
mod frp_import;
mod models;
mod providers;
mod servers_store;
mod ssh;
mod store;
mod tray;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
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
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Exit = event {
                commands::shutdown(app_handle);
            }
        });
}
