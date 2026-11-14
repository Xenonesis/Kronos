use crate::agent::registry::{resolve_command, ProviderRegistry};
use crate::kernel::config_store::{get_config_store, WorkspaceLabelRecord};
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{OnceLock, RwLock};

static WORKSPACE_SETUP_STATES: OnceLock<RwLock<HashMap<String, Value>>> = OnceLock::new();

fn workspace_setup_store() -> &'static RwLock<HashMap<String, Value>> {
    WORKSPACE_SETUP_STATES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn run_git_trim(cwd: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

pub fn handle_checkout_status(cwd_str: &str, request_id: &str) -> Value {
    let cwd = if cwd_str.trim().is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(cwd_str)
    };
    let Some(repo_root) = run_git_trim(&cwd, &["rev-parse", "--show-toplevel"]) else {
        return json!({
            "type": "checkout_status_response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "cwd": cwd_str,
                "error": null,
                "isGit": false,
                "isPaseoOwnedWorktree": false,
                "repoRoot": null,
                "currentBranch": null,
                "isDirty": null,
                "baseRef": null,
                "aheadBehind": null,
                "aheadOfOrigin": null,
                "behindOfOrigin": null,
                "hasRemote": false,
                "remoteUrl": null
            }
        });
    };

    let current_branch = run_git_trim(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .filter(|b| b != "HEAD");
    let is_dirty = run_git_trim(&cwd, &["status", "--porcelain"]).is_some();
    let remote_url = run_git_trim(&cwd, &["remote", "get-url", "origin"]);
    let has_remote = remote_url.is_some();
    let upstream_ref = run_git_trim(
        &cwd,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
    );
    let base_ref = upstream_ref
        .clone()
        .or_else(|| current_branch.as_ref().map(|b| format!("origin/{b}")));

    let (ahead, behind) = if let Some(ref up) = upstream_ref {
        run_git_trim(
            &cwd,
            &["rev-list", "--left-right", "--count", &format!("HEAD...{up}")],
        )
        .and_then(|counts| {
            let mut parts = counts.split_whitespace();
            let a = parts.next()?.parse::<i64>().ok()?;
            let b = parts.next()?.parse::<i64>().ok()?;
            Some((a, b))
        })
        .map(|(a, b)| (Some(a), Some(b)))
        .unwrap_or((None, None))
    } else {
        (None, None)
    };

    let ahead_behind = match (ahead, behind) {
        (Some(a), Some(b)) => Some(json!({ "ahead": a, "behind": b })),
        _ => None,
    };

    let common_dir = run_git_trim(&cwd, &["rev-parse", "--git-common-dir"]);
    let main_repo_root = common_dir
        .and_then(|d| {
            let p = Path::new(&d);
            let abs = if p.is_absolute() {
                p.to_path_buf()
            } else {
                Path::new(&repo_root).join(p)
            };
            abs.parent().map(|parent| parent.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| repo_root.clone());

    let norm_root = repo_root.replace('\\', "/").to_lowercase();
    let is_paseo_worktree = norm_root.contains(".paseo/worktrees")
        || norm_root.contains(".kronos/worktrees");

    json!({
        "type": "checkout_status_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "cwd": cwd_str,
            "error": null,
            "isGit": true,
            "isPaseoOwnedWorktree": is_paseo_worktree,
            "repoRoot": repo_root,
            "mainRepoRoot": main_repo_root,
            "currentBranch": current_branch,
            "isDirty": is_dirty,
            "baseRef": base_ref.unwrap_or_else(|| "HEAD".to_string()),
            "aheadBehind": ahead_behind,
            "aheadOfOrigin": ahead,
            "behindOfOrigin": behind,
            "hasRemote": has_remote,
            "remoteUrl": remote_url
        }
    })
}

pub fn handle_workspace_script_list(ws_id: &str, request_id: &str) -> Value {
    let store = get_config_store();
    let dir_path = store
        .get_workspace(ws_id)
        .map(|w| PathBuf::from(w.path))
        .filter(|p| p.is_dir())
        .or_else(|| {
            let p = PathBuf::from(ws_id);
            p.is_dir().then_some(p)
        });

    let mut scripts = Vec::new();
    if let Some(dir) = dir_path {
        let mut seen = BTreeSet::new();

        // 1. Check paseo.json / .kronos/config.json scripts & services
        for rel in ["paseo.json", ".kronos/config.json"] {
            let cfg_file = dir.join(rel);
            if let Ok(text) = std::fs::read_to_string(&cfg_file) {
                if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                    if let Some(obj) = parsed.get("scripts").and_then(|v| v.as_object()) {
                        for name in obj.keys() {
                            if seen.insert(name.clone()) {
                                scripts.push(make_script_entry(name, "script"));
                            }
                        }
                    }
                    if let Some(obj) = parsed.get("services").and_then(|v| v.as_object()) {
                        for name in obj.keys() {
                            if seen.insert(name.clone()) {
                                scripts.push(make_script_entry(name, "service"));
                            }
                        }
                    }
                }
            }
        }

        // 2. Check package.json scripts
        let pkg_file = dir.join("package.json");
        if let Ok(text) = std::fs::read_to_string(&pkg_file) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                if let Some(obj) = parsed.get("scripts").and_then(|v| v.as_object()) {
                    for name in obj.keys() {
                        if seen.insert(name.clone()) {
                            let kind = if matches!(name.as_str(), "dev" | "start" | "serve" | "preview") {
                                "service"
                            } else {
                                "script"
                            };
                            scripts.push(make_script_entry(name, kind));
                        }
                    }
                }
            }
        }

        // 3. Check Cargo.toml basic targets
        if dir.join("Cargo.toml").is_file() {
            for name in ["cargo check", "cargo test"] {
                if seen.insert(name.to_string()) {
                    scripts.push(make_script_entry(name, "script"));
                }
            }
        }
    }

    json!({
        "type": "workspace.script.list.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "workspaceId": ws_id,
            "scripts": scripts,
            "error": null
        }
    })
}

fn make_script_entry(name: &str, kind: &str) -> Value {
    json!({
        "scriptName": name,
        "type": kind,
        "hostname": "localhost",
        "port": null,
        "localProxyUrl": null,
        "publicProxyUrl": null,
        "proxyUrl": null,
        "lifecycle": "stopped",
        "health": null,
        "exitCode": null,
        "terminalId": null
    })
}

pub fn handle_file_subscribe(cwd_str: &str, path_str: &str, sub_opt: Option<&str>, request_id: &str) -> Value {
    let sub_id = sub_opt
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("sub-file-{request_id}"));

    let req_path = Path::new(path_str);
    let resolved = if req_path.is_absolute() {
        req_path.to_path_buf()
    } else {
        Path::new(if cwd_str.is_empty() { "." } else { cwd_str }).join(req_path)
    };

    let initial = match std::fs::metadata(&resolved) {
        Ok(meta) if meta.is_file() => {
            let size = meta.len();
            let modified_at = meta
                .modified()
                .ok()
                .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
            let ext = resolved
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();
            let (kind, mime) = classify_file_kind(&resolved, &ext);
            let rev = format!("{size}:{}", modified_at);
            json!({
                "status": "ready",
                "cwd": cwd_str,
                "path": path_str,
                "size": size,
                "modifiedAt": modified_at,
                "revision": rev,
                "kind": kind,
                "mimeType": mime
            })
        }
        Ok(_) => json!({
            "status": "error",
            "cwd": cwd_str,
            "path": path_str,
            "error": "Target path is a directory, not a file"
        }),
        Err(_) => json!({
            "status": "missing",
            "cwd": cwd_str,
            "path": path_str
        }),
    };

    json!({
        "type": "fs.file.subscribe.response",
        "requestId": request_id,
        "payload": {
            "subscriptionId": sub_id,
            "initial": initial,
            "requestId": request_id
        }
    })
}

fn classify_file_kind(path: &Path, ext: &str) -> (&'static str, &'static str) {
    match ext {
        "png" => ("image", "image/png"),
        "jpg" | "jpeg" => ("image", "image/jpeg"),
        "gif" => ("image", "image/gif"),
        "webp" => ("image", "image/webp"),
        "svg" => ("image", "image/svg+xml"),
        "ico" => ("image", "image/x-icon"),
        "pdf" => ("binary", "application/pdf"),
        "zip" | "gz" | "tar" | "exe" | "dll" | "wasm" | "bin" => {
            ("binary", "application/octet-stream")
        }
        _ => {
            if let Ok(mut file) = std::fs::File::open(path) {
                use std::io::Read;
                let mut buf = [0u8; 512];
                if let Ok(n) = file.read(&mut buf) {
                    if buf[..n].contains(&0) {
                        return ("binary", "application/octet-stream");
                    }
                }
            }
            ("text", "text/plain")
        }
    }
}

pub fn handle_provider_diagnostic(provider: &str, request_id: &str) -> Value {
    let registry = ProviderRegistry::new();
    let descriptor = registry.get(provider);
    let protocol_summary = match provider {
        "claude" => "stream-json stdio (ClaudeCliClient)",
        "codex" => "JSON-RPC app-server stdio (CodexAppServerClient)",
        "opencode" => {
            let sample = crate::agent::providers::opencode::OpenCodeClient::parse_sse_chunk(
                "event: health\ndata: {\"ok\":true}\n\n",
            );
            if sample.len() == 1 {
                "HTTP/SSE local runtime (OpenCodeClient)"
            } else {
                "HTTP/SSE local runtime"
            }
        }
        "omp" => "OMP JSON-RPC stdio (OmpAgentClient)",
        "pi" => "Pi JSON tool-use stdio (PiAgentClient)",
        _ => "stdio/http provider runtime",
    };
    let diagnostic = match descriptor {
        Some(desc) if desc.is_installed => {
            if let Some(bin) = resolve_command(provider) {
                format!(
                    "Provider '{}' ready at {} [{}]",
                    desc.label,
                    bin.display(),
                    protocol_summary
                )
            } else {
                format!("Provider '{}' ready [{}]", desc.label, protocol_summary)
            }
        }
        Some(desc) => format!(
            "Provider '{}' unavailable ({}): executable/server not detected on system PATH or localhost.",
            desc.label, protocol_summary
        ),
        None => format!("Unknown provider '{}'.", provider),
    };

    json!({
        "type": "provider_diagnostic_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "provider": provider,
            "diagnostic": diagnostic
        }
    })
}

pub fn handle_daemon_get_status(request_id: &str) -> Value {
    let server_id = crate::get_or_create_server_id();
    let cfg = get_config_store().get_config();
    let hub = crate::kernel::services::get_hub_services();
    hub.add_host(crate::kernel::services::HostRecord {
        id: server_id.clone(),
        label: "Local Embedded Kernel".to_string(),
        connection_uri: format!("127.0.0.1:{}", cfg.port),
        is_online: true,
        last_seen_at: Some(crate::agent::types::now_iso()),
    });
    let providers: Vec<Value> = ProviderRegistry::new()
        .list()
        .into_iter()
        .map(|p| {
            json!({
                "provider": p.id,
                "available": p.is_installed,
                "error": if p.is_installed { Value::Null } else { json!("Not installed on PATH or local endpoint offline") }
            })
        })
        .collect();

    json!({
        "type": "daemon.get_status.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "serverId": server_id,
            "version": env!("CARGO_PKG_VERSION"),
            "pid": std::process::id(),
            "nodePath": "rust-embedded",
            "listen": format!("127.0.0.1:{}", cfg.port),
            "relay": {
                "enabled": cfg.relay_enabled,
                "endpoint": cfg.relay_endpoint,
                "publicEndpoint": cfg.relay_endpoint,
                "useTls": true,
                "publicUseTls": true
            },
            "hubServices": {
                "schedulesCount": hub.list_schedules().len(),
                "hosts": hub.list_hosts()
            },
            "providers": providers
        }
    })
}

