use std::ffi::OsString;

#[cfg(feature = "ipc-acceptance")]
pub(crate) mod supervisor;
#[cfg(feature = "ipc-acceptance")]
pub(crate) mod commands;
#[cfg(feature = "ipc-acceptance")]
pub(crate) mod preflight;

pub enum BootAction {
    Continue,
    #[cfg(feature = "ipc-acceptance")]
    Exit(i32),
}

#[cfg(not(feature = "ipc-acceptance"))]
pub fn bootstrap(args: &[OsString]) -> Result<BootAction, String> {
    if args
        .iter()
        .skip(1)
        .any(|arg| arg.to_string_lossy().starts_with("--ipc-acceptance"))
    {
        return Err("This build cannot run IPC acceptance invocations".into());
    }
    Ok(BootAction::Continue)
}

#[cfg(feature = "ipc-acceptance")]
pub fn bootstrap(args: &[OsString]) -> Result<BootAction, String> {
    if args.len() == 2 && args[1] == "--ipc-acceptance-info" {
        crate::println!(
            "{}",
            serde_json::json!({"ipcAcceptance": true, "schemaVersion": 1})
        );
        return Ok(BootAction::Exit(0));
    }
    if args.len() == 3 && (args[1] == "--ipc-acceptance-run" || args[1] == "--ipc-acceptance-child")
    {
        let context = AcceptanceContext::from_descriptor(std::path::Path::new(&args[2]))?;
        context.validate_environment(
            std::env::var_os("KRONOS_HOME").as_deref(),
            std::env::var_os("PASEO_HOME").as_deref(),
        )?;
        if args[1] == "--ipc-acceptance-child" {
            // Ownership is established by the supervisor before the handshake releases us;
            // owned resources and the context must exist before any startup global initializes.
            supervisor::child_handshake(&context)?;
            context.claim_resources()?;
            install_context(context)?;
            return Ok(BootAction::Continue);
        }
        let mut command =
            tokio::process::Command::new(std::env::current_exe().map_err(|e| e.to_string())?);
        command
            .arg("--ipc-acceptance-child")
            .arg(&args[2])
            .current_dir(context.resource_plan().workspace);
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?;
        let exit = runtime.block_on(supervisor::supervise(
            &mut command,
            &context,
            std::time::Duration::from_secs(420),
        ))?;
        return Ok(BootAction::Exit(exit.exit_code));
    }
    Err("IPC acceptance requires a validated supervised run; startup is unavailable".into())
}

pub fn enter(args: &[OsString], _run: impl FnOnce()) -> Result<i32, String> {
    match bootstrap(args)? {
        BootAction::Continue => {
            _run();
            Ok(0)
        }
        #[cfg(feature = "ipc-acceptance")]
        BootAction::Exit(code) => Ok(code),
    }
}

#[cfg(feature = "ipc-acceptance")]
pub use policy::{AcceptanceCase, AcceptanceContext, AcceptanceTransport, RunClaim};

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn install_context(context: AcceptanceContext) -> Result<(), String> {
    let resources = RESOURCES
        .get()
        .ok_or("Acceptance resources must be claimed before context installation")?;
    if resources.run_id != context.run_id {
        return Err("Acceptance resource owner mismatch".into());
    }
    CONTEXT
        .set(context)
        .map_err(|_| "Acceptance context was already installed".into())
}

#[cfg(feature = "ipc-acceptance")]
static CONTEXT: std::sync::OnceLock<AcceptanceContext> = std::sync::OnceLock::new();
#[cfg(feature = "ipc-acceptance")]
static RESOURCES: std::sync::OnceLock<OwnedResources> = std::sync::OnceLock::new();
#[cfg(feature = "ipc-acceptance")]
struct OwnedResources {
    run_id: String,
    daemon: std::sync::Arc<tokio::sync::Mutex<crate::kernel::shm::ShmRingBuffer>>,
    telemetry: crate::kernel::shm::ShmRingBuffer,
    scenario: String,
    providers: std::sync::Mutex<commands::ProviderMeasurements>,
    #[cfg(windows)]
    pipe: std::sync::Mutex<Option<crate::kernel::server::PipeLease>>,
}

#[cfg(feature = "ipc-acceptance")]
fn claim_owned_resources(context: &AcceptanceContext) -> Result<(), String> {
    if RESOURCES.get().is_some() || CONTEXT.get().is_some() {
        return Err("Acceptance resources already claimed".into());
    }
    let scenario = context.scenario_script()?;
    let plan = context.resource_plan();
    let daemon =
        crate::kernel::shm::ShmRingBuffer::new_exclusive(&plan.daemon_mapping, 2 * 1024 * 1024)
            .map_err(|e| e.to_string())?;
    let telemetry =
        crate::kernel::shm::ShmRingBuffer::new_exclusive(&plan.telemetry_mapping, 64 * 1024)
            .map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let pipe = crate::kernel::server::PipeLease::claim(&plan.pipe).map_err(|e| e.to_string())?;
    RESOURCES
        .set(OwnedResources {
            run_id: context.run_id.clone(),
            daemon: std::sync::Arc::new(tokio::sync::Mutex::new(daemon)),
            telemetry,
            scenario,
            providers: std::sync::Mutex::new(commands::ProviderMeasurements::default()),
            #[cfg(windows)]
            pipe: std::sync::Mutex::new(Some(pipe)),
        })
        .map_err(|_| "Acceptance resource handoff raced".into())
}

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn record_provider_start(pid: Option<u32>) {
    if let Some(context) = active() {
        if let Some(resources) = RESOURCES
            .get()
            .filter(|resources| resources.run_id == context.run_id)
        {
            // Diagnostic failure must not alter provider execution or process ownership.
            if let Ok(mut measurements) = resources.providers.lock() {
                measurements.record(pid);
            }
        }
    }
}

