//! Native-only startup observation. No renderer, window, provider turn or pass verdict.
use tauri::Manager;

pub(super) fn start(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let Some(context) = super::active() else {
            crate::eprintln!("[acceptance] Preflight context missing");
            app.exit(1);
            return;
        };
        let result = tokio::time::timeout(std::time::Duration::from_secs(120), observe(&app))
            .await
            .map_err(|_| "Acceptance preflight startup deadline expired".to_string())
            .and_then(|result| result);
        let mut code = i32::from(result.is_err());
        let envelope = result.unwrap_or_else(|error| serde_json::json!({
            "schemaVersion":1,"runId":context.run_id,"caseId":context.case_id,"error":error
        }));
        if let Err(error) = context.write_result(&envelope) {
            crate::eprintln!("[acceptance] Preflight result write failed: {error}");
            code = 1;
        }
        app.exit(code);
    });
}

async fn observe(app: &tauri::AppHandle) -> Result<serde_json::Value, String> {
    let context = super::active().ok_or("Acceptance preflight context unavailable")?;
    let resources = super::RESOURCES
        .get()
        .filter(|resources| resources.run_id == context.run_id)
        .ok_or("Acceptance preflight resource owner mismatch")?;
    let core = crate::daemon::ensure_embedded_ready().await?;
    // Readiness publishes before the initial listener starts. Observe its real result;
    // do not start a replacement listener to turn a failed preflight into success.
    loop {
        if core.is_stopped() { return Err("Acceptance preflight core stopped".into()); }
        let network = core.network_status();
        if let Some(error) = network.error { return Err(error); }
        #[cfg(windows)]
        let transferred = resources.pipe.lock().map_err(|_| "Acceptance preflight pipe lease poisoned")?.is_none();
        #[cfg(not(windows))]
        let transferred = false;
        if network.port.is_some() && transferred { break; }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let plan = context.resource_plan();
    let manager = crate::kernel::server::get_agent_manager();
    let mut installed: Vec<_> = manager.registry().list().into_iter()
        .filter(|provider| provider.is_installed).map(|provider| provider.id).collect();
    installed.sort();
    let providers = super::provider_observations()?;
    let network = core.network_status();
    let paths = app.path();
    let measurements = serde_json::json!({
        "identifier":app.config().identifier,
        "resourceClaimsRunId":resources.run_id,
        // Transfer is not named-pipe delivery; later channel scenarios supply delivery proof.
        "initialPipeLeaseTransferred":true,
        "engineEpoch":core.acceptance_epoch,
        "engineReady":crate::daemon::embedded_phase() == crate::daemon::EnginePhase::Ready,
        "coreStopped":core.is_stopped(),
        "schedulerRegistered":crate::kernel::schedules::scheduler_registered(),
        "windowCount":app.webview_windows().len(),
        "configuredWindowCount":app.config().app.windows.len(),
        "osIntegrationsEnabled":super::os_integrations_enabled(),
        "networkPort":network.port,"networkError":network.error,
        "installedProviders":installed,
        "externalMcpServerCount":crate::kernel::mcp::McpHub::load_external_servers().len(),
        "providerPids":providers.provider_pids,"providerStarts":providers.provider_starts,
        "directories":{
            "home":crate::get_data_dir(),
            "managerHome":manager.acceptance_home(),
            "configHome":crate::kernel::config_store::get_config_store().home_dir(),
            "tauriConfig":paths.app_config_dir().map_err(|error| error.to_string())?,
            "tauriData":paths.app_data_dir().map_err(|error| error.to_string())?,
            "tauriLocalData":paths.app_local_data_dir().map_err(|error| error.to_string())?,
            "tauriCache":paths.app_cache_dir().map_err(|error| error.to_string())?,
            "tauriLogs":paths.app_log_dir().map_err(|error| error.to_string())?,
            "logs":crate::kernel::telemetry::log_directory(super::acceptance_logs_dir().as_deref()),
            "schedules":crate::kernel::schedules::get_schedules_dir(),
            "mcp":crate::kernel::mcp::McpHub::servers_config_path().ok_or("Acceptance preflight MCP path missing")?
        },
        "modelRoots":crate::kernel::server::current_whisper_model_roots(),
        "profiles":plan.profiles,
        "providerCommands":{
            "codex":crate::agent::registry::resolve_command("codex").ok_or("Acceptance preflight fixture unavailable")?,
            "node":crate::agent::registry::resolve_command("node").ok_or("Acceptance preflight fixture runtime unavailable")?
        }
    });
    context.preflight_result(measurements)
}