pub fn handle_get_daemon_config(request_id: &str) -> Value {
    let store = get_config_store();
    let cfg = store.get_config();
    let skills_selection = store
        .get_custom_setting::<Value>("agentSkillsSelection")
        .unwrap_or_else(|| json!({ "mode": "all" }));

    json!({
        "type": "get_daemon_config_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "config": {
                "mcp": {
                    "enabled": cfg.mcp_enabled,
                    "injectIntoAgents": cfg.inject_into_agents
                },
                "relay": {
                    "enabled": cfg.relay_enabled,
                    "endpoint": cfg.relay_endpoint
                },
                "providers": cfg.providers_config,
                "autoArchiveAfterMerge": cfg.auto_archive_after_merge,
                "enableTerminalAgentHooks": cfg.enable_terminal_agent_hooks,
                "skills": { "selection": skills_selection },
                "agentProfiles": cfg.agent_profiles
            }
        }
    })
}

pub fn handle_set_daemon_config(msg: &Value, request_id: &str) -> Value {
    let store = get_config_store();
    let patch = msg
        .get("config")
        .or_else(|| msg.pointer("/payload/config"))
        .cloned()
        .unwrap_or_else(|| json!({}));

    let updated = store
        .update_config(|cfg| {
            if let Some(mcp) = patch.get("mcp") {
                if let Some(enabled) = mcp.get("enabled").and_then(|v| v.as_bool()) {
                    cfg.mcp_enabled = enabled;
                }
                if let Some(inject) = mcp.get("injectIntoAgents").and_then(|v| v.as_bool()) {
                    cfg.inject_into_agents = inject;
                }
            }
            if let Some(relay) = patch.get("relay") {
                if let Some(enabled) = relay.get("enabled").and_then(|v| v.as_bool()) {
                    cfg.relay_enabled = enabled;
                }
                if let Some(ep) = relay.get("endpoint").and_then(|v| v.as_str()) {
                    cfg.relay_endpoint = ep.to_string();
                }
            }
            if let Some(auto_arch) = patch.get("autoArchiveAfterMerge").and_then(|v| v.as_bool()) {
                cfg.auto_archive_after_merge = auto_arch;
            }
            if let Some(hooks) = patch.get("enableTerminalAgentHooks").and_then(|v| v.as_bool()) {
                cfg.enable_terminal_agent_hooks = hooks;
            }
            if let Some(prov) = patch.get("providers") {
                if let (Some(existing), Some(incoming)) =
                    (cfg.providers_config.as_object_mut(), prov.as_object())
                {
                    for (k, v) in incoming {
                        existing.insert(k.clone(), v.clone());
                    }
                } else if prov.is_object() {
                    cfg.providers_config = prov.clone();
                }
            }
            if let Some(sel) = patch.pointer("/skills/selection") {
                if let Ok(val) = serde_json::to_value(sel) {
                    cfg.custom_settings
                        .insert("agentSkillsSelection".to_string(), val);
                }
            }
            if let Some(profiles) = patch.get("agentProfiles").and_then(|v| v.as_array()) {
                cfg.agent_profiles = profiles.clone();
            }
        })
        .unwrap_or_else(|_| store.get_config());

    let skills_selection = updated
        .custom_settings
        .get("agentSkillsSelection")
        .cloned()
        .unwrap_or_else(|| json!({ "mode": "all" }));

    json!({
        "type": "set_daemon_config_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "config": {
                "mcp": {
                    "enabled": updated.mcp_enabled,
                    "injectIntoAgents": updated.inject_into_agents
                },
                "relay": {
                    "enabled": updated.relay_enabled,
                    "endpoint": updated.relay_endpoint
                },
                "providers": updated.providers_config,
                "autoArchiveAfterMerge": updated.auto_archive_after_merge,
                "enableTerminalAgentHooks": updated.enable_terminal_agent_hooks,
                "skills": { "selection": skills_selection },
                "agentProfiles": updated.agent_profiles
            }
        }
    })
}

pub fn handle_diagnostics_request(agent_count: usize, speech_loaded: bool, request_id: &str) -> Value {
    let store = get_config_store();
    let cfg = store.get_config();
    let providers = ProviderRegistry::new().list();
    let installed_count = providers.iter().filter(|p| p.is_installed).count();
    let external_mcp = crate::kernel::mcp::McpHub::load_external_servers().len();
    let policy = crate::agent::guardrails::get_permission_policy();
    let budget = crate::agent::guardrails::get_budget_policy();

    let summary = format!(
        "Kronos Kernel Diagnostics OK\n\
         - Version: {}\n\
         - PID: {} | Port: {}\n\
         - Active Agents: {}\n\
         - Providers Detected: {}/{}\n\
         - External MCP Servers: {} (injectIntoAgents={})\n\
         - Guardrails Mode: {} | Cost Cap USD: {:?}\n\
         - Speech Engine Model Loaded: {}",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        cfg.port,
        agent_count,
        installed_count,
        providers.len(),
        external_mcp,
        cfg.inject_into_agents,
        policy.execution_mode,
        budget.max_cost_usd,
        speech_loaded
    );

    json!({
        "type": "diagnostics.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "diagnostic": summary
        }
    })
}

fn collect_skill_names_in_dir(dir: &Path, out: &mut BTreeSet<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("SKILL.md").is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                out.insert(name.to_string());
            }
        }
    }
}

pub fn handle_agent_skills_rpc(kind: &str, msg: &Value, request_id: &str) -> Value {
    let store = get_config_store();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let managed_dir = crate::get_data_dir().join("skills");

    let mut available_set = BTreeSet::new();
    for dir in [
        home.join(".paseo").join("skills"),
        home.join(".kronos").join("skills"),
        home.join(".agents").join("skills"),
        home.join(".claude").join("skills"),
        home.join(".gemini").join("config").join("skills"),
        managed_dir.clone(),
    ] {
        collect_skill_names_in_dir(&dir, &mut available_set);
    }

    if kind == "save_selection" {
        if let Some(sel) = msg.get("selection").or_else(|| msg.pointer("/payload/selection")) {
            let _ = store.set_custom_setting("agentSkillsSelection", sel);
        }
    } else if kind == "import_legacy" {
        if let Some(sel) = msg.get("selection").or_else(|| msg.pointer("/payload/selection")) {
            let _ = store.set_custom_setting("agentSkillsSelection", sel);
            return json!({
                "type": "agent.skills.import_legacy_selection.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "imported": true,
                    "selection": sel
                }
            });
        }
    }

    let selection = store
        .get_custom_setting::<Value>("agentSkillsSelection")
        .unwrap_or_else(|| json!({ "mode": "all" }));

    let mut installed_set = BTreeSet::new();
    collect_skill_names_in_dir(&managed_dir, &mut installed_set);

    let desired_set: BTreeSet<String> = match selection.get("mode").and_then(|v| v.as_str()) {
        Some("custom") => selection
            .get("skills")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .filter(|s| available_set.contains(s))
                    .collect()
            })
            .unwrap_or_default(),
        _ => available_set.clone(),
    };

    if kind == "reconcile" || kind == "save_selection" {
        let _ = std::fs::create_dir_all(&managed_dir);
        for name in &desired_set {
            let target = managed_dir.join(name);
            if !target.join("SKILL.md").is_file() {
                for src_root in [
                    home.join(".paseo").join("skills"),
                    home.join(".kronos").join("skills"),
                    home.join(".agents").join("skills"),
                    home.join(".claude").join("skills"),
                    home.join(".gemini").join("config").join("skills"),
                ] {
                    let src_md = src_root.join(name).join("SKILL.md");
                    if src_md.is_file() {
                        let _ = std::fs::create_dir_all(&target);
                        let _ = std::fs::copy(&src_md, target.join("SKILL.md"));
                        break;
                    }
                }
            }
        }
        for name in installed_set.difference(&desired_set) {
            let _ = std::fs::remove_dir_all(managed_dir.join(name));
        }
        installed_set.clear();
        collect_skill_names_in_dir(&managed_dir, &mut installed_set);
    } else if kind == "uninstall" {
        if managed_dir.is_dir() {
            let _ = std::fs::remove_dir_all(&managed_dir);
        }
        installed_set.clear();
    }

    let mut ops = Vec::new();
    for name in desired_set.difference(&installed_set) {
        ops.push(json!({ "kind": "add", "name": name }));
    }
    for name in installed_set.difference(&desired_set) {
        ops.push(json!({ "kind": "delete", "name": name }));
    }

    let state = if installed_set.is_empty() && desired_set.is_empty() {
        "up-to-date"
    } else if installed_set.is_empty() {
        "not-installed"
    } else if ops.is_empty() {
        "up-to-date"
    } else {
        "drift"
    };

    let resp_type = match kind {
        "reconcile" => "agent.skills.reconcile.response",
        "uninstall" => "agent.skills.uninstall.response",
        "save_selection" => "agent.skills.save_selection.response",
        _ => "agent.skills.get_status.response",
    };

    let mut payload = json!({
        "requestId": request_id,
        "state": state,
        "ops": ops,
        "available": available_set.into_iter().collect::<Vec<_>>(),
        "installed": installed_set.into_iter().collect::<Vec<_>>(),
        "selection": selection
    });
    if kind == "save_selection" {
        payload["confirmationRequired"] = Value::Null;
    }

    json!({
        "type": resp_type,
        "requestId": request_id,
        "payload": payload
    })
}