#[cfg(feature = "ipc-acceptance")]
fn start_acceptance_surfaces(
    case: &AcceptanceCase,
    start: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    if case == &AcceptanceCase::Preflight {
        Ok(())
    } else {
        start()
    }
}

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn acceptance_preflight_startup(app: tauri::AppHandle) -> Result<(), String> {
    match active() {
        Some(context) if context.case_id == AcceptanceCase::Preflight => {
            preflight::start(app);
            Ok(())
        }
        Some(context) => {
            start_acceptance_surfaces(&context.case_id, || commands::start(&app, false))
        }
        None => Ok(()),
    }
}

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn provider_observations() -> Result<commands::ProviderObservation, String> {
    let context = active().ok_or("Acceptance context unavailable")?;
    let resources = RESOURCES
        .get()
        .filter(|resources| resources.run_id == context.run_id)
        .ok_or("Acceptance provider observation owner mismatch")?;
    resources
        .providers
        .lock()
        .map_err(|_| "Native provider observations poisoned")?
        .snapshot()
}

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn owned_daemon_ring(
) -> std::sync::Arc<tokio::sync::Mutex<crate::kernel::shm::ShmRingBuffer>> {
    RESOURCES
        .get()
        .expect("active acceptance owns resources")
        .daemon
        .clone()
}
#[cfg(feature = "ipc-acceptance")]
pub(crate) fn owned_telemetry_ring() -> &'static crate::kernel::shm::ShmRingBuffer {
    &RESOURCES
        .get()
        .expect("active acceptance owns resources")
        .telemetry
}
#[cfg(feature = "ipc-acceptance")]
pub(crate) fn scenario_source() -> &'static str {
    &RESOURCES
        .get()
        .expect("active acceptance owns resources")
        .scenario
}

/// The only pipe an acceptance listener may bind; the canonical pipe stays untouched.
#[cfg(all(windows, feature = "ipc-acceptance"))]
pub(crate) fn acceptance_pipe_name() -> Option<String> {
    active().map(|context| context.resource_plan().pipe)
}

/// Hand the pre-claimed first pipe instance to the listener runtime, or re-claim it
/// after a confirmed owned drop. A non-owned name fails closed instead of binding
/// the canonical pipe.
#[cfg(all(windows, feature = "ipc-acceptance"))]
pub(crate) fn acceptance_pipe_server(
    name: &str,
) -> Result<Option<tokio::net::windows::named_pipe::NamedPipeServer>, String> {
    let Some(context) = active() else {
        return Ok(None);
    };
    let plan = context.resource_plan();
    if plan.pipe != name {
        return Err("Acceptance listener cannot bind a non-owned pipe".into());
    }
    let resources = RESOURCES
        .get()
        .filter(|resources| resources.run_id == context.run_id)
        .ok_or("Acceptance pipe owner mismatch")?;
    let lease = {
        let mut slot = resources
            .pipe
            .lock()
            .map_err(|_| "Acceptance pipe lease poisoned")?;
        match slot.take() {
            Some(lease) => lease,
            None => crate::kernel::server::PipeLease::claim(&plan.pipe)
                .map_err(|error| error.to_string())?,
        }
    };
    if lease.name() != plan.pipe {
        return Err("Pipe lease name mismatch".into());
    }
    let server = lease.into_server().map_err(|error| error.to_string())?;
    Ok(Some(server))
}

#[cfg(feature = "ipc-acceptance")]
pub fn active() -> Option<&'static AcceptanceContext> {
    CONTEXT.get()
}

/// Host OS integrations (tray, notifications, deep links, updater, window-state,
/// global shortcuts) are disabled for supervised acceptance runs.
#[cfg(feature = "ipc-acceptance")]
pub fn os_integrations_enabled() -> bool {
    active().is_none()
}

#[cfg(not(feature = "ipc-acceptance"))]
pub fn os_integrations_enabled() -> bool {
    true
}

/// Apply the private identity before Tauri resolves application directories or creates windows.
pub fn configure_startup(config: &mut tauri::utils::config::Config) -> Result<(), String> {
    #[cfg(feature = "ipc-acceptance")]
    active()
        .ok_or("Acceptance startup requires installed context")?
        .configure_startup(config);
    #[cfg(not(feature = "ipc-acceptance"))]
    let _ = config;
    Ok(())
}

/// Bridge script captured from ordinary startup so private acceptance windows are
/// initialized exactly like the real app window instead of a lookalike.
#[cfg(feature = "ipc-acceptance")]
static BRIDGE_SCRIPT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn set_bridge_script(script: String) -> Result<(), String> {
    BRIDGE_SCRIPT
        .set(script)
        .map_err(|_| "Acceptance bridge script was already installed".into())
}

#[cfg(feature = "ipc-acceptance")]
pub(crate) fn bridge_script() -> Option<&'static str> {
    BRIDGE_SCRIPT.get().map(String::as_str)
}

/// WebView2 profile for a browser role; acceptance runs never share a host profile.
#[cfg(feature = "ipc-acceptance")]
pub fn window_data_directory(role: usize) -> Option<std::path::PathBuf> {
    active().map(|context| context.resource_plan().profiles[role.min(2)].clone())
}

#[cfg(not(feature = "ipc-acceptance"))]
pub fn window_data_directory(_role: usize) -> Option<std::path::PathBuf> {
    None
}

/// Owned log directory for a validated acceptance run; `None` keeps host logging.
#[cfg(feature = "ipc-acceptance")]
pub fn acceptance_logs_dir() -> Option<std::path::PathBuf> {
    active().map(|context| context.resource_plan().logs)
}

#[cfg(not(feature = "ipc-acceptance"))]
pub fn acceptance_logs_dir() -> Option<std::path::PathBuf> {
    None
}

#[cfg(feature = "ipc-acceptance")]
mod policy {
    use serde::Deserialize;
    use std::io::{Read, Write};
    use std::path::{Path, PathBuf};

    const MAX_DESCRIPTOR_BYTES: u64 = 64 * 1024;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    #[serde(rename_all = "lowercase")]
    pub enum AcceptanceTransport {
        Ipc,
        Websocket,
    }