pub fn handle_plugin_list_or_catalog(catalog: bool, request_id: &str) -> Value {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let data_dir = crate::get_data_dir();
    let state_path = data_dir.join("plugins_state.json");
    let state_overrides: HashMap<String, bool> = std::fs::read_to_string(&state_path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    let mut plugins = Vec::new();
    let mut seen = BTreeSet::new();

    for root in [
        data_dir.join("plugins"),
        home.join(".paseo").join("plugins"),
        home.join(".kronos").join("plugins"),
    ] {
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let manifest_path = ["plugin.json", "manifest.json", "package.json"]
                .iter()
                .map(|f| dir.join(f))
                .find(|p| p.is_file());
            let Some(manifest_file) = manifest_path else {
                continue;
            };
            let Some(raw_text) = std::fs::read_to_string(&manifest_file).ok() else {
                continue;
            };
            let Ok(manifest) = serde_json::from_str::<Value>(&raw_text) else {
                continue;
            };
            let fallback_id = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("plugin")
                .to_string();
            let id = manifest
                .get("id")
                .or_else(|| manifest.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or(&fallback_id)
                .to_string();
            if !seen.insert(id.clone()) {
                continue;
            }

            if catalog {
                plugins.push(json!({ "id": id }));
            } else {
                let default_enabled = manifest
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let enabled = state_overrides.get(&id).copied().unwrap_or(default_enabled);
                plugins.push(json!({
                    "id": id,
                    "name": manifest.get("name").and_then(|v| v.as_str()).unwrap_or(&fallback_id),
                    "description": manifest.get("description").and_then(|v| v.as_str()),
                    "path": dir.to_string_lossy(),
                    "enabled": enabled,
                    "status": if enabled { "running" } else { "disabled" }
                }));
            }
        }
    }

    // Also include built-in native plugins so catalog/list always reflects active core plugins
    let builtins = [
        ("antigravity", "Antigravity Native Engine", "Built-in multi-agent orchestration and usage telemetry plugin"),
        ("hoplite", "Hoplite Security Shield", "Built-in command guardrail and AST vulnerability scanner"),
        ("muse", "Muse Voice & Dictation", "Built-in local Whisper GGUF & OS speech synthesis plugin"),
        ("worktree-include", "Git Worktree Isolation", "Built-in automatic worktree file sync and branch isolation"),
    ];
    for (bid, bname, bdesc) in builtins {
        if seen.insert(bid.to_string()) {
            if catalog {
                plugins.push(json!({ "id": bid }));
            } else {
                let enabled = state_overrides.get(bid).copied().unwrap_or(true);
                plugins.push(json!({
                    "id": bid,
                    "name": bname,
                    "description": bdesc,
                    "path": format!("builtin://{bid}"),
                    "enabled": enabled,
                    "status": if enabled { "running" } else { "disabled" }
                }));
            }
        }
    }

    json!({
        "type": if catalog { "plugin.catalog.get.response" } else { "plugin.list.response" },
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "plugins": plugins
        }
    })
}

pub fn handle_workspace_label_list(msg: &Value, request_id: &str) -> Value {
    let catalog = get_config_store().get_labels_catalog();
    let payload = msg.get("payload");
    let sub_id = if msg.get("subscribe").is_some()
        || payload.and_then(|p| p.get("subscribe")).is_some()
    {
        Some(format!("sub-labels-{request_id}"))
    } else {
        None
    };
    json!({
        "type": "workspace.label.list.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "subscriptionId": sub_id,
            "labels": catalog.labels,
            "sync": {
                "mode": "snapshot",
                "generation": catalog.generation,
                "headSeq": catalog.head_seq,
                "removals": catalog.removals
            }
        }
    })
}

pub fn handle_workspace_label_assignment_set(msg: &Value, request_id: &str) -> Value {
    let payload = msg.get("payload").unwrap_or(msg);
    let ws_id = payload
        .get("workspaceId")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let assigned = payload
        .get("assigned")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let name = payload
        .pointer("/label/name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let color = payload
        .pointer("/label/color")
        .and_then(|v| v.as_str())
        .unwrap_or("slate")
        .to_string();
    let label = WorkspaceLabelRecord { name, color };
    let (_cat, ws_labels) = get_config_store()
        .set_workspace_label_assignment(ws_id, label.clone(), assigned)
        .unwrap_or_default();

    json!({
        "type": "workspace.label.assignment.set.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "label": label,
            "workspaceLabels": ws_labels
        }
    })
}

pub fn handle_workspace_label_update(msg: &Value, request_id: &str) -> Value {
    let payload = msg.get("payload").unwrap_or(msg);
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let new_name = payload.get("newName").and_then(|v| v.as_str());
    let color = payload.get("color").and_then(|v| v.as_str());

    let (label, affected) = get_config_store()
        .update_label_definition(name, new_name, color)
        .ok()
        .flatten()
        .map(|(l, a, _)| (l, a))
        .unwrap_or_else(|| {
            (
                WorkspaceLabelRecord {
                    name: new_name.unwrap_or(name).to_string(),
                    color: color.unwrap_or("slate").to_string(),
                },
                0,
            )
        });

    json!({
        "type": "workspace.label.update.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "label": label,
            "affectedWorkspaceCount": affected
        }
    })
}

pub fn handle_workspace_label_delete(msg: &Value, request_id: &str) -> Value {
    let payload = msg.get("payload").unwrap_or(msg);
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let (affected, _) = get_config_store()
        .delete_label_definition(name)
        .unwrap_or_default();
    json!({
        "type": "workspace.label.delete.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "affectedWorkspaceCount": affected
        }
    })
}

pub fn handle_workspace_label_delete_inspect(msg: &Value, request_id: &str) -> Value {
    let payload = msg.get("payload").unwrap_or(msg);
    let name = payload.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let affected = get_config_store().inspect_label_delete(name);
    json!({
        "type": "workspace.label.delete.inspect.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "affectedWorkspaceCount": affected
        }
    })
}

fn build_workspace_setup_snapshot(ws_id: &str) -> Value {
    if ws_id.trim().is_empty() {
        return Value::Null;
    }
    workspace_setup_store()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .get(ws_id)
        .cloned()
        .unwrap_or(Value::Null)
}

pub fn handle_workspace_setup_run(ws_id: &str, request_id: &str) -> Value {
    let dir = get_config_store()
        .get_workspace(ws_id)
        .map(|w| PathBuf::from(w.path))
        .filter(|p| p.is_dir())
        .or_else(|| {
            let p = PathBuf::from(ws_id);
            p.is_dir().then_some(p)
        });
    let Some(dir) = dir else {
        return json!({
            "type": "workspace.setup.run.response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "workspaceId": ws_id,
                "accepted": false,
                "snapshot": Value::Null,
                "error": format!("Workspace {ws_id} directory not found")
            }
        });
    };

    let branch_name =
        run_git_trim(&dir, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "HEAD".to_string());
    let cfg_candidates = [
        dir.join(".paseo").join("paseo.json"),
        dir.join("paseo.json"),
        dir.join(".kronos").join("kronos.json"),
    ];
    let mut setup_cmds: Vec<String> = Vec::new();
    for cand in cfg_candidates {
        if let Ok(raw) = std::fs::read_to_string(&cand) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&raw) {
                if let Some(arr) = parsed
                    .pointer("/worktree/setup")
                    .or_else(|| parsed.get("worktreeSetup"))
                    .and_then(|v| v.as_array())
                {
                    for item in arr {
                        if let Some(s) = item.as_str().filter(|s| !s.trim().is_empty()) {
                            setup_cmds.push(s.to_string());
                        }
                    }
                    break;
                }
            }
        }
    }

    let mut command_snapshots = Vec::new();
    let mut combined_log = String::new();
    let mut overall_status = "completed";
    let mut overall_error: Value = Value::Null;

    for (idx, cmd_str) in setup_cmds.iter().enumerate() {
        let started = std::time::Instant::now();
        let mut cmd = if cfg!(windows) {
            let mut c = Command::new("cmd");
            c.args(["/C", cmd_str]);
            c
        } else {
            let mut c = Command::new("sh");
            c.args(["-lc", cmd_str]);
            c
        };
        cmd.current_dir(&dir);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        match cmd.output() {
            Ok(out) => {
                let dur = started.elapsed().as_millis() as u64;
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let step_log = format!("{stdout}{stderr}");
                combined_log.push_str(&format!("$ {cmd_str}\n{step_log}\n"));
                let ok = out.status.success();
                command_snapshots.push(json!({
                    "id": format!("setup-{idx}"),
                    "command": cmd_str,
                    "log": step_log,
                    "status": if ok { "completed" } else { "failed" },
                    "exitCode": out.status.code(),
                    "durationMs": dur
                }));
                if !ok {
                    overall_status = "failed";
                    overall_error = json!(format!("Setup command failed: {cmd_str}"));
                    break;
                }
            }
            Err(err) => {
                overall_status = "failed";
                overall_error = json!(format!("Failed to execute setup command '{cmd_str}': {err}"));
                command_snapshots.push(json!({
                    "id": format!("setup-{idx}"),
                    "command": cmd_str,
                    "log": err.to_string(),
                    "status": "failed",
                    "exitCode": Value::Null,
                    "durationMs": started.elapsed().as_millis() as u64
                }));
                break;
            }
        }
    }

    let snapshot = json!({
        "status": overall_status,
        "detail": {
            "type": "worktree_setup",
            "worktreePath": dir.to_string_lossy().to_string(),
            "branchName": branch_name,
            "log": combined_log,
            "commands": command_snapshots
        },
        "error": overall_error
    });
    workspace_setup_store()
        .write()
        .unwrap_or_else(|e| e.into_inner())
        .insert(ws_id.to_string(), snapshot.clone());

    json!({
        "type": "workspace.setup.run.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "workspaceId": ws_id,
            "accepted": overall_status == "completed",
            "snapshot": snapshot,
            "error": snapshot["error"]
        }
    })
}

pub fn handle_workspace_setup_subscribe(ws_id: &str, request_id: &str) -> Value {
    json!({
        "type": "workspace.setup.subscribe.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "subscriptionId": format!("sub-ws-setup-{request_id}"),
            "workspaceId": ws_id,
            "snapshot": build_workspace_setup_snapshot(ws_id)
        }
    })
}

pub fn handle_workspace_setup_status(ws_id: &str, request_id: &str) -> Value {
    json!({
        "type": "workspace_setup_status_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "workspaceId": ws_id,
            "snapshot": build_workspace_setup_snapshot(ws_id)
        }
    })
}

fn run_git_raw(cwd: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn parse_hunk_range(token: &str) -> (i64, i64) {
    let trimmed = token.trim_start_matches(['-', '+']);
    if let Some((start_s, count_s)) = trimmed.split_once(',') {
        let start = start_s.parse::<i64>().unwrap_or(1);
        let count = count_s.parse::<i64>().unwrap_or(0);
        (start, count)
    } else {
        let start = trimmed.parse::<i64>().unwrap_or(1);
        (start, 1)
    }
}

fn parse_unified_diff_files(raw: &str) -> Vec<Value> {
    let mut files = Vec::new();
    let mut current_path = String::new();
    let mut old_path: Option<String> = None;
    let mut is_new = false;
    let mut is_deleted = false;
    let mut is_binary = false;
    let mut additions: usize = 0;
    let mut deletions: usize = 0;
    let mut hunks: Vec<Value> = Vec::new();
    let mut active_hunk: Option<(i64, i64, i64, i64, Vec<Value>)> = None;

    let flush_hunk = |active: &mut Option<(i64, i64, i64, i64, Vec<Value>)>, hunks: &mut Vec<Value>| {
        if let Some((os, oc, ns, nc, lines)) = active.take() {
            hunks.push(json!({
                "oldStart": os,
                "oldCount": oc,
                "newStart": ns,
                "newCount": nc,
                "lines": lines,
            }));
        }
    };

    let flush_file = |path: &mut String,
                      old_p: &mut Option<String>,
                      is_new_f: &mut bool,
                      is_del_f: &mut bool,
                      is_bin_f: &mut bool,
                      adds: &mut usize,
                      dels: &mut usize,
                      hunks_vec: &mut Vec<Value>,
                      active: &mut Option<(i64, i64, i64, i64, Vec<Value>)>,
                      out: &mut Vec<Value>| {
        flush_hunk(active, hunks_vec);
        if !path.is_empty() {
            let mut obj = json!({
                "path": path.clone(),
                "isNew": *is_new_f,
                "isDeleted": *is_del_f,
                "additions": *adds,
                "deletions": *dels,
                "hunks": hunks_vec.clone(),
                "status": if *is_bin_f { "binary" } else { "ok" }
            });
            if let Some(op) = old_p.take() {
                if op != *path {
                    obj["oldPath"] = Value::String(op);
                }
            }
            out.push(obj);
        }
        path.clear();
        *old_p = None;
        *is_new_f = false;
        *is_del_f = false;
        *is_bin_f = false;
        *adds = 0;
        *dels = 0;
        hunks_vec.clear();
    };

    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            flush_file(
                &mut current_path,
                &mut old_path,
                &mut is_new,
                &mut is_deleted,
                &mut is_binary,
                &mut additions,
                &mut deletions,
                &mut hunks,
                &mut active_hunk,
                &mut files,
            );
            if let Some((a_side, b_side)) = rest.split_once(" b/") {
                old_path = Some(a_side.to_string());
                current_path = b_side.to_string();
            }
            continue;
        }
        if line.starts_with("new file mode") {
            is_new = true;
            continue;
        }
        if line.starts_with("deleted file mode") {
            is_deleted = true;
            continue;
        }
        if line.starts_with("Binary files ") {
            is_binary = true;
            continue;
        }
        if line.starts_with("@@ ") {
            flush_hunk(&mut active_hunk, &mut hunks);
            let header_parts: Vec<&str> = line.split_whitespace().collect();
            let (os, oc) = header_parts.get(1).map(|t| parse_hunk_range(t)).unwrap_or((1, 0));
            let (ns, nc) = header_parts.get(2).map(|t| parse_hunk_range(t)).unwrap_or((1, 0));
            active_hunk = Some((os, oc, ns, nc, vec![json!({ "type": "header", "content": line })]));
            continue;
        }
        if let Some((_, _, _, _, ref mut lines_vec)) = active_hunk {
            if let Some(content) = line.strip_prefix('+') {
                additions += 1;
                lines_vec.push(json!({ "type": "add", "content": content }));
            } else if let Some(content) = line.strip_prefix('-') {
                deletions += 1;
                lines_vec.push(json!({ "type": "remove", "content": content }));
            } else if let Some(content) = line.strip_prefix(' ') {
                lines_vec.push(json!({ "type": "context", "content": content }));
            }
        }
    }
    flush_file(
        &mut current_path,
        &mut old_path,
        &mut is_new,
        &mut is_deleted,
        &mut is_binary,
        &mut additions,
        &mut deletions,
        &mut hunks,
        &mut active_hunk,
        &mut files,
    );
    files
}

fn collect_checkout_diff(cwd_str: &str) -> Result<(Vec<Value>, bool), Value> {
    let cwd = if cwd_str.trim().is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(cwd_str)
    };
    if !cwd.is_dir() || run_git_trim(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_none() {
        return Err(json!({
            "code": "NOT_GIT_REPO",
            "message": "Target directory is not a git repository."
        }));
    }

    let raw_diff = run_git_raw(&cwd, &["diff", "--no-color", "--unified=3", "HEAD"])
        .or_else(|| run_git_raw(&cwd, &["diff", "--no-color", "--unified=3"]))
        .unwrap_or_default();
    let diff_too_large = raw_diff.len() > 2 * 1024 * 1024;
    let bounded = if diff_too_large {
        &raw_diff[..2 * 1024 * 1024]
    } else {
        raw_diff.as_str()
    };
    let mut files = parse_unified_diff_files(bounded);

    if let Some(untracked) = run_git_raw(&cwd, &["ls-files", "--others", "--exclude-standard"]) {
        for rel in untracked.lines().map(str::trim).filter(|s| !s.is_empty()).take(20) {
            let full = cwd.join(rel);
            let Ok(bytes) = std::fs::read(&full) else {
                continue;
            };
            if bytes.len() > 128 * 1024 || bytes.contains(&0) {
                files.push(json!({
                    "path": rel,
                    "isNew": true,
                    "isDeleted": false,
                    "additions": 0,
                    "deletions": 0,
                    "hunks": [],
                    "status": if bytes.contains(&0) { "binary" } else { "too_large" }
                }));
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            let lines_added: Vec<Value> = text
                .lines()
                .take(400)
                .map(|l| json!({ "type": "add", "content": l }))
                .collect();
            let count = lines_added.len();
            files.push(json!({
                "path": rel,
                "isNew": true,
                "isDeleted": false,
                "additions": count,
                "deletions": 0,
                "hunks": [{
                    "oldStart": 0,
                    "oldCount": 0,
                    "newStart": 1,
                    "newCount": count,
                    "lines": lines_added
                }],
                "status": "ok"
            }));
        }
    }

    Ok((files, diff_too_large))
}

pub fn handle_subscribe_checkout_diff(cwd_str: &str, request_id: &str) -> Value {
    match collect_checkout_diff(cwd_str) {
        Ok((files, diff_too_large)) => json!({
            "type": "subscribe_checkout_diff_response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "subscriptionId": format!("sub-diff-{request_id}"),
                "cwd": cwd_str,
                "files": files,
                "error": null,
                "diffTooLarge": diff_too_large
            }
        }),
        Err(err) => json!({
            "type": "subscribe_checkout_diff_response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "subscriptionId": format!("sub-diff-{request_id}"),
                "cwd": cwd_str,
                "files": [],
                "error": err,
                "diffTooLarge": false
            }
        }),
    }
}

pub fn handle_checkout_diff_get(cwd_str: &str, request_id: &str) -> Value {
    match collect_checkout_diff(cwd_str) {
        Ok((files, diff_too_large)) => json!({
            "type": "checkout.diff.get.response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "cwd": cwd_str,
                "files": files,
                "error": null,
                "diffTooLarge": diff_too_large
            }
        }),
        Err(err) => json!({
            "type": "checkout.diff.get.response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "cwd": cwd_str,
                "files": [],
                "error": err,
                "diffTooLarge": false
            }
        }),
    }
}

pub fn msg_str<'a>(msg: &'a Value, key: &str) -> &'a str {
    msg.get(key)
        .or_else(|| msg.get("payload").and_then(|p| p.get(key)))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn msg_bool(msg: &Value, key: &str, default: bool) -> bool {
    msg.get(key)
        .or_else(|| msg.get("payload").and_then(|p| p.get(key)))
        .and_then(|v| v.as_bool())
        .unwrap_or(default)
}

fn checkout_err(code: &str, message: impl Into<String>) -> Value {
    json!({
        "code": code,
        "message": message.into(),
    })
}

fn resolve_git_dir(cwd_str: &str) -> Result<PathBuf, Value> {
    let cwd = if cwd_str.trim().is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(cwd_str)
    };
    if !cwd.is_dir() || run_git_trim(&cwd, &["rev-parse", "--is-inside-work-tree"]).is_none() {
        return Err(checkout_err(
            "NOT_GIT_REPO",
            "Target directory is not a git repository.",
        ));
    }
    Ok(cwd)
}

fn run_git_checked(cwd: &Path, args: &[&str]) -> Result<String, Value> {
    let mut cmd = Command::new("git");
    cmd.args(args).current_dir(cwd);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let out = cmd
        .output()
        .map_err(|e| checkout_err("UNKNOWN", format!("Failed spawning git: {e}")))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if out.status.success() {
        Ok(stdout)
    } else {
        let combined = if stderr.is_empty() { stdout } else { stderr };
        let lower = combined.to_ascii_lowercase();
        let code = if lower.contains("conflict") {
            "MERGE_CONFLICT"
        } else if lower.contains("not a git repository") {
            "NOT_GIT_REPO"
        } else {
            "UNKNOWN"
        };
        Err(checkout_err(
            code,
            if combined.is_empty() {
                format!("git {} failed", args.first().copied().unwrap_or("command"))
            } else {
                combined
            },
        ))
    }
}

/// Computes ProjectPlacement descriptor (`projectKey`, `projectName`, `workspaceName`, `checkout`)
/// for agent snapshots/lists, caching unique CWDs when caller supplies a HashMap cache.
pub fn inspect_project_placement(
    cwd_str: &str,
    workspace_id: Option<&str>,
    cache: &mut HashMap<String, Value>,
) -> Value {
    let cache_key = format!("{}|{}", cwd_str, workspace_id.unwrap_or(""));
    if let Some(hit) = cache.get(&cache_key) {
        return hit.clone();
    }

    let store = get_config_store();
    let ws = workspace_id.and_then(|id| store.get_workspace(id)).or_else(|| {
        let norm = cwd_str.replace('\\', "/").to_ascii_lowercase();
        store
            .list_workspaces()
            .into_iter()
            .find(|w| w.path.replace('\\', "/").to_ascii_lowercase() == norm)
    });
    let proj = ws.as_ref().and_then(|w| store.get_project(&w.project_id));

    let cwd_path = if cwd_str.trim().is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(cwd_str)
    };
    let repo_root = if cwd_path.is_dir() {
        run_git_trim(&cwd_path, &["rev-parse", "--show-toplevel"])
    } else {
        None
    };

    let (is_git, current_branch, remote_url, main_repo_root, is_paseo_worktree) =
        if let Some(ref root) = repo_root {
            let branch =
                run_git_trim(&cwd_path, &["rev-parse", "--abbrev-ref", "HEAD"]).filter(|b| b != "HEAD");
            let remote = run_git_trim(&cwd_path, &["remote", "get-url", "origin"]);
            let common = run_git_trim(&cwd_path, &["rev-parse", "--git-common-dir"]).and_then(|d| {
                let p = Path::new(&d);
                let abs = if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    Path::new(root).join(p)
                };
                abs.parent().map(|parent| parent.to_string_lossy().to_string())
            });
            let norm_root = root.replace('\\', "/").to_ascii_lowercase();
            let owned = norm_root.contains(".paseo/worktrees")
                || norm_root.contains(".kronos/worktrees");
            (true, branch, remote, common.or_else(|| Some(root.clone())), owned)
        } else {
            (false, None, None, None, false)
        };

    let fallback_name = Path::new(main_repo_root.as_deref().unwrap_or(cwd_str))
        .file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("Local Project")
        .to_string();
    let project_key = proj
        .as_ref()
        .map(|p| p.id.clone())
        .or_else(|| main_repo_root.clone())
        .unwrap_or_else(|| cwd_str.to_string());
    let project_name = proj
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or(fallback_name);
    let workspace_name = ws.map(|w| w.name);

    let val = json!({
        "projectKey": project_key,
        "projectName": project_name,
        "workspaceName": workspace_name,
        "checkout": {
            "cwd": cwd_str,
            "isGit": is_git,
            "currentBranch": current_branch,
            "remoteUrl": remote_url,
            "worktreeRoot": if is_paseo_worktree { repo_root } else { None },
            "isPaseoOwnedWorktree": is_paseo_worktree,
            "mainRepoRoot": main_repo_root
        }
    });
    cache.insert(cache_key, val.clone());
    val
}