    #[derive(Debug, Deserialize, serde::Serialize, Clone, PartialEq, Eq)]
    #[serde(rename_all = "kebab-case")]
    pub enum AcceptanceCase {
        Preflight,
        NativeSmoke,
        TrustReload,
        TurnHistory,
        Approval,
        Terminal,
        FilesWatchPlugins,
        NetworkIndependent,
        Backpressure,
        RemoteRecovery,
        RemotePeer,
        Measurements,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Descriptor {
        schema_version: u32,
        run_id: String,
        transport: AcceptanceTransport,
        case_id: AcceptanceCase,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Owner {
        schema_version: u32,
        run_id: String,
    }

    fn validate_absolute(path: &Path) -> Result<(), String> {
        if !path.is_absolute() {
            return Err("Run paths must be absolute".into());
        }
        let text = path.to_str().ok_or("Run path must be Unicode")?;
        if text
            .split(['/', '\\'])
            .any(|part| matches!(part, "." | ".."))
        {
            return Err("Run path cannot contain traversal components".into());
        }
        #[cfg(windows)]
        if !matches!(path.components().next(), Some(std::path::Component::Prefix(prefix)) if matches!(prefix.kind(), std::path::Prefix::Disk(_)))
        {
            return Err("Run path cannot use UNC/device/verbatim prefixes".into());
        }
        reject_links(path)
    }

    fn reject_links(path: &Path) -> Result<(), String> {
        for ancestor in path.ancestors() {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            let metadata = match std::fs::symlink_metadata(ancestor) {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => return Err(format!("Cannot inspect run path: {error}")),
            };
            if metadata.file_type().is_symlink() {
                return Err("Run paths cannot contain symbolic links".into());
            }
            #[cfg(windows)]
            {
                use std::os::windows::fs::MetadataExt;
                if metadata.file_attributes()
                    & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
                    != 0
                {
                    return Err("Run paths cannot contain reparse points".into());
                }
            }
        }
        Ok(())
    }

    fn read_owned_file(path: &Path, maximum: u64) -> Result<Vec<u8>, String> {
        reject_links(path)?;
        let file = std::fs::File::open(path)
            .map_err(|error| format!("Cannot open run policy: {error}"))?;
        if !file
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err("Run policy must be a regular file".into());
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
            };
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, info.as_mut_ptr()) }
                == 0
            {
                return Err(format!(
                    "Cannot inspect run policy links: {}",
                    std::io::Error::last_os_error()
                ));
            }
            if unsafe { info.assume_init() }.nNumberOfLinks != 1 {
                return Err("Run policy cannot be hard-linked".into());
            }
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if file.metadata().map_err(|error| error.to_string())?.nlink() != 1 {
                return Err("Run policy cannot be hard-linked".into());
            }
        }
        let mut bytes = Vec::new();
        file.take(maximum + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("Cannot read run policy: {error}"))?;
        if bytes.len() as u64 > maximum {
            return Err(format!("Owned run file exceeds {maximum} bytes"));
        }
        Ok(bytes)
    }

    fn read_owned_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
        serde_json::from_slice(&read_owned_file(path, MAX_DESCRIPTOR_BYTES)?)
            .map_err(|error| format!("Invalid run policy: {error}"))
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct LaunchClaim {
        schema_version: u32,
        run_id: String,
        process_id: u32,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum RunClaim {
        Launch,
        Child,
    }

    #[derive(Debug)]
    pub struct ResourcePlan {
        pub home: PathBuf,
        pub logs: PathBuf,
        pub mcp: PathBuf,
        pub workspace: PathBuf,
        pub bin: PathBuf,
        pub temporary: PathBuf,
        pub reports: PathBuf,
        pub profiles: [PathBuf; 3],
        pub identifier: String,
        pub daemon_mapping: String,
        pub telemetry_mapping: String,
        pub pipe: String,
        pub shell_integrations: bool,
        pub portable_detection: bool,
        pub host_discovery: bool,
    }

    #[derive(Debug)]
    pub struct AcceptanceContext {
        pub root: PathBuf,
        pub run_id: String,
        pub transport: AcceptanceTransport,
        pub case_id: AcceptanceCase,
    }

    impl AcceptanceContext {
        pub(crate) fn configure_startup(&self, config: &mut tauri::utils::config::Config) {
            let plan = self.resource_plan();
            config.identifier = plan.identifier;
            config.app.app_directories_override = Some(
                tauri::utils::config::AppDirectoriesOverride::Root(plan.home),
            );
            // Only the shared, hidden acceptance builder may create a run's windows.
            config.app.windows.clear();
        }

        pub(crate) fn preflight_result(
            &self,
            measurements: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            if self.case_id != AcceptanceCase::Preflight {
                return Err("Acceptance preflight case mismatch".into());
            }
            let mut native = measurements
                .as_object()
                .ok_or("Acceptance preflight measurements must be an object")?
                .clone();
            for key in [
                "runId",
                "caseId",
                "transport",
                "pid",
                "claims",
                "outcome",
                "renderer",
            ] {
                if native.contains_key(key) {
                    return Err(format!("Reserved acceptance preflight field: {key}"));
                }
            }
            let identity = serde_json::json!({"runId":self.run_id,"caseId":self.case_id,"transport":match self.transport { AcceptanceTransport::Ipc => "ipc", AcceptanceTransport::Websocket => "websocket" },"pid":std::process::id(),"claims":self.preflight_claims()?});
            native.extend(
                identity
                    .as_object()
                    .expect("native identity object")
                    .clone(),
            );
            // Evidence only: Node joins cleanup/process claims and case-specific measurements.
            Ok(
                serde_json::json!({"schemaVersion":1,"runId":self.run_id,"caseId":self.case_id,"native":native}),
            )
        }

        pub(crate) fn preflight_claims(&self) -> Result<serde_json::Value, String> {
            let owner: Owner = read_owned_json(&self.root.join("owner.json"))?;
            let launch: LaunchClaim = read_owned_json(&self.root.join("launch.claim"))?;
            let child: LaunchClaim = read_owned_json(&self.root.join("child.claim"))?;
            if owner.schema_version != 1
                || owner.run_id != self.run_id
                || launch.schema_version != 1
                || launch.run_id != self.run_id
                || child.schema_version != 1
                || child.run_id != self.run_id
                || launch.process_id == 0
                || launch.process_id == child.process_id
                || child.process_id != std::process::id()
            {
                return Err("Acceptance preflight claim identity mismatch".into());
            }
            Ok(
                serde_json::json!({"ownerRunId":owner.run_id,"launcherPid":launch.process_id,"childPid":child.process_id}),
            )
        }

        pub(crate) fn write_secondary_result(
            &self,
            result: &serde_json::Value,
        ) -> Result<(), String> {
            if self.case_id != AcceptanceCase::TrustReload {
                return Err("Secondary acceptance results require trust-reload".into());
            }
            self.write_report_file("secondary.json", result)
        }

        pub(crate) fn read_secondary_result(&self) -> Result<serde_json::Value, String> {
            if self.case_id != AcceptanceCase::TrustReload {
                return Err("Secondary acceptance results require trust-reload".into());
            }
            let bytes = read_owned_file(
                &self.resource_plan().reports.join("secondary.json"),
                8 * 1024 * 1024,
            )?;
            serde_json::from_slice(&bytes)
                .map_err(|error| format!("Invalid secondary acceptance result: {error}"))
        }

        pub(crate) fn write_result(&self, result: &serde_json::Value) -> Result<(), String> {
            use std::io::Write;
            let bytes = serde_json::to_vec(result).map_err(|error| error.to_string())?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err("Acceptance result exceeds 8 MiB".into());
            }
            let reports = self.resource_plan().reports;
            // Root was validated before canonicalization (which adds \\?\ on Windows).
            reject_links(&reports)?;
            if !reports.is_dir() {
                return Err("Acceptance reports directory is missing".into());
            }
            // Exclusive finalization rejects existing files, including hardlinks, without touching them.
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(reports.join("result.json"))
                .map_err(|error| format!("Cannot create acceptance result: {error}"))?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|error| format!("Cannot persist acceptance result: {error}"))
        }

        fn write_report_file(&self, name: &str, result: &serde_json::Value) -> Result<(), String> {
            use std::io::Write;
            let bytes = serde_json::to_vec(result).map_err(|error| error.to_string())?;
            if bytes.len() > 8 * 1024 * 1024 {
                return Err("Acceptance result exceeds 8 MiB".into());
            }
            let reports = self.resource_plan().reports;
            reject_links(&reports)?;
            if !reports.is_dir() {
                return Err("Acceptance reports directory is missing".into());
            }
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(reports.join(name))
                .map_err(|error| format!("Cannot create acceptance result: {error}"))?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|error| format!("Cannot persist acceptance result: {error}"))
        }

        pub fn scenario_script(&self) -> Result<String, String> {
            String::from_utf8(read_owned_file(&self.root.join("scenario.js"), 256 * 1024)?)
                .map_err(|e| e.to_string())
        }

        pub fn claim_resources(&self) -> Result<(), String> {
            super::claim_owned_resources(self)
        }

        #[cfg(windows)]
        pub fn claim_pipe(&self) -> Result<std::os::windows::io::OwnedHandle, String> {
            let resources = super::RESOURCES
                .get()
                .ok_or("Acceptance resources unavailable")?;
            if resources.run_id != self.run_id {
                return Err("Pipe resource owner mismatch".into());
            }
            let initial = resources.pipe.lock().unwrap().take();
            let lease = match initial {
                Some(lease) => lease,
                None => crate::kernel::server::PipeLease::claim(&self.resource_plan().pipe)
                    .map_err(|e| e.to_string())?,
            };
            if lease.name() != self.resource_plan().pipe {
                return Err("Pipe lease name mismatch".into());
            }
            Ok(lease.into_handle())
        }

        pub fn resource_plan(&self) -> ResourcePlan {
            ResourcePlan {
                home: self.root.join("home"),
                logs: self.root.join("logs"),
                mcp: self.root.join("home/mcp_servers.json"),
                workspace: self.root.join("workspace"),
                bin: self.root.join("bin"),
                temporary: self.root.join("tmp"),
                reports: self.root.join("reports"),
                profiles: ["main", "secondary", "guest"]
                    .map(|role| self.root.join("profile").join(role)),
                identifier: format!("sh.paseo.desktop.ipc-acceptance.{}", self.run_id),
                daemon_mapping: format!("kronos-ipc-acceptance-{}-daemon", self.run_id),
                telemetry_mapping: format!("kronos-ipc-acceptance-{}-telemetry", self.run_id),
                pipe: format!(r"\\.\pipe\kronos-ipc-acceptance-{}-pipe", self.run_id),
                shell_integrations: false,
                portable_detection: false,
                host_discovery: false,
            }
        }

        pub fn fixture_command(&self, cmd: &str) -> Option<PathBuf> {
            let bin = self.resource_plan().bin;
            for name in ["codex", "node"] {
                let filename = format!("{name}{}", std::env::consts::EXE_SUFFIX);
                let path = bin.join(&filename);
                if cmd == name || cmd == filename || Path::new(cmd) == path {
                    return (reject_links(&path).is_ok() && path.is_file()).then_some(path);
                }
            }
            None
        }

        pub fn claim(&self, kind: RunClaim) -> Result<(), String> {
            reject_links(&self.root)?;
            let owner: Owner = read_owned_json(&self.root.join("owner.json"))?;
            if owner.schema_version != 1 || owner.run_id != self.run_id {
                return Err("Run owner marker changed".into());
            }
            let name = match kind {
                RunClaim::Launch => "launch.claim",
                RunClaim::Child => {
                    let parent: LaunchClaim = read_owned_json(&self.root.join("launch.claim"))?;
                    if parent.schema_version != 1
                        || parent.run_id != self.run_id
                        || parent.process_id == 0
                    {
                        return Err("Run launch claim does not match child".into());
                    }
                    "child.claim"
                }
            };
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 1, "runId": self.run_id, "processId": std::process::id()
            }))
            .map_err(|error| error.to_string())?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.root.join(name))
                .map_err(|error| format!("Cannot claim run: {error}"))?;
            file.write_all(&bytes)
                .map_err(|error| format!("Cannot write run claim: {error}"))
        }

        pub fn validate_environment(
            &self,
            kronos_home: Option<&std::ffi::OsStr>,
            paseo_home: Option<&std::ffi::OsStr>,
        ) -> Result<(), String> {
            let expected = self.root.join("home");
            for home in [kronos_home, paseo_home] {
                let path = Path::new(home.ok_or("Both run home variables are required")?);
                validate_absolute(path)?;
                if path.canonicalize().map_err(|error| error.to_string())? != expected {
                    return Err("Run home variables must match the owned home".into());
                }
            }
            Ok(())
        }

        pub fn from_descriptor(path: &Path) -> Result<Self, String> {
            validate_absolute(path)?;
            if path.file_name() != Some(std::ffi::OsStr::new("run.json")) {
                return Err("Run descriptor must be named run.json".into());
            }
            let descriptor: Descriptor = read_owned_json(path)?;
            if descriptor.schema_version != 1
                || descriptor.run_id.len() != 32
                || !descriptor
                    .run_id
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
            {
                return Err("Invalid run descriptor version or run ID".into());
            }
            let root = path.parent().ok_or("Run descriptor has no parent")?;
            let expected_name = format!("kronos-ipc-acceptance-{}", descriptor.run_id);
            if root.file_name() != Some(std::ffi::OsStr::new(&expected_name)) {
                return Err("Descriptor is not in its owned run directory".into());
            }
            let owner: Owner = read_owned_json(&root.join("owner.json"))?;
            if owner.schema_version != 1 || owner.run_id != descriptor.run_id {
                return Err("Run owner marker does not match descriptor".into());
            }
            for relative in [
                "home",
                "profile",
                "profile/main",
                "profile/secondary",
                "profile/guest",
                "logs",
                "tmp",
                "workspace",
                "bin",
                "reports",
                "scenario.js",
                "launch.claim",
                "child.claim",
            ] {
                reject_links(&root.join(relative))?;
            }
            match std::fs::symlink_metadata(root.join("child.claim")) {
                Ok(_) => return Err("Run descriptor was already consumed".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.to_string()),
            }
            Ok(Self {
                root: root.canonicalize().map_err(|error| error.to_string())?,
                run_id: descriptor.run_id,
                transport: descriptor.transport,
                case_id: descriptor.case_id,
            })
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::Cell;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[cfg(not(feature = "ipc-acceptance"))]
    #[test]
    fn test_ordinary_build_rejects_acceptance_before_side_effects() {
        // Removing the boot sentry must not execute logger/CLI/desktop setup.
        for flag in [
            "--ipc-acceptance-info",
            "--ipc-acceptance-run",
            "--ipc-acceptance-child",
            "--ipc-acceptance-unknown",
        ] {
            let called = Cell::new(false);
            let result = enter(&args(&["desktop", flag]), || called.set(true));
            assert!(result.is_err(), "ordinary build accepted {flag}");
            assert!(!called.get(), "rejected {flag} reached desktop setup");
        }
    }

    #[cfg(not(feature = "ipc-acceptance"))]
    #[test]
    fn test_normal_boot_policy_is_unchanged() {
        for arguments in [
            args(&["desktop"]),
            args(&["desktop", "daemon", "--port", "0"]),
            args(&["desktop", "kronos://workspace/owned"]),
        ] {
            let called = Cell::new(0);
            assert_eq!(enter(&arguments, || called.set(called.get() + 1)), Ok(0));
            assert_eq!(called.get(), 1);
        }
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_build_requires_supervised_invocation() {
        // A private test executable must never double-click into production startup.
        for arguments in [
            args(&["desktop"]),
            args(&["desktop", "daemon", "--port", "0"]),
            args(&["desktop", "--ipc-acceptance-child", "missing.json"]),
            args(&["desktop", "--ipc-acceptance-run"]),
            args(&["desktop", "--ipc-acceptance-info", "extra"]),
            args(&["desktop", "--ipc-acceptance-unknown"]),
        ] {
            let called = Cell::new(false);
            assert!(enter(&arguments, || called.set(true)).is_err());
            assert!(!called.get());
        }
        let called = Cell::new(false);
        assert_eq!(
            enter(&args(&["desktop", "--ipc-acceptance-info"]), || called
                .set(true)),
            Ok(0)
        );
        assert!(!called.get());
    }

    #[cfg(feature = "ipc-acceptance")]
    pub(crate) struct Fixture {
        _temporary: tempfile::TempDir,
        pub(crate) root: std::path::PathBuf,
        pub(crate) descriptor: std::path::PathBuf,
    }

    #[cfg(feature = "ipc-acceptance")]
    impl Fixture {
        pub(crate) fn new() -> Self {
            let temporary = tempfile::tempdir().unwrap();
            let root = temporary
                .path()
                .join("kronos-ipc-acceptance-0123456789abcdef0123456789abcdef");
            std::fs::create_dir_all(root.join("home")).unwrap();
            std::fs::write(
                root.join("owner.json"),
                br#"{"schemaVersion":1,"runId":"0123456789abcdef0123456789abcdef"}"#,
            )
            .unwrap();
            let descriptor = root.join("run.json");
            let fixture = Self {
                _temporary: temporary,
                root,
                descriptor,
            };
            fixture.write_descriptor(serde_json::json!({
                "schemaVersion": 1,
                "runId": "0123456789abcdef0123456789abcdef",
                "transport": "ipc",
                "caseId": "preflight"
            }));
            fixture
        }
        pub(crate) fn retain(self) -> std::path::PathBuf {
            self._temporary.keep()
        }

        fn write_descriptor(&self, value: serde_json::Value) {
            std::fs::write(&self.descriptor, value.to_string()).unwrap();
        }
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_startup_propagates_surface_creation_errors() {
        let mut attempted = false;
        let result = start_acceptance_surfaces(&AcceptanceCase::TrustReload, || {
            attempted = true;
            Err("window builder failed".into())
        });
        assert_eq!(result, Err("window builder failed".into()));
        assert!(attempted, "UI case skipped its initial trusted surface");
        let mut preflight_attempted = false;
        assert_eq!(
            start_acceptance_surfaces(&AcceptanceCase::Preflight, || {
                preflight_attempted = true;
                Ok(())
            }),
            Ok(())
        );
        assert!(
            !preflight_attempted,
            "Native-only preflight created a window"
        );
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_preflight_result_is_native_evidence_only() {
        let fixture = Fixture::new();
        let mut context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        context.claim(RunClaim::Launch).unwrap();
        std::fs::write(fixture.root.join("launch.claim"), serde_json::json!({"schemaVersion":1,"runId":context.run_id,"processId":std::process::id()+1}).to_string()).unwrap();
        context.claim(RunClaim::Child).unwrap();
        let result = context
            .preflight_result(serde_json::json!({"engineEpoch":7,"windowCount":0}))
            .expect("Native preflight evidence rejected");
        assert_eq!(result["schemaVersion"], 1);
        assert_eq!(result["runId"], context.run_id);
        assert_eq!(result["caseId"], "preflight");
        assert_eq!(result["native"]["runId"], context.run_id);
        assert_eq!(result["native"]["caseId"], "preflight");
        assert_eq!(result["native"]["transport"], "ipc");
        assert_eq!(result["native"]["pid"], std::process::id());
        assert_eq!(result["native"]["claims"]["childPid"], std::process::id());
        assert_eq!(result["native"]["engineEpoch"], 7);
        assert!(result.get("outcome").is_none());
        assert!(result.get("renderer").is_none());
        assert!(context.preflight_result(serde_json::json!([])).is_err());
        for key in [
            "runId",
            "caseId",
            "transport",
            "pid",
            "claims",
            "outcome",
            "renderer",
        ] {
            assert!(
                context
                    .preflight_result(serde_json::json!({key:"spoofed"}))
                    .is_err(),
                "Reserved native field accepted: {key}"
            );
        }
        context.case_id = AcceptanceCase::NativeSmoke;
        assert!(context.preflight_result(serde_json::json!({})).is_err());
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_preflight_claims_bind_current_child() {
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        context.claim(RunClaim::Launch).unwrap();
        let child_pid = std::process::id();
        let launcher_pid = child_pid.checked_add(1).unwrap();
        let claim = |pid, run_id: &str| {
            serde_json::json!({"schemaVersion":1,"runId":run_id,"processId":pid}).to_string()
        };
        let launch = fixture.root.join("launch.claim");
        let child = fixture.root.join("child.claim");
        std::fs::write(&launch, claim(launcher_pid, &context.run_id)).unwrap();
        context.claim(RunClaim::Child).unwrap();
        let evidence = context
            .preflight_claims()
            .expect("Valid owned child claims rejected");
        assert_eq!(evidence["ownerRunId"], context.run_id);
        assert_eq!(evidence["launcherPid"], launcher_pid);
        assert_eq!(evidence["childPid"], child_pid);
        for (path, pid, run) in [
            (&child, launcher_pid, context.run_id.as_str()),
            (&child, child_pid, "another-run"),
            (&launch, 0, context.run_id.as_str()),
            (&launch, child_pid, context.run_id.as_str()),
            (&launch, launcher_pid, "another-run"),
        ] {
            let before = std::fs::read(path).unwrap();
            std::fs::write(path, claim(pid, run)).unwrap();
            assert!(
                context.preflight_claims().is_err(),
                "Invalid claim authorized preflight"
            );
            std::fs::write(path, before).unwrap();
        }
        std::fs::write(
            fixture.root.join("owner.json"),
            br#"{"schemaVersion":1,"runId":"another-run"}"#,
        )
        .unwrap();
        assert!(
            context.preflight_claims().is_err(),
            "Changed owner authorized preflight"
        );
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_startup_config_is_run_owned() {
        use tauri::utils::config::{AppDirectoriesOverride, Config, WindowConfig};
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let mut config: Config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let original = config.clone();
        assert!(
            configure_startup(&mut config).is_err(),
            "Private startup accepted missing context"
        );
        assert_eq!(
            serde_json::to_value(&config).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        config.app.windows.push(WindowConfig::default());
        context.configure_startup(&mut config);
        assert_eq!(
            config.identifier, "sh.paseo.desktop.ipc-acceptance.0123456789abcdef0123456789abcdef",
            "Startup kept production identity"
        );
        assert_eq!(
            config.app.app_directories_override,
            Some(AppDirectoriesOverride::Root(
                fixture.root.canonicalize().unwrap().join("home")
            ))
        );
        assert!(
            config.app.windows.is_empty(),
            "Config may auto-create an unowned window"
        );
        assert_eq!(
            serde_json::to_value(&config.app.security).unwrap(),
            serde_json::to_value(&original.app.security).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&config.build).unwrap(),
            serde_json::to_value(&original.build).unwrap()
        );
        assert_eq!(config.plugins.0, original.plugins.0);
        assert!(
            active().is_none(),
            "Config mutation installed global acceptance state"
        );
    }

    #[cfg(not(feature = "ipc-acceptance"))]
    #[test]
    fn test_normal_startup_config_is_unchanged() {
        let mut config: tauri::utils::config::Config =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let before = serde_json::to_value(&config).unwrap();
        configure_startup(&mut config).unwrap();
        assert_eq!(serde_json::to_value(&config).unwrap(), before);
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_trust_reload_secondary_result_is_exclusive() {
        let fixture = Fixture::new();
        let mut context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        context.case_id = AcceptanceCase::TrustReload;
        let reports = fixture.root.join("reports");
        std::fs::create_dir(&reports).unwrap();
        let result = serde_json::json!({"schemaVersion":1,"runId":context.run_id,"caseId":"trust-reload","native":{"role":"secondary"}});
        context.write_secondary_result(&result).unwrap();
        assert_eq!(context.read_secondary_result().unwrap(), result);
        assert!(
            !reports.join("result.json").exists(),
            "Secondary finalized the primary result"
        );
        let before = std::fs::read(reports.join("secondary.json")).unwrap();
        assert!(context
            .write_secondary_result(&serde_json::json!({"overwrite":true}))
            .is_err());
        assert_eq!(
            std::fs::read(reports.join("secondary.json")).unwrap(),
            before
        );
        context.case_id = AcceptanceCase::NativeSmoke;
        assert!(context.write_secondary_result(&result).is_err());
        assert!(context.read_secondary_result().is_err());
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_result_file_is_bounded_and_exclusive() {
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let reports = fixture.root.join("reports");
        std::fs::create_dir(&reports).unwrap();
        let oversized = serde_json::json!({"renderer": "x".repeat(8 * 1024 * 1024)});
        assert!(context.write_result(&oversized).is_err());
        assert!(
            !reports.join("result.json").exists(),
            "Overflow left a result file"
        );
        let result = serde_json::json!({"schemaVersion":1,"native":{"engineEpoch":7},"renderer":{"outcome":"pass"}});
        context
            .write_result(&result)
            .expect("Owned result must be written");
        let bytes = std::fs::read(reports.join("result.json")).unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            result
        );
        assert!(context
            .write_result(&serde_json::json!({"overwrite":true}))
            .is_err());
        assert_eq!(std::fs::read(reports.join("result.json")).unwrap(), bytes);
        std::fs::remove_file(reports.join("result.json")).unwrap();
        let sentinel = fixture.root.join("sentinel.json");
        std::fs::write(&sentinel, b"sentinel").unwrap();
        std::fs::hard_link(&sentinel, reports.join("result.json")).unwrap();
        assert!(
            context.write_result(&result).is_err(),
            "Hardlink overwritten"
        );
        assert_eq!(std::fs::read(sentinel).unwrap(), b"sentinel");
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_scenario_is_bounded_and_unlinked() {
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let script = fixture.root.join("scenario.js");
        std::fs::write(&script, vec![b'x'; 256 * 1024 + 1]).unwrap();
        assert!(
            context.scenario_script().is_err(),
            "oversized scenario accepted"
        );
        std::fs::write(&script, "void 0;").unwrap();
        assert_eq!(context.scenario_script().unwrap(), "void 0;");
        std::fs::hard_link(&script, fixture.root.join("alias.js")).unwrap();
        assert!(
            context.scenario_script().is_err(),
            "hard-linked scenario accepted"
        );
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_descriptor_is_strict() {
        // Reject malformed/unknown policy, without rejecting a real owned fixture.
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        assert_eq!(context.run_id, "0123456789abcdef0123456789abcdef");
        assert_eq!(context.transport, AcceptanceTransport::Ipc);
        assert_eq!(context.case_id, AcceptanceCase::Preflight);
        assert_eq!(context.root, fixture.root.canonicalize().unwrap());
        let valid: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&fixture.descriptor).unwrap()).unwrap();
        for (key, value) in [
            ("schemaVersion", serde_json::json!(2)),
            ("transport", serde_json::json!("IPC")),
            ("transport", serde_json::json!("node")),
            ("caseId", serde_json::json!("unknown")),
            ("runId", serde_json::json!("../not-owned")),
            (
                "runId",
                serde_json::json!("0123456789ABCDEF0123456789ABCDEF"),
            ),
            ("runId", serde_json::json!("")),
            ("extra", serde_json::json!(true)),
        ] {
            let mut invalid = valid.clone();
            invalid[key] = value;
            fixture.write_descriptor(invalid);
            assert!(
                AcceptanceContext::from_descriptor(&fixture.descriptor).is_err(),
                "accepted invalid {key}"
            );
        }
        for key in ["schemaVersion", "runId", "transport", "caseId"] {
            let mut invalid = valid.clone();
            invalid.as_object_mut().unwrap().remove(key);
            fixture.write_descriptor(invalid);
            assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
        }
        std::fs::write(&fixture.descriptor, vec![b' '; 64 * 1024 + 1]).unwrap();
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
        std::fs::write(&fixture.descriptor, b"not json").unwrap();
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
    }

    #[cfg(feature = "ipc-acceptance")]
    #[tokio::test]
    async fn test_acceptance_root_cannot_escape_or_be_reused() {
        // Prefix/owner/consumed-run mistakes must reject before any desktop effects.
        let fixture = Fixture::new();
        let wrong = fixture._temporary.path().join("not-an-owned-run");
        std::fs::rename(&fixture.root, &wrong).unwrap();
        assert!(AcceptanceContext::from_descriptor(&wrong.join("run.json")).is_err());
        std::fs::rename(&wrong, &fixture.root).unwrap();
        assert!(
            AcceptanceContext::from_descriptor(std::path::Path::new("run.json"))
                .unwrap_err()
                .contains("absolute")
        );
        assert!(
            AcceptanceContext::from_descriptor(&fixture.root.join("home/../run.json")).is_err()
        );
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        assert!(active().is_none());
        let home = fixture.root.join("home");
        assert!(context
            .validate_environment(Some(home.as_os_str()), Some(home.as_os_str()))
            .is_ok());
        assert!(context
            .validate_environment(None, Some(home.as_os_str()))
            .is_err());
        assert!(context
            .validate_environment(Some(home.as_os_str()), None)
            .is_err());
        assert!(context
            .validate_environment(Some(fixture.root.as_os_str()), Some(home.as_os_str()))
            .is_err());
        assert!(context
            .validate_environment(Some(home.as_os_str()), Some(fixture.root.as_os_str()))
            .is_err());
        std::fs::write(
            fixture.root.join("owner.json"),
            br#"{"schemaVersion":1,"runId":"00000000000000000000000000000000"}"#,
        )
        .unwrap();
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
        std::fs::remove_file(fixture.root.join("owner.json")).unwrap();
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
        std::fs::write(
            fixture.root.join("owner.json"),
            br#"{"schemaVersion":1,"runId":"0123456789abcdef0123456789abcdef"}"#,
        )
        .unwrap();
        std::fs::write(fixture.root.join("child.claim"), "consumed").unwrap();
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
        std::fs::remove_file(fixture.root.join("child.claim")).unwrap();
        let alias = fixture.root.join("alias.json");
        std::fs::hard_link(&fixture.descriptor, &alias).unwrap();
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
        std::fs::remove_file(alias).unwrap();
        #[cfg(windows)]
        {
            for path in [
                r"\\server\share\run.json",
                r"\\?\C:\run.json",
                r"\\.\C:\run.json",
                r"C:run.json",
            ] {
                assert!(AcceptanceContext::from_descriptor(std::path::Path::new(path)).is_err());
            }
            let target = fixture._temporary.path().join("outside-owned-run");
            std::fs::create_dir(&target).unwrap();
            std::fs::write(target.join("sentinel"), "unchanged").unwrap();
            let junction = fixture.root.join("profile");
            let mut child = tokio::process::Command::new(std::env::var_os("ComSpec").unwrap())
                .args([
                    std::ffi::OsStr::new("/d"),
                    std::ffi::OsStr::new("/c"),
                    std::ffi::OsStr::new("mklink"),
                    std::ffi::OsStr::new("/J"),
                    junction.as_os_str(),
                    target.as_os_str(),
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true)
                .spawn()
                .unwrap();
            let result =
                tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await;
            if result.is_err() {
                let _ = child.start_kill();
                if !matches!(
                    tokio::time::timeout(std::time::Duration::from_secs(3), child.wait()).await,
                    Ok(Ok(_))
                ) {
                    let retained = fixture._temporary.keep();
                    panic!(
                        "Junction helper exit unconfirmed; retain {}",
                        retained.display()
                    );
                }
                panic!("Junction helper exceeded deadline");
            }
            assert!(
                result.unwrap().unwrap().success(),
                "owned junction fixture creation failed"
            );
            assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
            assert_eq!(
                std::fs::read_to_string(target.join("sentinel")).unwrap(),
                "unchanged"
            );
            std::fs::remove_dir(junction).unwrap();
        }
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_ok());
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_fixture_discovery_never_uses_host_fallbacks() {
        // A decoy PATH or absolute provider command must not escape the fixture resolver.
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let bin = context.resource_plan().bin;
        std::fs::create_dir(&bin).unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        for command in ["codex", "node"] {
            let expected = bin.join(format!("{command}{suffix}"));
            std::fs::write(&expected, b"resolver-only fixture, never executed").unwrap();
            for input in [command.to_string(), expected.to_string_lossy().to_string()] {
                let called = Cell::new(false);
                let actual = crate::agent::registry::resolve_command_with_context(
                    &input,
                    Some(&context),
                    || {
                        called.set(true);
                        Some(fixture.root.parent().unwrap().join("decoy-provider"))
                    },
                );
                assert_eq!(actual, Some(expected.clone()));
                assert!(!called.get());
            }
        }
        for command in [
            "claude",
            "omp",
            "ollama",
            "lms",
            "../codex",
            "C:\\unowned\\codex.exe",
        ] {
            assert!(
                crate::agent::registry::resolve_command_with_context(
                    command,
                    Some(&context),
                    || Some(bin.join("node"))
                )
                .is_none(),
                "escaped fixture resolver: {command}"
            );
        }
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        assert!(!crate::agent::registry::local_port_available_with_context(
            Some(&context),
            || std::net::TcpStream::connect(address).is_ok()
        ));
        assert_eq!(
            listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        assert!(crate::agent::registry::local_port_available_with_context(
            None,
            || true
        ));
        let fallback = fixture.root.join("normal-command");
        assert_eq!(
            crate::agent::registry::resolve_command_with_context("normal", None, || Some(
                fallback.clone()
            )),
            Some(fallback)
        );
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_resource_plan_owns_all_paths() {
        // Production names or host discovery must never reach the private run.
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let plan = context.resource_plan();
        let root = fixture.root.canonicalize().unwrap();
        assert_eq!(plan.home, root.join("home"));
        assert_eq!(plan.logs, root.join("logs"));
        assert_eq!(plan.mcp, root.join("home/mcp_servers.json"));
        assert_eq!(plan.workspace, root.join("workspace"));
        assert_eq!(plan.bin, root.join("bin"));
        assert_eq!(plan.temporary, root.join("tmp"));
        assert_eq!(plan.reports, root.join("reports"));
        assert_eq!(
            plan.profiles,
            [
                root.join("profile/main"),
                root.join("profile/secondary"),
                root.join("profile/guest")
            ]
        );
        assert_eq!(
            plan.identifier,
            "sh.paseo.desktop.ipc-acceptance.0123456789abcdef0123456789abcdef"
        );
        assert_eq!(
            plan.daemon_mapping,
            "kronos-ipc-acceptance-0123456789abcdef0123456789abcdef-daemon"
        );
        assert_eq!(
            plan.telemetry_mapping,
            "kronos-ipc-acceptance-0123456789abcdef0123456789abcdef-telemetry"
        );
        assert_eq!(
            plan.pipe,
            r"\\.\pipe\kronos-ipc-acceptance-0123456789abcdef0123456789abcdef-pipe"
        );
        assert!(!plan.shell_integrations);
        assert!(!plan.portable_detection);
        assert!(!plan.host_discovery);
        assert!(
            active().is_none(),
            "pure planning must not publish startup authority"
        );
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_acceptance_run_claims_are_exclusive() {
        let fixture = Fixture::new();
        let context = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        assert!(context.claim(RunClaim::Child).is_err());
        context.claim(RunClaim::Launch).unwrap();
        let original = std::fs::read(fixture.root.join("launch.claim")).unwrap();
        assert!(context.claim(RunClaim::Launch).is_err());
        assert_eq!(
            std::fs::read(fixture.root.join("launch.claim")).unwrap(),
            original
        );
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_ok());
        context.claim(RunClaim::Child).unwrap();
        let original = std::fs::read(fixture.root.join("child.claim")).unwrap();
        assert!(context.claim(RunClaim::Child).is_err());
        assert_eq!(
            std::fs::read(fixture.root.join("child.claim")).unwrap(),
            original
        );
        assert!(AcceptanceContext::from_descriptor(&fixture.descriptor).is_err());
    }
}