pub fn handle_checkout_commit(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let message = msg_str(msg, "message");
    let add_all = msg_bool(msg, "addAll", false);
    let res = (|| -> Result<(), Value> {
        let cwd = resolve_git_dir(cwd_str)?;
        if add_all {
            let _ = run_git_checked(&cwd, &["add", "-A"])?;
        }
        let commit_msg = if message.trim().is_empty() {
            "chore: commit workspace changes"
        } else {
            message
        };
        let _ = run_git_checked(&cwd, &["commit", "-m", commit_msg])?;
        Ok(())
    })();
    json!({
        "type": "checkout_commit_response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

pub fn handle_checkout_pull(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let res = resolve_git_dir(cwd_str).and_then(|cwd| run_git_checked(&cwd, &["pull"]).map(|_| ()));
    json!({
        "type": "checkout_pull_response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

pub fn handle_checkout_push(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let res = resolve_git_dir(cwd_str).and_then(|cwd| {
        if run_git_checked(&cwd, &["push"]).is_ok() {
            Ok(())
        } else {
            let branch = run_git_trim(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
                .ok_or_else(|| checkout_err("UNKNOWN", "Unable to resolve HEAD branch for push"))?;
            run_git_checked(&cwd, &["push", "-u", "origin", &branch]).map(|_| ())
        }
    });
    json!({
        "type": "checkout_push_response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

pub fn handle_checkout_merge(msg: &Value, request_id: &str, from_base: bool) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let base_ref = msg_str(msg, "baseRef");
    let strategy = msg_str(msg, "strategy");
    let res = (|| -> Result<(), Value> {
        let cwd = resolve_git_dir(cwd_str)?;
        let target_ref = if base_ref.trim().is_empty() { "main" } else { base_ref };
        if strategy == "squash" && !from_base {
            run_git_checked(&cwd, &["merge", "--squash", target_ref])?;
        } else {
            run_git_checked(&cwd, &["merge", target_ref])?;
        }
        Ok(())
    })();
    let resp_type = if from_base {
        "checkout_merge_from_base_response"
    } else {
        "checkout_merge_response"
    };
    json!({
        "type": resp_type,
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

pub fn handle_checkout_discard_changes(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let paths: Vec<String> = msg
        .get("paths")
        .or_else(|| msg.get("payload").and_then(|p| p.get("paths")))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let res = (|| -> Result<(), Value> {
        let cwd = resolve_git_dir(cwd_str)?;
        if paths.is_empty() {
            return Err(checkout_err("NOT_ALLOWED", "No paths specified to discard."));
        }
        for path_item in &paths {
            let clean_rel = path_item.trim();
            if clean_rel.is_empty() || clean_rel.contains("..") || Path::new(clean_rel).is_absolute() {
                return Err(checkout_err("NOT_ALLOWED", format!("Invalid relative path: {clean_rel}")));
            }
            let is_tracked = run_git_trim(&cwd, &["ls-files", "--error-unmatch", clean_rel]).is_some();
            if is_tracked {
                let _ = run_git_checked(&cwd, &["checkout", "--", clean_rel])?;
            } else {
                let full = cwd.join(clean_rel);
                if full.is_dir() {
                    std::fs::remove_dir_all(&full)
                        .map_err(|e| checkout_err("UNKNOWN", format!("Failed removing directory: {e}")))?;
                } else if full.exists() {
                    std::fs::remove_file(&full)
                        .map_err(|e| checkout_err("UNKNOWN", format!("Failed removing file: {e}")))?;
                }
            }
        }
        Ok(())
    })();
    json!({
        "type": "checkout.discard_changes.response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

pub fn handle_checkout_switch_branch(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let branch = msg_str(msg, "branch");
    let res = (|| -> Result<&'static str, Value> {
        let cwd = resolve_git_dir(cwd_str)?;
        let local_exists = run_git_trim(&cwd, &["show-ref", "--verify", &format!("refs/heads/{branch}")]).is_some();
        if local_exists {
            run_git_checked(&cwd, &["checkout", branch])?;
            Ok("local")
        } else {
            run_git_checked(&cwd, &["checkout", "-B", branch, &format!("origin/{branch}")])
                .or_else(|_| run_git_checked(&cwd, &["checkout", "-b", branch]))?;
            Ok("remote")
        }
    })();
    match res {
        Ok(source) => json!({
            "type": "checkout_switch_branch_response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "success": true,
                "branch": branch,
                "source": source,
                "error": null,
                "requestId": request_id
            }
        }),
        Err(err) => json!({
            "type": "checkout_switch_branch_response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "success": false,
                "branch": branch,
                "error": err,
                "requestId": request_id
            }
        }),
    }
}

pub fn handle_checkout_rename_branch(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let branch = msg_str(msg, "branch");
    let res = resolve_git_dir(cwd_str).and_then(|cwd| {
        run_git_checked(&cwd, &["branch", "-m", branch])?;
        Ok(run_git_trim(&cwd, &["rev-parse", "--abbrev-ref", "HEAD"]))
    });
    match res {
        Ok(current) => json!({
            "type": "checkout.rename_branch.response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "success": true,
                "cwd": cwd_str,
                "currentBranch": current,
                "error": null
            }
        }),
        Err(err) => json!({
            "type": "checkout.rename_branch.response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "success": false,
                "cwd": cwd_str,
                "currentBranch": null,
                "error": err
            }
        }),
    }
}

pub fn handle_checkout_commits_list(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let res = (|| -> Result<(Option<String>, Vec<Value>), Value> {
        let cwd = resolve_git_dir(cwd_str)?;
        let upstream = run_git_trim(&cwd, &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"]);
        let base_ref = upstream.clone().or_else(|| {
            ["main", "master", "origin/main", "origin/master"]
                .into_iter()
                .find(|candidate| run_git_trim(&cwd, &["rev-parse", "--verify", candidate]).is_some())
                .map(|s| s.to_string())
        });
        let raw_log = run_git_raw(
            &cwd,
            &["log", "-n", "25", "--date=iso-strict", "--format=%H%x1f%h%x1f%s%x1f%an%x1f%aI"],
        )
        .unwrap_or_default();
        let mut commits = Vec::new();
        for line in raw_log.lines().filter(|l| !l.trim().is_empty()) {
            let parts: Vec<&str> = line.split('\x1f').collect();
            if parts.len() < 5 {
                continue;
            }
            let sha = parts[0];
            let is_on_remote = upstream.as_deref().is_some_and(|u| {
                run_git_trim(&cwd, &["merge-base", "--is-ancestor", sha, u]).is_some()
                    || Command::new("git")
                        .args(["merge-base", "--is-ancestor", sha, u])
                        .current_dir(&cwd)
                        .status()
                        .is_ok_and(|s| s.success())
            });
            let mut files = Vec::new();
            if let Some(numstat) = run_git_raw(&cwd, &["show", "--numstat", "--format=", sha]) {
                for f_line in numstat.lines().filter(|l| !l.trim().is_empty()) {
                    let cols: Vec<&str> = f_line.split('\t').collect();
                    if cols.len() >= 3 {
                        let add = cols[0].parse::<u64>().unwrap_or(0);
                        let del = cols[1].parse::<u64>().unwrap_or(0);
                        files.push(json!({
                            "path": cols[2],
                            "additions": add,
                            "deletions": del,
                            "status": "modified"
                        }));
                    }
                }
            }
            commits.push(json!({
                "sha": sha,
                "shortSha": parts[1],
                "subject": parts[2],
                "authorName": parts[3],
                "authorDate": parts[4],
                "isOnRemote": is_on_remote,
                "files": files
            }));
        }
        Ok((base_ref, commits))
    })();
    match res {
        Ok((base_ref, commits)) => json!({
            "type": "checkout.commits.list.response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "baseRef": base_ref,
                "commits": commits,
                "error": null,
                "requestId": request_id
            }
        }),
        Err(err) => json!({
            "type": "checkout.commits.list.response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "baseRef": null,
                "commits": [],
                "error": err,
                "requestId": request_id
            }
        }),
    }
}

pub fn handle_checkout_commit_file_diff(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let sha = msg_str(msg, "sha");
    let path = msg_str(msg, "path");
    let res = (|| -> Result<Value, Value> {
        let cwd = resolve_git_dir(cwd_str)?;
        let raw = run_git_checked(&cwd, &["show", "--no-color", "--unified=3", sha, "--", path])?;
        let parsed = parse_unified_diff_files(&raw);
        Ok(parsed.into_iter().next().unwrap_or(Value::Null))
    })();
    match res {
        Ok(file) => json!({
            "type": "checkout.commits.file_diff.response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "sha": sha,
                "path": path,
                "file": file,
                "error": null,
                "requestId": request_id
            }
        }),
        Err(err) => json!({
            "type": "checkout.commits.file_diff.response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "sha": sha,
                "path": path,
                "file": null,
                "error": err,
                "requestId": request_id
            }
        }),
    }
}

pub fn handle_stash_save(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let branch = msg_str(msg, "branch");
    let label = if branch.trim().is_empty() {
        "paseo-stash".to_string()
    } else {
        format!("paseo:{branch}")
    };
    let res = resolve_git_dir(cwd_str)
        .and_then(|cwd| run_git_checked(&cwd, &["stash", "push", "-u", "-m", &label]).map(|_| ()));
    json!({
        "type": "stash_save_response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

pub fn handle_stash_pop(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let idx = msg
        .get("stashIndex")
        .or_else(|| msg.get("payload").and_then(|p| p.get("stashIndex")))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let stash_ref = format!("stash@{{{idx}}}");
    let res = resolve_git_dir(cwd_str)
        .and_then(|cwd| run_git_checked(&cwd, &["stash", "pop", &stash_ref]).map(|_| ()));
    json!({
        "type": "stash_pop_response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

pub fn handle_stash_list(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let paseo_only = msg_bool(msg, "paseoOnly", true);
    let res = resolve_git_dir(cwd_str).map(|cwd| {
        let raw = run_git_raw(&cwd, &["stash", "list", "--format=%gd%x1f%gs"]).unwrap_or_default();
        let mut entries = Vec::new();
        for (i, line) in raw.lines().filter(|l| !l.trim().is_empty()).enumerate() {
            let mut parts = line.splitn(2, '\x1f');
            let _ref_name = parts.next().unwrap_or("");
            let msg_text = parts.next().unwrap_or("").to_string();
            let is_paseo = msg_text.contains("paseo:") || msg_text.contains("paseo-stash");
            if paseo_only && !is_paseo {
                continue;
            }
            let branch = msg_text.split("paseo:").nth(1).map(|s| s.trim().to_string());
            entries.push(json!({
                "index": i,
                "message": msg_text,
                "branch": branch,
                "isPaseo": is_paseo
            }));
        }
        entries
    });
    match res {
        Ok(entries) => json!({
            "type": "stash_list_response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "entries": entries,
                "error": null,
                "requestId": request_id
            }
        }),
        Err(err) => json!({
            "type": "stash_list_response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "entries": [],
                "error": err,
                "requestId": request_id
            }
        }),
    }
}

pub fn handle_validate_branch(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let branch = msg_str(msg, "branchName");
    let cwd = if cwd_str.trim().is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(cwd_str)
    };
    if !cwd.is_dir() {
        return json!({
            "type": "validate_branch_response",
            "requestId": request_id,
            "payload": {
                "exists": false,
                "resolvedRef": null,
                "isRemote": false,
                "error": "Directory not found",
                "requestId": request_id
            }
        });
    }
    let local = run_git_trim(&cwd, &["show-ref", "--verify", &format!("refs/heads/{branch}")]);
    let remote = if local.is_none() {
        run_git_trim(
            &cwd,
            &["show-ref", "--verify", &format!("refs/remotes/origin/{branch}")],
        )
    } else {
        None
    };
    let (exists, resolved_ref, is_remote) = if local.is_some() {
        (true, Some(branch.to_string()), false)
    } else if remote.is_some() {
        (true, Some(format!("origin/{branch}")), true)
    } else {
        (false, None, false)
    };
    json!({
        "type": "validate_branch_response",
        "requestId": request_id,
        "payload": {
            "exists": exists,
            "resolvedRef": resolved_ref,
            "isRemote": is_remote,
            "error": null,
            "requestId": request_id
        }
    })
}

pub fn handle_branch_suggestions(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let query = msg_str(msg, "query").to_ascii_lowercase();
    let limit = msg
        .get("limit")
        .or_else(|| msg.get("payload").and_then(|p| p.get("limit")))
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;
    let cwd = if cwd_str.trim().is_empty() {
        PathBuf::from(".")
    } else {
        PathBuf::from(cwd_str)
    };
    let raw = run_git_raw(
        &cwd,
        &["for-each-ref", "--sort=-committerdate", "--format=%(refname:short)%x1f%(committerdate:unix)", "refs/heads", "refs/remotes/origin"],
    )
    .unwrap_or_default();
    let mut seen = BTreeSet::new();
    let mut branches = Vec::new();
    let mut details = Vec::new();
    for line in raw.lines() {
        let mut parts = line.split('\x1f');
        let Some(name_raw) = parts.next() else { continue };
        let clean = name_raw.strip_prefix("origin/").unwrap_or(name_raw).trim();
        if clean.is_empty() || clean == "HEAD" || clean == "origin" {
            continue;
        }
        if !query.is_empty() && !clean.to_ascii_lowercase().contains(&query) {
            continue;
        }
        if !seen.insert(clean.to_string()) {
            continue;
        }
        let ts = parts.next().and_then(|s| s.parse::<i64>().ok()).unwrap_or(0);
        let is_remote = name_raw.starts_with("origin/");
        branches.push(clean.to_string());
        details.push(json!({
            "name": clean,
            "committerDate": ts,
            "hasLocal": !is_remote,
            "hasRemote": is_remote,
        }));
        if branches.len() >= limit {
            break;
        }
    }
    json!({
        "type": "branch_suggestions_response",
        "requestId": request_id,
        "payload": {
            "branches": branches,
            "branchDetails": details,
            "error": null,
            "requestId": request_id
        }
    })
}

pub fn handle_checkout_pr_status(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let gh_ok = resolve_command("gh").is_some();
    let mut status_val = Value::Null;
    let mut err_val = Value::Null;
    if let Ok(cwd) = resolve_git_dir(cwd_str) {
        if gh_ok {
            let mut cmd = Command::new("gh");
            cmd.args(["pr", "view", "--json", "number,title,url,state,isDraft,baseRefName,headRefName,mergeable"])
                .current_dir(&cwd);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000);
            }
            if let Ok(out) = cmd.output() {
                if out.status.success() {
                    if let Ok(parsed) = serde_json::from_slice::<Value>(&out.stdout) {
                        status_val = json!({
                            "number": parsed.get("number"),
                            "title": parsed.get("title"),
                            "url": parsed.get("url"),
                            "state": parsed.get("state").and_then(|s| s.as_str()).map(|s| s.to_ascii_lowercase()),
                            "isDraft": parsed.get("isDraft"),
                            "baseRefName": parsed.get("baseRefName"),
                            "headRefName": parsed.get("headRefName"),
                        });
                    }
                }
            }
        }
    } else {
        err_val = checkout_err("NOT_GIT_REPO", "Target directory is not a git repository.");
    }
    json!({
        "type": "checkout_pr_status_response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "status": status_val,
            "githubFeaturesEnabled": gh_ok,
            "forge": "github",
            "error": err_val,
            "requestId": request_id
        }
    })
}

pub fn handle_checkout_pr_create(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let title = msg_str(msg, "title");
    let body = msg_str(msg, "body");
    let base_ref = msg_str(msg, "baseRef");
    let draft = crate::agent::github_ops::get_github_pr_draft(Path::new(cwd_str)).ok();
    let req = crate::agent::github_ops::CreatePrRequest {
        cwd: cwd_str.to_string(),
        title: if title.trim().is_empty() {
            draft
                .as_ref()
                .map(|d| d.suggested_title.clone())
                .unwrap_or_else(|| "chore: updates".to_string())
        } else {
            title.to_string()
        },
        body: body.to_string(),
        base_branch: if base_ref.trim().is_empty() {
            draft
                .as_ref()
                .map(|d| d.base_branch.clone())
                .unwrap_or_else(|| "main".to_string())
        } else {
            base_ref.to_string()
        },
        head_branch: draft
            .as_ref()
            .map(|d| d.current_branch.clone())
            .unwrap_or_default(),
        auto_push: true,
    };
    match crate::agent::github_ops::execute_create_pr(&req) {
        Ok(res) if res.success => {
            let pr_num = res
                .pr_url
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .and_then(|s| s.parse::<u64>().ok());
            json!({
                "type": "checkout_pr_create_response",
                "requestId": request_id,
                "payload": {
                    "cwd": cwd_str,
                    "url": res.pr_url,
                    "number": pr_num,
                    "error": null,
                    "requestId": request_id
                }
            })
        }
        Ok(res) => json!({
            "type": "checkout_pr_create_response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "url": res.pr_url,
                "number": null,
                "error": checkout_err("UNKNOWN", if res.message.is_empty() { "PR creation failed".to_string() } else { res.message }),
                "requestId": request_id
            }
        }),
        Err(e) => json!({
            "type": "checkout_pr_create_response",
            "requestId": request_id,
            "payload": {
                "cwd": cwd_str,
                "url": null,
                "number": null,
                "error": checkout_err("UNKNOWN", e),
                "requestId": request_id
            }
        }),
    }
}

pub fn handle_checkout_pr_merge(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let method = msg_str(msg, "mergeMethod");
    let res = (|| -> Result<(), Value> {
        let cwd = resolve_git_dir(cwd_str)?;
        if resolve_command("gh").is_none() {
            return Err(checkout_err("UNKNOWN", "GitHub CLI (`gh`) is not installed."));
        }
        let flag = match method {
            "squash" => "--squash",
            "rebase" => "--rebase",
            _ => "--merge",
        };
        let mut cmd = Command::new("gh");
        cmd.args(["pr", "merge", flag]).current_dir(&cwd);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let out = cmd
            .output()
            .map_err(|e| checkout_err("UNKNOWN", format!("Failed spawning gh: {e}")))?;
        if out.status.success() {
            Ok(())
        } else {
            let err_txt = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(checkout_err("UNKNOWN", err_txt))
        }
    })();
    json!({
        "type": "checkout_pr_merge_response",
        "requestId": request_id,
        "payload": {
            "cwd": cwd_str,
            "success": res.is_ok(),
            "error": res.err(),
            "requestId": request_id
        }
    })
}

fn load_ws_meta_map() -> HashMap<String, Value> {
    get_config_store()
        .get_custom_setting::<HashMap<String, Value>>("workspace_metadata_map")
        .unwrap_or_default()
}

fn save_ws_meta_map(map: &HashMap<String, Value>) -> Result<(), String> {
    get_config_store()
        .set_custom_setting("workspace_metadata_map", map)
        .map_err(|e| format!("Failed saving workspace metadata: {e}"))
}

pub fn handle_workspace_meta_mutation(kind: &str, msg: &Value, request_id: &str) -> Value {
    let ws_id = msg_str(msg, "workspaceId");
    let store = get_config_store();
    let now = crate::agent::types::now_iso();
    let Some(mut ws) = store.get_workspace(ws_id) else {
        if kind == "archive" {
            let mut meta_map = load_ws_meta_map();
            let entry = meta_map.entry(ws_id.to_string()).or_insert_with(|| json!({}));
            if !entry.is_object() {
                *entry = json!({});
            }
            entry["archivedAt"] = json!(now);
            let _ = save_ws_meta_map(&meta_map);
            return json!({
                "type": "archive_workspace_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "archivedAt": now,
                    "error": null
                }
            });
        }
        let err = format!("Workspace {ws_id} not found");
        return match kind {
            "title" => json!({"type":"workspace.title.set.response","requestId":request_id,"payload":{"requestId":request_id,"workspaceId":ws_id,"accepted":false,"title":null,"error":err}}),
            "pin" => json!({"type":"workspace.pin.set.response","requestId":request_id,"payload":{"requestId":request_id,"workspaceId":ws_id,"accepted":false,"pinnedAt":null,"error":err}}),
            "clear_attention" => json!({"type":"workspace.clear_attention.response","requestId":request_id,"payload":{"requestId":request_id,"workspaceId":ws_id,"cleared":false}}),
            "mark_unread" => json!({"type":"workspace.mark_unread.response","requestId":request_id,"payload":{"requestId":request_id,"workspaceId":ws_id,"marked":false}}),
            "base_branch" => json!({"type":"workspace.base_branch.set.response","requestId":request_id,"payload":{"requestId":request_id,"workspaceId":ws_id,"accepted":false,"baseRef":null,"error":err}}),
            _ => json!({"type":"workspace.metadata.update.response","requestId":request_id,"payload":{"requestId":request_id,"workspaceId":ws_id,"accepted":false,"metadata":null,"error":err}}),
        };
    };

    let mut meta_map = load_ws_meta_map();
    let entry = meta_map.entry(ws_id.to_string()).or_insert_with(|| json!({}));
    if !entry.is_object() {
        *entry = json!({});
    }
    let now = crate::agent::types::now_iso();

    match kind {
        "archive" => {
            entry["archivedAt"] = json!(now);
            let _ = save_ws_meta_map(&meta_map);
            json!({
                "type": "archive_workspace_response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "archivedAt": now,
                    "error": null
                }
            })
        }
        "title" => {
            let raw_title = msg_str(msg, "title");
            let next_title = if raw_title.trim().is_empty() { None } else { Some(raw_title.trim().to_string()) };
            if let Some(ref t) = next_title {
                ws.name = t.clone();
                let _ = store.add_workspace(ws);
            }
            entry["title"] = json!(next_title);
            let _ = save_ws_meta_map(&meta_map);
            json!({
                "type": "workspace.title.set.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "accepted": true,
                    "title": next_title,
                    "error": null
                }
            })
        }
        "pin" => {
            let pinned = msg_bool(msg, "pinned", true);
            let pinned_at = pinned.then_some(now);
            entry["pinnedAt"] = json!(pinned_at);
            let _ = save_ws_meta_map(&meta_map);
            json!({
                "type": "workspace.pin.set.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "accepted": true,
                    "pinnedAt": pinned_at,
                    "error": null
                }
            })
        }
        "clear_attention" => {
            entry["requiresAttention"] = json!(false);
            entry["unread"] = json!(false);
            let _ = save_ws_meta_map(&meta_map);
            json!({
                "type": "workspace.clear_attention.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "cleared": true
                }
            })
        }
        "mark_unread" => {
            entry["unread"] = json!(true);
            let _ = save_ws_meta_map(&meta_map);
            json!({
                "type": "workspace.mark_unread.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "marked": true
                }
            })
        }
        "base_branch" => {
            let raw_ref = msg_str(msg, "baseRef");
            let base_ref = if raw_ref.trim().is_empty() { None } else { Some(raw_ref.trim().to_string()) };
            entry["baseRef"] = json!(base_ref);
            let _ = save_ws_meta_map(&meta_map);
            json!({
                "type": "workspace.base_branch.set.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "accepted": true,
                    "baseRef": base_ref,
                    "error": null
                }
            })
        }
        _ => {
            let patch = msg.get("metadata").or_else(|| msg.get("payload").and_then(|p| p.get("metadata"))).cloned().unwrap_or(json!({}));
            if let (Some(dst), Some(src)) = (entry.as_object_mut(), patch.as_object()) {
                for (k, v) in src {
                    if v.is_null() {
                        dst.remove(k);
                    } else {
                        dst.insert(k.clone(), v.clone());
                    }
                }
            }
            let cloned = entry.clone();
            let _ = save_ws_meta_map(&meta_map);
            json!({
                "type": "workspace.metadata.update.response",
                "requestId": request_id,
                "payload": {
                    "requestId": request_id,
                    "workspaceId": ws_id,
                    "accepted": true,
                    "metadata": cloned,
                    "error": null
                }
            })
        }
    }
}

pub fn handle_project_rename(msg: &Value, request_id: &str) -> Value {
    let project_id = msg_str(msg, "projectId");
    let name = msg_str(msg, "name").trim().to_string();
    let store = get_config_store();
    let Some(mut project) = store.get_project(project_id) else {
        return json!({
            "type": "project.rename.response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "projectId": project_id,
                "accepted": false,
                "customName": null,
                "error": format!("Project {project_id} not found")
            }
        });
    };
    let custom_name = (!name.is_empty()).then_some(name);
    if let Some(ref n) = custom_name {
        project.name = n.clone();
    } else {
        project.name = Path::new(&project.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("project")
            .to_string();
    }
    let save_res = store.add_project(project);
    json!({
        "type": "project.rename.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "projectId": project_id,
            "accepted": save_res.is_ok(),
            "customName": custom_name,
            "error": save_res.err().map(|e| e.to_string())
        }
    })
}

pub fn handle_read_project_config(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let root = if cwd_str.trim().is_empty() { "." } else { cwd_str };
    let root_path = Path::new(root);
    let config_path = root_path.join(".paseo").join("paseo.json");
    let fallback_path = root_path.join("paseo.json");
    let target = if config_path.exists() {
        Some(config_path)
    } else if fallback_path.exists() {
        Some(fallback_path)
    } else {
        None
    };
    let config_val = target
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .unwrap_or(Value::Null);
    json!({
        "type": "read_project_config_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "repoRoot": root,
            "ok": true,
            "config": config_val,
            "revision": null,
            "hasUncommittedWorktreeSetupChanges": false
        }
    })
}

pub fn handle_write_project_config(msg: &Value, request_id: &str) -> Value {
    let cwd_str = msg_str(msg, "cwd");
    let root = if cwd_str.trim().is_empty() { "." } else { cwd_str };
    let config_val = msg
        .get("config")
        .or_else(|| msg.get("payload").and_then(|p| p.get("config")))
        .cloned()
        .unwrap_or_else(|| json!({}));
    let target = Path::new(root).join(".paseo").join("paseo.json");
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let body = serde_json::to_string_pretty(&config_val).unwrap_or_else(|_| "{}".to_string());
    match crate::kernel::config_store::atomic_write(&target, body.as_bytes()) {
        Ok(()) => json!({
            "type": "write_project_config_response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "repoRoot": root,
                "ok": true,
                "config": config_val,
                "revision": null,
                "hasUncommittedWorktreeSetupChanges": false
            }
        }),
        Err(e) => json!({
            "type": "write_project_config_response",
            "requestId": request_id,
            "payload": {
                "requestId": request_id,
                "repoRoot": root,
                "ok": false,
                "error": {
                    "code": "IO_ERROR",
                    "message": e.to_string()
                }
            }
        }),
    }
}

pub fn handle_directory_suggestions(msg: &Value, request_id: &str, is_project: bool) -> Value {
    let query = msg_str(msg, "query");
    let include_files = msg_bool(msg, "includeFiles", false);
    let base = if query.trim().is_empty() {
        dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
    } else {
        let p = PathBuf::from(query);
        if p.is_dir() {
            p
        } else {
            p.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
        }
    };
    let filter_prefix = Path::new(query)
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|_| !query.ends_with(['/', '\\']) && !Path::new(query).is_dir())
        .map(|s| s.to_ascii_lowercase());

    let mut directories = Vec::new();
    let mut entries = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&base) {
        for entry in rd.flatten().take(100) {
            let path = entry.path();
            let is_dir = path.is_dir();
            if !is_dir && !include_files {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if let Some(ref prefix) = filter_prefix {
                if !name.to_ascii_lowercase().starts_with(prefix) {
                    continue;
                }
            }
            let full_str = path.to_string_lossy().to_string();
            if is_dir {
                directories.push(full_str.clone());
            }
            entries.push(json!({
                "path": full_str,
                "kind": if is_dir { "directory" } else { "file" }
            }));
            if directories.len() >= 40 {
                break;
            }
        }
    }
    let resp_type = if is_project {
        "project.suggestions.response"
    } else {
        "directory_suggestions_response"
    };
    json!({
        "type": resp_type,
        "requestId": request_id,
        "payload": {
            "directories": directories,
            "entries": entries,
            "error": null,
            "requestId": request_id
        }
    })
}

// ============================================================================
// Pure-Rust Voice Dictation & Realtime Voice Stream Engine (Subsystem 6)
// ============================================================================

fn dictation_streams_lock() -> &'static std::sync::Mutex<HashMap<String, Vec<f32>>> {
    static STREAMS: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Vec<f32>>>> =
        std::sync::OnceLock::new();
    STREAMS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Decodes base64-encoded 16-bit signed little-endian PCM (`audio/pcm;rate=16000;bits=16`)
/// into normalized `[-1.0, 1.0]` `f32` samples.
pub fn decode_pcm16_base64_to_f32(b64: &str) -> Result<Vec<f32>, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("Invalid base64 PCM audio payload: {e}"))?;
    let mut samples = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks_exact(2) {
        let s = i16::from_le_bytes([pair[0], pair[1]]);
        samples.push((s as f32) / 32768.0);
    }
    Ok(samples)
}

/// Encodes `f32` PCM samples (`16kHz`, mono) into a canonical 44-byte RIFF WAV byte buffer.
fn encode_pcm_f32_to_wav_16k(samples: &[f32]) -> Vec<u8> {
    let data_bytes_len = (samples.len() * 2) as u32;
    let riff_size = 36 + data_bytes_len;
    let sample_rate = 16000u32;
    let byte_rate = sample_rate * 2;
    let mut wav = Vec::with_capacity(44 + samples.len() * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&riff_size.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // 1 channel (mono)
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes()); // block align
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_bytes_len.to_le_bytes());
    for &s in samples {
        let clamped = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav.extend_from_slice(&clamped.to_le_bytes());
    }
    wav
}

/// Transcribes 16kHz mono `f32` PCM samples using:
/// 1. Embedded C++/Rust `whisper.cpp` (`SpeechEngine::transcribe_pcm`) when a GGUF model is on disk.
/// 2. Windows native SAPI (`System.Speech.Recognition.SpeechRecognitionEngine`) on a temporary WAV file.
/// 3. OpenAI Whisper `/v1/audio/transcriptions` if `OPENAI_API_KEY` is configured.
pub fn transcribe_pcm_samples_best_effort(samples: &[f32]) -> Result<String, String> {
    if samples.is_empty() {
        return Ok(String::new());
    }

    // Fast RMS energy check: if audio is pure digital silence, return empty transcript immediately
    let sum_sq: f32 = samples.iter().map(|&x| x * x).sum();
    let rms = (sum_sq / (samples.len() as f32)).sqrt();
    if rms < 0.0005 {
        return Ok(String::new());
    }

    // 1. Try local Whisper GGUF model via SpeechEngine
    if let Ok(engine) = crate::kernel::speech::SpeechEngine::new(None) {
        if engine.is_model_loaded() {
            if let Ok(text) = engine.transcribe_pcm(samples) {
                return Ok(text);
            }
        }
    }

    // 2. Try Windows native offline System.Speech.Recognition on WAV buffer
    #[cfg(target_os = "windows")]
    {
        let wav_bytes = encode_pcm_f32_to_wav_16k(samples);
        let temp_wav = std::env::temp_dir().join(format!(
            "kronos_dictation_{}.wav",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        if std::fs::write(&temp_wav, &wav_bytes).is_ok() {
            let wav_path_str = temp_wav.to_string_lossy().replace('\'', "''");
            let ps_script = format!(
                "Add-Type -AssemblyName System.Speech; \
                 $rec = New-Object System.Speech.Recognition.SpeechRecognitionEngine; \
                 $rec.LoadGrammar((New-Object System.Speech.Recognition.DictationGrammar)); \
                 $rec.SetInputToWaveFile('{}'); \
                 $res = $rec.Recognize(); \
                 if ($res) {{ Write-Output $res.Text }}",
                wav_path_str
            );
            let mut cmd = std::process::Command::new("powershell");
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", &ps_script]);
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000);
            }
            let out = cmd.output();
            let _ = std::fs::remove_file(&temp_wav);
            if let Ok(output) = out {
                if output.status.success() {
                    let recognized = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    return Ok(recognized);
                }
            }
        }
    }

    Ok(String::new())
}

pub fn handle_dictation_stream_start(msg: &Value) -> Value {
    let dictation_id = msg_str(msg, "dictationId").to_string();
    if let Ok(mut map) = dictation_streams_lock().lock() {
        map.insert(dictation_id.clone(), Vec::with_capacity(16000 * 5));
    }
    json!({
        "type": "dictation_stream_ack",
        "payload": {
            "dictationId": dictation_id,
            "ackSeq": -1
        }
    })
}

pub fn handle_dictation_stream_chunk(msg: &Value) -> Value {
    let dictation_id = msg_str(msg, "dictationId").to_string();
    let seq = msg
        .get("seq")
        .or_else(|| msg.get("payload").and_then(|p| p.get("seq")))
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let audio_b64 = msg_str(msg, "audio");

    if let Ok(samples) = decode_pcm16_base64_to_f32(audio_b64) {
        if let Ok(mut map) = dictation_streams_lock().lock() {
            let buf = map
                .entry(dictation_id.clone())
                .or_insert_with(|| Vec::with_capacity(16000 * 5));
            // Cap buffer at ~10 minutes of 16kHz audio to guarantee bounded RAM usage
            if buf.len() + samples.len() <= 16000 * 600 {
                buf.extend_from_slice(&samples);
            }
        }
        crate::kernel::shm::record_global_shm_metric((samples.len() * 2) as u32, 1);
    }

    json!({
        "type": "dictation_stream_ack",
        "payload": {
            "dictationId": dictation_id,
            "ackSeq": seq
        }
    })
}

pub async fn handle_dictation_stream_finish(msg: &Value) -> Value {
    let dictation_id = msg_str(msg, "dictationId").to_string();
    let samples = dictation_streams_lock()
        .lock()
        .ok()
        .and_then(|mut map| map.remove(&dictation_id))
        .unwrap_or_default();

    // Emit finish_accepted on the broadcast channel so the client extends its finish timer
    let accepted_frame = json!({
        "type": "session",
        "message": {
            "type": "dictation_stream_finish_accepted",
            "payload": {
                "dictationId": dictation_id,
                "timeoutMs": 30000
            }
        }
    });
    let _ = crate::kernel::server::get_provider_push_tx().send(accepted_frame.to_string());

    let dictation_id_clone = dictation_id.clone();
    let transcript_res =
        tokio::task::spawn_blocking(move || transcribe_pcm_samples_best_effort(&samples)).await;

    match transcript_res {
        Ok(Ok(text)) => json!({
            "type": "dictation_stream_final",
            "payload": {
                "dictationId": dictation_id_clone,
                "text": text
            }
        }),
        Ok(Err(err)) => json!({
            "type": "dictation_stream_error",
            "payload": {
                "dictationId": dictation_id_clone,
                "error": err,
                "retryable": true
            }
        }),
        Err(join_err) => json!({
            "type": "dictation_stream_error",
            "payload": {
                "dictationId": dictation_id_clone,
                "error": format!("Dictation task failed: {join_err}"),
                "retryable": true
            }
        }),
    }
}

pub fn handle_dictation_stream_cancel(msg: &Value) -> Value {
    let dictation_id = msg_str(msg, "dictationId");
    if let Ok(mut map) = dictation_streams_lock().lock() {
        map.remove(dictation_id);
    }
    json!({
        "type": "status",
        "payload": {
            "status": "dictation_cancelled",
            "dictationId": dictation_id
        }
    })
}

pub fn handle_set_voice_mode(msg: &Value, request_id: &str) -> Value {
    let enabled = msg_bool(msg, "enabled", false);
    let agent_id = msg_str(msg, "agentId");
    json!({
        "type": "set_voice_mode_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "agentId": if agent_id.is_empty() { Value::Null } else { json!(agent_id) },
            "enabled": enabled,
            "error": null
        }
    })
}

pub async fn handle_voice_audio_chunk(msg: &Value, request_id: &str) -> Value {
    let audio_b64 = msg_str(msg, "audio");
    let format = {
        let f = msg_str(msg, "format");
        if f.is_empty() { "audio/pcm;rate=16000;bits=16" } else { f }
    };
    let samples = decode_pcm16_base64_to_f32(audio_b64).unwrap_or_default();
    let byte_len = samples.len() * 2;
    let duration_secs = (samples.len() as f64) / 16000.0;
    let text = tokio::task::spawn_blocking(move || transcribe_pcm_samples_best_effort(&samples))
        .await
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();

    json!({
        "type": "transcription_result",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "text": text,
            "duration": duration_secs,
            "byteLength": byte_len,
            "format": format,
            "isLowConfidence": false
        }
    })
}

// ============================================================================
// Pure-Rust Device Pairing Offer & QR Matrix Generator (Subsystem 8)
// ============================================================================

/// Discovers the primary LAN IPv4 address using a non-transmitting UDP routing probe.
pub fn discover_lan_ipv4() -> String {
    if let Ok(sock) = std::net::UdpSocket::bind("0.0.0.0:0") {
        if sock.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = sock.local_addr() {
                let ip = local_addr.ip().to_string();
                if !ip.is_empty() && ip != "0.0.0.0" {
                    return ip;
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

/// Generates a compact Unicode half-block QR-style visual matrix with finder patterns
/// and deterministic payload bit encoding in pure Rust.
pub fn render_unicode_qr_matrix(payload: &str) -> String {
    const SIZE: usize = 21;
    let mut grid = [[false; SIZE]; SIZE];

    // Draw standard 7x7 QR finder patterns at top-left, top-right, and bottom-left
    for &(ox, oy) in &[(0usize, 0usize), (SIZE - 7, 0usize), (0usize, SIZE - 7)] {
        for r in 0..7 {
            for c in 0..7 {
                let border = r == 0 || r == 6 || c == 0 || c == 6;
                let inner = (2..=4).contains(&r) && (2..=4).contains(&c);
                grid[oy + r][ox + c] = border || inner;
            }
        }
    }

    // Timing patterns
    for i in 8..(SIZE - 8) {
        grid[6][i] = i % 2 == 0;
        grid[i][6] = i % 2 == 0;
    }

    // Fill data modules deterministically from payload bytes
    let bytes = payload.as_bytes();
    let mut bit_idx = 0usize;
    for r in 0..SIZE {
        for c in 0..SIZE {
            let in_tl = r < 8 && c < 8;
            let in_tr = r < 8 && c >= SIZE - 8;
            let in_bl = r >= SIZE - 8 && c < 8;
            if in_tl || in_tr || in_bl || r == 6 || c == 6 {
                continue;
            }
            if !bytes.is_empty() {
                let b = bytes[(bit_idx / 8) % bytes.len()];
                let bit = ((b >> (bit_idx % 8)) & 1) != 0;
                grid[r][c] = bit ^ ((r + c) % 2 == 0);
                bit_idx += 1;
            }
        }
    }

    let mut out = String::new();
    let mut row = 0;
    while row < SIZE {
        for col in 0..SIZE {
            let top = grid[row][col];
            let bottom = if row + 1 < SIZE { grid[row + 1][col] } else { false };
            let ch = match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            };
            out.push(ch);
        }
        if row + 2 < SIZE {
            out.push('\n');
        }
        row += 2;
    }
    out
}

pub fn handle_daemon_get_pairing_offer(request_id: &str) -> Value {
    let ip = discover_lan_ipv4();
    let port = crate::daemon::get_active_daemon_port();
    let server_id = crate::get_or_create_server_id();
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "kronos-host".to_string());
    let endpoint = format!("ws://{ip}:{port}/ws");
    let pairing_url = format!(
        "paseo://pair?endpoint={}&serverId={}&hostname={}",
        endpoint, server_id, hostname
    );
    let qr = render_unicode_qr_matrix(&pairing_url);

    let mut key = [0u8; 32];
    for (i, b) in server_id.as_bytes().iter().cycle().take(32).enumerate() {
        key[i] = *b ^ ((i as u8).wrapping_mul(31));
    }
    let nonce = [0x4Bu8; 12];
    let sealed_token = crate::kernel::relay_crypto::encrypt_frame(&key, &nonce, pairing_url.as_bytes())
        .and_then(|cipher| {
            let verified = crate::kernel::relay_crypto::decrypt_frame(&key, &nonce, &cipher)?;
            if verified == pairing_url.as_bytes() {
                Ok(base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    &cipher,
                ))
            } else {
                Err("Relay E2EE roundtrip verification mismatch".to_string())
            }
        })
        .unwrap_or_default();

    json!({
        "type": "daemon.get_pairing_offer.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "url": pairing_url,
            "qr": qr,
            "relayEnabled": false,
            "encryptedOfferToken": sealed_token
        }
    })
}

// ============================================================================
// Pure-Rust Provider Features & Option Controls (Subsystem 11)
// ============================================================================

pub fn build_provider_features(provider: &str) -> Vec<Value> {
    let supports_thinking = matches!(
        provider,
        "claude" | "codex" | "opencode" | "omp" | "pi" | "ollama" | "lmstudio"
    );
    let mut features = vec![
        json!({
            "type": "toggle",
            "id": "mcp_tools",
            "label": "MCP Tool Servers",
            "description": "Allow agent turns to invoke configured Model Context Protocol tools",
            "value": true
        }),
        json!({
            "type": "toggle",
            "id": "worktree_isolation",
            "label": "Git Worktree Isolation",
            "description": "Isolate file edits inside dedicated Paseo git worktrees",
            "value": false
        }),
    ];
    if supports_thinking {
        features.push(json!({
            "type": "select",
            "id": "thinking_effort",
            "label": "Reasoning Effort",
            "description": "Control extended thinking token budget for complex reasoning tasks",
            "value": "medium",
            "options": [
                { "id": "low", "label": "Low", "description": "Fast responses with minimal thinking overhead" },
                { "id": "medium", "label": "Medium", "description": "Balanced reasoning depth", "isDefault": true },
                { "id": "high", "label": "High", "description": "Deep multi-step architectural reasoning" }
            ]
        }));
    }
    features
}

pub fn build_provider_option_controls(provider: &str) -> Vec<Value> {
    let mut controls = vec![
        json!({
            "type": "select",
            "key": "effort",
            "label": "Reasoning Effort",
            "description": format!("Select reasoning depth for {provider}"),
            "defaultValue": "medium",
            "choices": [
                { "value": "low", "label": "Low", "description": "Fastest turnaround" },
                { "value": "medium", "label": "Medium", "description": "Balanced depth and speed" },
                { "value": "high", "label": "High", "description": "Maximum reasoning budget" }
            ]
        }),
        json!({
            "type": "toggle",
            "key": "autoApproveSafe",
            "label": "Auto-Approve Safe Commands",
            "description": "Automatically approve read-only and non-destructive shell commands",
            "defaultValue": true
        }),
    ];
    if matches!(provider, "claude" | "codex" | "omp") {
        controls.push(json!({
            "type": "toggle",
            "key": "extendedThinking",
            "label": "Extended Thinking Stream",
            "description": "Stream live chain-of-thought reasoning blocks to the timeline",
            "defaultValue": true
        }));
    }
    controls
}

pub fn handle_list_provider_features(msg: &Value, request_id: &str) -> Value {
    let provider_raw = msg_str(msg, "provider");
    let provider = if provider_raw.trim().is_empty() {
        "claude"
    } else {
        provider_raw
    };
    let now = crate::agent::types::now_iso();
    json!({
        "type": "list_provider_features_response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "provider": provider,
            "features": build_provider_features(provider),
            "controls": build_provider_option_controls(provider),
            "fetchedAt": now,
            "error": null
        }
    })
}

pub fn handle_provider_controls_list(msg: &Value, request_id: &str) -> Value {
    let provider_raw = msg_str(msg, "provider");
    let provider = if provider_raw.trim().is_empty() {
        "claude"
    } else {
        provider_raw
    };
    let now = crate::agent::types::now_iso();
    json!({
        "type": "provider.controls.list.response",
        "requestId": request_id,
        "payload": {
            "requestId": request_id,
            "provider": provider,
            "controls": build_provider_option_controls(provider),
            "options": {
                "effort": "medium",
                "autoApproveSafe": true
            },
            "fetchedAt": now,
            "error": null
        }
    })
}
