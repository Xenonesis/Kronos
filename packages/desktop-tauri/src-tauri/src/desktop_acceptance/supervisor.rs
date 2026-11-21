use super::{AcceptanceContext, RunClaim};
use crate::agent::runner::turn_process::ProcessTree;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

const READY: &[u8] = b"KRONOS_IPC_ACCEPTANCE_READY ";
const CAPTURE_LIMIT: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Challenge {
    run_id: String,
    parent_pid: u32,
    child_pid: u32,
    nonce: String,
}

// Called by the private child before context installation or any application globals.
// A claim file or environment variable alone never satisfies this live pipe exchange.
pub fn child_handshake(run: &AcceptanceContext) -> Result<(), String> {
    use std::io::{BufRead, Read, Write};
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Storage::FileSystem::{GetFileType, FILE_TYPE_PIPE},
            System::Console::{GetStdHandle, STD_INPUT_HANDLE},
        };
        if GetFileType(GetStdHandle(STD_INPUT_HANDLE)) != FILE_TYPE_PIPE {
            return Err("Supervisor handshake requires an inherited pipe".into());
        }
    }
    let mut input = std::io::stdin().lock();
    let mut line = String::new();
    (&mut input)
        .take(4097)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line.len() > 4096 || !line.ends_with('\n') {
        return Err("Missing bounded supervisor challenge".into());
    }
    let challenge: Challenge =
        serde_json::from_str(&line).map_err(|_| "Invalid supervisor challenge")?;
    if challenge.run_id != run.run_id
        || challenge.child_pid != std::process::id()
        || challenge.parent_pid == 0
        || challenge.nonce.len() != 64
        || !challenge
            .nonce
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err("Supervisor challenge does not match child".into());
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::System::{JobObjects::IsProcessInJob, Threading::GetCurrentProcess};
        let mut assigned = 0;
        if IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut assigned) == 0
            || assigned == 0
        {
            return Err("Acceptance child is not owned by a Job".into());
        }
    }
    #[cfg(not(windows))]
    return Err("Acceptance supervision requires verified Windows Job ownership".into());
    let mut output = std::io::stdout().lock();
    output
        .write_all(b"\n")
        .and_then(|_| output.write_all(READY))
        .and_then(|_| output.write_all(line.as_bytes()))
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())?;
    line.clear();
    (&mut input)
        .take(4097)
        .read_line(&mut line)
        .map_err(|e| e.to_string())?;
    if line != format!("GO {}\n", challenge.nonce) {
        return Err("Supervisor did not release owned child".into());
    }
    run.claim(RunClaim::Child)
}

pub(super) fn argument_path(path: &std::path::Path) -> std::path::PathBuf {
    // Validation rejects user-supplied verbatim/device paths. Canonical owned DOS
    // paths must cross the child argument/environment boundary in that same spelling.
    #[cfg(windows)]
    if matches!(path.components().next(), Some(std::path::Component::Prefix(prefix))
        if matches!(prefix.kind(), std::path::Prefix::VerbatimDisk(_)))
    {
        return std::path::PathBuf::from(
            path.as_os_str()
                .to_string_lossy()
                .trim_start_matches(r"\\?\"),
        );
    }
    path.to_path_buf()
}

fn owned_environment(command: &mut Command, run: &AcceptanceContext) {
    let plan = run.resource_plan();
    let home = argument_path(&plan.home);
    let temporary = argument_path(&plan.temporary);
    command.env_clear();
    for key in ["SystemRoot", "WINDIR", "COMSPEC", "PATHEXT"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    for key in ["KRONOS_HOME", "PASEO_HOME", "HOME", "USERPROFILE"] {
        command.env(key, &home);
    }
    for key in ["TEMP", "TMP", "TMPDIR"] {
        command.env(key, &temporary);
    }
    command
        .env("APPDATA", home.join("AppData/Roaming"))
        .env("LOCALAPPDATA", home.join("AppData/Local"))
        .env("CODEX_HOME", home.join(".codex"))
        .env("PATH", argument_path(&plan.bin))
        .env(
            "KRONOS_IPC_ACCEPTANCE_DESCRIPTOR",
            argument_path(&run.root.join("run.json")),
        )
        // The managed transport stays opt-in. Only this owned run selects it, from its
        // own descriptor; without it the renderer never opens an embedded session.
        .env(
            "KRONOS_DESKTOP_LOCAL_TRANSPORT",
            match run.transport {
                crate::desktop_acceptance::AcceptanceTransport::Ipc => "ipc",
                crate::desktop_acceptance::AcceptanceTransport::Websocket => "websocket",
            },
        );
}

async fn capture_output<R: AsyncRead + Unpin>(
    mut input: R,
    mut output: tokio::fs::File,
    mut ready: Option<(Challenge, tokio::sync::oneshot::Sender<()>)>,
) -> Result<(), String> {
    let mut bytes = [0u8; 8192];
    let mut total = 0;
    let mut line = Vec::new();
    loop {
        let read = input.read(&mut bytes).await.map_err(|e| e.to_string())?;
        if read == 0 {
            return Ok(());
        }
        total += read;
        if total > CAPTURE_LIMIT {
            return Err("Acceptance diagnostic output exceeds bound".into());
        }
        output
            .write_all(&bytes[..read])
            .await
            .map_err(|e| e.to_string())?;
        if ready.is_none() {
            continue;
        }
        for &byte in &bytes[..read] {
            if byte == b'\n' {
                if let Some(payload) = line.strip_prefix(READY) {
                    let observed: Challenge = serde_json::from_slice(payload)
                        .map_err(|_| "Invalid child acknowledgment")?;
                    let (expected, sender) = ready.take().unwrap();
                    if observed != expected {
                        return Err("Child acknowledgment mismatch".into());
                    }
                    let _ = sender.send(());
                    break;
                }
                line.clear();
            } else {
                if line.len() >= 4096 {
                    return Err("Child pre-handshake line exceeds bound".into());
                }
                line.push(byte);
            }
        }
    }
}

#[derive(Debug)]
pub struct OwnedExit {
    pub exit_code: i32,
    pub leader_reaped: bool,
    pub tree_empty: bool,
    pub timed_out: bool,
}

pub async fn supervise(
    command: &mut Command,
    run: &AcceptanceContext,
    deadline: Duration,
) -> Result<OwnedExit, String> {
    #[cfg(not(windows))]
    return Err("Acceptance supervision requires verified Windows Job ownership".into());
    let started = tokio::time::Instant::now();
    let until = started + deadline;
    if deadline.is_zero() || deadline > Duration::from_secs(420) {
        return Err("Acceptance workload deadline must be within 420 seconds".into());
    }
    let plan = run.resource_plan();
    if command.as_std().get_current_dir() != Some(plan.workspace.as_path()) {
        return Err("Acceptance CWD must be the owned workspace".into());
    }
    run.claim(RunClaim::Launch)?;
    std::fs::create_dir_all(&plan.logs).map_err(|e| e.to_string())?;
    let mut files = Vec::new();
    for name in ["supervisor-stdout.log", "supervisor-stderr.log"] {
        files.push(
            tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(plan.logs.join(name))
                .await
                .map_err(|e| e.to_string())?,
        );
    }
    owned_environment(command, run);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let (mut tree, mut child) = ProcessTree::spawn_strict(command)?;
    let setup = tree.attach(&child);
    if let Err(error) = setup {
        let cleanup = tree.cleanup(&mut child).await;
        return Err(format!(
            "{error}; cleanup={cleanup:?}; retain {}",
            run.root.display()
        ));
    }
    let (Some(mut stdin), Some(stdout), Some(stderr), Some(pid)) = (
        child.stdin.take(),
        child.stdout.take(),
        child.stderr.take(),
        child.id(),
    ) else {
        let cleanup = tree.cleanup(&mut child).await;
        return Err(format!(
            "Missing child pipes/PID; cleanup={cleanup:?}; retain {}",
            run.root.display()
        ));
    };
    use chacha20poly1305::aead::rand_core::{OsRng, RngCore};
    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let challenge = Challenge {
        run_id: run.run_id.clone(),
        parent_pid: std::process::id(),
        child_pid: pid,
        nonce: nonce.iter().map(|b| format!("{b:02x}")).collect(),
    };
    let (ack_sender, ack_receiver) = tokio::sync::oneshot::channel();
    let (failure_sender, mut failure_receiver) = tokio::sync::mpsc::unbounded_channel();
    let stderr_file = files.pop().unwrap();
    let stdout_file = files.pop().unwrap();
    let failure = failure_sender.clone();
    let expected = challenge.clone();
    let mut stdout_task = tokio::spawn(async move {
        let result = capture_output(stdout, stdout_file, Some((expected, ack_sender))).await;
        if let Err(error) = &result {
            let _ = failure.send(error.clone());
        }
        result
    });
    let mut stderr_task = tokio::spawn(async move {
        let result = capture_output(stderr, stderr_file, None).await;
        if let Err(error) = &result {
            let _ = failure_sender.send(error.clone());
        }
        result
    });
    let workload = tokio::time::timeout_at(until, async {
        std::fs::create_dir_all(&plan.reports).map_err(|e| e.to_string())?;
        let launch = serde_json::json!({"schemaVersion": 1, "runId": run.run_id,
            "parentPid": std::process::id(), "childPid": pid,
            "executable": command.as_std().get_program(),
            "argv": command.as_std().get_args().collect::<Vec<_>>()});
        let mut launch_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(plan.reports.join("launch.json"))
            .await
            .map_err(|e| e.to_string())?;
        launch_file
            .write_all(launch.to_string().as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        let mut bytes = serde_json::to_vec(&challenge).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        stdin.write_all(&bytes).await.map_err(|e| e.to_string())?;
        tokio::time::timeout(Duration::from_secs(120), ack_receiver)
            .await
            .map_err(|_| "Acceptance startup handshake expired")?
            .map_err(|_| "Child did not acknowledge supervisor")?;
        stdin
            .write_all(format!("GO {}\n", challenge.nonce).as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        drop(stdin);
        tokio::select! {
            status = child.wait() => status.map_err(|e| e.to_string()),
            failure = failure_receiver.recv() => {
                match failure {
                    Some(error) => Err(error),
                    None => child.wait().await.map_err(|e| e.to_string()),
                }
            }
        }
    })
    .await;
    let cleanup = tree.cleanup(&mut child).await;
    let drain = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(&mut stdout_task, &mut stderr_task)
    })
    .await;
    let output = match drain {
        Ok((Ok(Ok(())), Ok(Ok(())))) => Ok(()),
        other => {
            stdout_task.abort();
            stderr_task.abort();
            Err(format!("Acceptance capture/drain failed: {other:?}"))
        }
    };
    let timed_out = workload.is_err();
    let exit_code = match &workload {
        Ok(Ok(status)) => status.code().unwrap_or(1),
        _ => 124,
    };
    let exit = confirmed_exit(
        run,
        OwnedExit {
            exit_code,
            leader_reaped: cleanup.is_ok(),
            tree_empty: cleanup.is_ok(),
            timed_out,
        },
        cleanup,
    )?;
    output.map_err(|error| format!("{error}; retain {}", run.root.display()))?;
    if let Ok(Err(error)) = workload {
        return Err(format!("{error}; retain {}", run.root.display()));
    }
    let receipt = serde_json::json!({"schemaVersion": 1, "runId": run.run_id,
        "parentPid": std::process::id(), "childPid": pid,
        "exitCode": exit.exit_code, "leaderReaped": exit.leader_reaped,
        "treeEmpty": exit.tree_empty, "timedOut": exit.timed_out,
        "jobAssignedBeforeResume": true,
        "elapsedMs": started.elapsed().as_millis()});
    let mut receipt_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(plan.reports.join("supervisor-exit.json"))
        .await
        .map_err(|error| {
            format!(
                "Cannot save cleanup receipt: {error}; retain {}",
                run.root.display()
            )
        })?;
    receipt_file
        .write_all(receipt.to_string().as_bytes())
        .await
        .map_err(|error| {
            format!(
                "Cannot save cleanup receipt: {error}; retain {}",
                run.root.display()
            )
        })?;
    Ok(exit)
}

#[cfg(all(windows, feature = "ipc-acceptance"))]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BuildExit {
    pub exit_code: i32,
    pub leader_reaped: bool,
    pub tree_empty: bool,
    pub timed_out: bool,
    pub job_assigned_before_resume: bool,
    pub elapsed_ms: u128,
}

/// Build command runs under the same suspended ProcessTree/Job used by acceptance children.
#[cfg(all(windows, feature = "ipc-acceptance"))]
pub async fn supervise_build(
    command: &mut Command,
    run: &AcceptanceContext,
    deadline: Duration,
) -> Result<BuildExit, String> {
    if deadline.is_zero() || deadline > Duration::from_secs(5400) {
        return Err("Acceptance build deadline must be within 5400 seconds".into());
    }
    let plan = run.resource_plan();
    if command
        .as_std()
        .get_current_dir()
        .is_none_or(|cwd| !cwd.is_absolute() || !cwd.is_dir())
    {
        return Err("Acceptance build requires an absolute existing working directory".into());
    }
    let capture_executable = command
        .as_std()
        .get_program()
        .to_str()
        .ok_or("Acceptance build executable must be valid Unicode")?
        .to_owned();
    let capture_argv = command
        .as_std()
        .get_args()
        .map(|argument| {
            argument
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "Acceptance build argument must be valid Unicode".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let capture_cwd = command
        .as_std()
        .get_current_dir()
        .and_then(|cwd| cwd.to_str())
        .ok_or("Acceptance build working directory must be valid Unicode")?
        .to_owned();
    run.claim(RunClaim::Launch)?;
    if !plan.logs.is_dir() || !plan.reports.is_dir() {
        return Err(format!(
            "Build run directories missing; retain {}",
            run.root.display()
        ));
    }
    let stdout_path = plan.logs.join("build-stdout.log");
    let stderr_path = plan.logs.join("build-stderr.log");
    let stdout_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stdout_path)
        .await
        .map_err(|error| error.to_string())?;
    let stderr_file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stderr_path)
        .await
        .map_err(|error| error.to_string())?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let (mut tree, mut child) = ProcessTree::spawn_strict(command)?;
    if let Err(error) = tree.attach(&child) {
        let cleanup = tree.cleanup(&mut child).await;
        return Err(format!(
            "{error}; cleanup={cleanup:?}; retain {}",
            run.root.display()
        ));
    }
    let child_pid = child.id().ok_or("Acceptance build PID unavailable")?;
    let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
        let cleanup = tree.cleanup(&mut child).await;
        return Err(format!(
            "Build output pipes missing; cleanup={cleanup:?}; retain {}",
            run.root.display()
        ));
    };
    let (failure_tx, mut failure_rx) = tokio::sync::mpsc::unbounded_channel();
    let stdout_tx = failure_tx.clone();
    let mut stdout_task = tokio::spawn(async move {
        let result = capture_output(stdout, stdout_file, None).await;
        if let Err(error) = &result {
            let _ = stdout_tx.send(error.clone());
        }
        result
    });
    let mut stderr_task = tokio::spawn(async move {
        let result = capture_output(stderr, stderr_file, None).await;
        if let Err(error) = &result {
            let _ = failure_tx.send(error.clone());
        }
        result
    });
    let started = tokio::time::Instant::now();
    let workload = tokio::time::timeout(deadline, async {
        tokio::select! {
            status = child.wait() => status.map_err(|error| error.to_string()),
            failure = failure_rx.recv() => match failure {
                Some(error) => Err(error),
                None => child.wait().await.map_err(|error| error.to_string()),
            },
        }
    })
    .await;
    let timed_out = matches!(workload, Err(_));
    let exit_code = match &workload {
        Ok(Ok(status)) => status.code().unwrap_or(1),
        _ => 124,
    };
    let cleanup = tree.cleanup(&mut child).await;
    let drain = tokio::time::timeout(Duration::from_secs(3), async {
        tokio::join!(&mut stdout_task, &mut stderr_task)
    })
    .await;
    let captured = matches!(drain, Ok((Ok(Ok(())), Ok(Ok(())))));
    if !captured {
        stdout_task.abort();
        stderr_task.abort();
    }
    cleanup.map_err(|error| {
        format!(
            "Build tree exit unconfirmed: {error}; retain {}",
            run.root.display()
        )
    })?;
    if !captured {
        return Err(format!(
            "Build capture/drain failed; retain {}",
            run.root.display()
        ));
    }
    if let Ok(Err(error)) = workload {
        return Err(format!(
            "Build command failed: {error}; retain {}",
            run.root.display()
        ));
    }
    let receipt = BuildExit {
        exit_code,
        leader_reaped: true,
        tree_empty: true,
        timed_out,
        job_assigned_before_resume: true,
        elapsed_ms: started.elapsed().as_millis(),
    };
    let report = serde_json::json!({
        "schemaVersion": 1,
        "runId": run.run_id,
        "parentPid": std::process::id(),
        "childPid": child_pid,
        "executable": capture_executable,
        "argv": capture_argv,
        "cwd": capture_cwd,
        "exitCode": receipt.exit_code,
        "leaderReaped": receipt.leader_reaped,
        "treeEmpty": receipt.tree_empty,
        "timedOut": receipt.timed_out,
        "jobAssignedBeforeResume": receipt.job_assigned_before_resume,
        "elapsedMs": receipt.elapsed_ms
    });
    let bytes = serde_json::to_vec(&report).map_err(|error| error.to_string())?;
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(plan.reports.join("build-capture.json"))
        .await
        .map_err(|error| {
            format!(
                "Cannot save build receipt: {error}; retain {}",
                run.root.display()
            )
        })?;
    file.write_all(&bytes).await.map_err(|error| {
        format!(
            "Cannot save build receipt: {error}; retain {}",
            run.root.display()
        )
    })?;
    Ok(receipt)
}

fn confirmed_exit(
    run: &AcceptanceContext,
    exit: OwnedExit,
    cleanup: Result<(), String>,
) -> Result<OwnedExit, String> {
    cleanup.map_err(|error| format!("Unconfirmed exit: {error}; retain {}", run.root.display()))?;
    if !exit.leader_reaped || !exit.tree_empty {
        return Err(format!("Unconfirmed exit; retain {}", run.root.display()));
    }
    Ok(exit)
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::desktop_acceptance::{tests::Fixture, AcceptanceCase};
    use std::path::Path;
    use std::time::Instant;

    fn command(run: &AcceptanceContext) -> Command {
        std::fs::create_dir_all(run.resource_plan().workspace.clone()).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args([
            "--exact",
            "desktop_acceptance::supervisor::tests::owned_child_entry",
            "--ignored",
            "--nocapture",
        ]);
        command.current_dir(run.resource_plan().workspace);
        command
    }

    fn terminated(pid: u32) -> bool {
        use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
        use windows_sys::Win32::System::Threading::{
            OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE,
        };
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if handle.is_null() {
                return std::io::Error::last_os_error().raw_os_error() == Some(87);
            }
            let exited = WaitForSingleObject(handle, 0) == WAIT_OBJECT_0;
            CloseHandle(handle);
            exited
        }
    }

    fn assert_descendant_terminated(root: &Path) {
        let pid: u32 = std::fs::read_to_string(root.join("descendant.pid"))
            .unwrap()
            .parse()
            .unwrap();
        assert!(terminated(pid), "owned descendant still running: {pid}");
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    fn test_owned_environment_opts_into_the_descriptor_transport() {
        let fixture = Fixture::new();
        let run = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        owned_environment(&mut command, &run);
        let value = command
            .as_std()
            .get_envs()
            .find(|(key, _)| *key == std::ffi::OsStr::new("KRONOS_DESKTOP_LOCAL_TRANSPORT"))
            .and_then(|(_, value)| value)
            .map(|value| value.to_string_lossy().into_owned());
        assert_eq!(
            value.as_deref(),
            Some(match run.transport {
                crate::desktop_acceptance::AcceptanceTransport::Ipc => "ipc",
                crate::desktop_acceptance::AcceptanceTransport::Websocket => "websocket",
            }),
            "the owned child never opted into the descriptor transport, so the renderer could not open a managed session"
        );
    }

    #[cfg(feature = "ipc-acceptance")]
    #[tokio::test]
    async fn test_build_capture_owns_job_and_bounded_logs() {
        let fixture = Fixture::new();
        let run = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let plan = run.resource_plan();
        for directory in [&plan.workspace, &plan.logs, &plan.reports] {
            std::fs::create_dir_all(directory).unwrap();
        }
        let retained_root = fixture.retain();
        let parent_environment: Vec<_> = std::env::vars_os().collect();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "desktop_acceptance::supervisor::tests::owned_build_probe",
                "--ignored",
                "--nocapture",
            ])
            .current_dir(run.resource_plan().workspace);
        let exit = supervise_build(&mut command, &run, Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(exit.exit_code, 0);
        assert!(exit.leader_reaped && exit.tree_empty && !exit.timed_out);
        assert!(exit.job_assigned_before_resume);
        assert!(
            std::fs::read_to_string(run.resource_plan().logs.join("build-stdout.log"))
                .unwrap()
                .contains("BUILD_CAPTURE_PROBE")
        );
        let report: serde_json::Value = serde_json::from_slice(
            &std::fs::read(run.resource_plan().reports.join("build-capture.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(report["jobAssignedBeforeResume"], true);
        assert_eq!(report["treeEmpty"], true);
        assert_eq!(report["exitCode"], 0);
        assert!(
            report["executable"].is_string(),
            "capture executable must be a string"
        );
        assert!(
            report["argv"]
                .as_array()
                .is_some_and(|argv| argv.iter().all(serde_json::Value::is_string)),
            "capture arguments must be strings"
        );
        assert!(
            report["cwd"].is_string(),
            "capture working directory must be a string"
        );
        assert_eq!(std::env::vars_os().collect::<Vec<_>>(), parent_environment);
        std::fs::remove_dir_all(retained_root).unwrap();
    }

    #[cfg(feature = "ipc-acceptance")]
    #[tokio::test]
    #[ignore = "receipt-bound private desktop build; explicit gated invocation only"]
    async fn test_acceptance_private_build_capture() {
        use std::path::PathBuf;
        assert_eq!(
            std::env::var("KRONOS_IPC_ACCEPTANCE_BUILD_MODE")
                .ok()
                .as_deref(),
            Some("1")
        );
        let descriptor = std::env::var_os("KRONOS_IPC_ACCEPTANCE_DESCRIPTOR")
            .expect("private build descriptor missing");
        let run = AcceptanceContext::from_descriptor(Path::new(&descriptor)).unwrap();
        assert_eq!(run.case_id, super::super::AcceptanceCase::Preflight);
        let node = argument_path(
            &PathBuf::from(
                std::env::var_os("KRONOS_IPC_ACCEPTANCE_BUILD_NODE")
                    .expect("private build Node path missing"),
            )
            .canonicalize()
            .unwrap(),
        );
        let desktop = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("desktop package directory missing")
            .to_path_buf();
        let repository = desktop.parent().unwrap().parent().unwrap();
        let tauri_cli = repository.join("node_modules/@tauri-apps/cli/tauri.js");
        assert!(tauri_cli.is_file(), "installed Tauri CLI missing");
        let plan = run.resource_plan();
        std::fs::create_dir_all(plan.logs).unwrap();
        std::fs::create_dir_all(&plan.reports).unwrap();
        let build_parent = run.root.parent().expect("Acceptance run root has no build parent");
        assert_eq!(
            build_parent.file_name().unwrap(),
            std::ffi::OsStr::new("builds"),
            "Acceptance build root must stay short and repository-local"
        );
        std::fs::create_dir_all(&build_parent).unwrap();
        // `run.root` is canonicalized, so on Windows it is a verbatim (`\\?\`) path.
        // Cargo would propagate that spelling into OUT_DIR and CMake would hand it to
        // cl.exe, which cannot open verbatim paths. Cross the child boundary in DOS form.
        let target = argument_path(&build_parent.join(run.run_id.clone()));
        let compiler_output = target.join(
            r"release\build\zstd-sys-561478f143bf0047\out\fb80479a5fb81f6a-zstd_compress_superblock.o",
        );
        let compiler_path_length = compiler_output.to_string_lossy().len();
        let cmake_probe_length = target
            .join(r"release/build/whisper-rs-sys-fb17adba72a7f8e0/out/build/CMakeFiles/CMakeScratch/TryCompile-abcdefgh-ijklmnop/CMakeFiles/cmTC_123456.dir/12345678.obj")
            .to_string_lossy()
            .len();
        assert!(
            compiler_path_length < 260 && cmake_probe_length < 250,
            "MSVC/CMake output paths exceed compiler limits (lengths {compiler_path_length}/{cmake_probe_length}; MSVC<260, CMake<250)"
        );
        std::fs::create_dir_all(&target).unwrap();
        let build_path = std::env::var_os("KRONOS_IPC_ACCEPTANCE_BUILD_PATH")
            .expect("private build PATH missing");
        let cargo_home = std::env::var_os("KRONOS_IPC_ACCEPTANCE_BUILD_CARGO_HOME")
            .expect("private build Cargo home missing");
        let rustup_home = std::env::var_os("KRONOS_IPC_ACCEPTANCE_BUILD_RUSTUP_HOME")
            .expect("private build Rustup home missing");
        assert!(
            Path::new(&build_path).is_absolute(),
            "private build PATH must be absolute"
        );
        assert!(
            Path::new(&cargo_home).is_absolute(),
            "private build Cargo home must be absolute"
        );
        assert!(
            Path::new(&rustup_home).is_absolute(),
            "private build Rustup home must be absolute"
        );
        let mut command = Command::new(node);
        command.env_clear();
        for key in [
            "SystemRoot",
            "SystemDrive",
            "WINDIR",
            "COMSPEC",
            "PATHEXT",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "CommonProgramFiles",
            "CommonProgramFiles(x86)",
            "OS",
            "PROCESSOR_ARCHITECTURE",
            "PROCESSOR_IDENTIFIER",
            "PROCESSOR_LEVEL",
            "PROCESSOR_REVISION",
            "NUMBER_OF_PROCESSORS",
            "INCLUDE",
            "LIB",
            "LIBPATH",
            "VCINSTALLDIR",
            "VCToolsInstallDir",
            "VCToolsVersion",
            "VSINSTALLDIR",
            "VisualStudioVersion",
            "WindowsSdkDir",
            "WindowsSDKVersion",
            "WindowsSDKLibVersion",
            "UniversalCRTSdkDir",
            "UCRTVersion",
            "VSCMD_ARG_TGT_ARCH",
            "VSCMD_ARG_HOST_ARCH",
            "VSCMD_VER",
        ] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        if let Some(install) = std::env::var_os("VSINSTALLDIR") {
            // The cmake crate prefers `CMAKE`; a PATH CMake (MinGW) rejects the MSVC toolchain.
            let cmake = PathBuf::from(&install)
                .join(r"Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe");
            if cmake.is_file() {
                command.env("CMAKE", argument_path(&cmake));
            }
        }
        command
            .env("PATH", build_path)
            .env("CARGO_HOME", cargo_home)
            .env("RUSTUP_HOME", rustup_home)
            .arg(tauri_cli)
            .args([
                "build",
                "--no-bundle",
                "--features",
                "ipc-acceptance",
                "--ci",
            ])
            .current_dir(&desktop)
            .env("CI", "true")
            .env("CARGO_BUILD_JOBS", "4")
            .env("CARGO_NET_OFFLINE", "true")
            .env("CMAKE_GENERATOR", "NMake Makefiles")
            .env("CARGO_TARGET_DIR", &target)
            .env("CMAKE_OBJECT_PATH_MAX", "250")
            .env("KRONOS_HOME", argument_path(&plan.home))
            .env("KRONOS_WITH_NODE", "0")
            .env("PASEO_HOME", argument_path(&plan.home))
            .env("HOME", argument_path(&plan.home))
            .env("USERPROFILE", argument_path(&plan.home))
            .env("APPDATA", argument_path(&plan.home.join("AppData/Roaming")))
            .env(
                "LOCALAPPDATA",
                argument_path(&plan.home.join("AppData/Local")),
            )
            .env(
                "NPM_CONFIG_USERCONFIG",
                argument_path(&plan.home.join(".npmrc")),
            )
            .env(
                "NPM_CONFIG_GLOBALCONFIG",
                argument_path(&plan.home.join("npm-globalrc")),
            )
            .env(
                "NPM_CONFIG_CACHE",
                argument_path(&run.root.join("npm-cache")),
            )
            .env("NPM_CONFIG_OFFLINE", "true")
            .env("EXPO_OFFLINE", "1")
            .env("EXPO_NO_TELEMETRY", "1")
            .env("TEMP", argument_path(&plan.temporary))
            .env("TMP", argument_path(&plan.temporary))
            .env("TMPDIR", argument_path(&plan.temporary))
            .env_remove("CARGO_PROFILE_TEST_OPT_LEVEL")
            .env_remove("CARGO_PROFILE_TEST_DEBUG")
            .env_remove("CARGO_INCREMENTAL")
            .env_remove("CLEAR_CACHE")
            .env_remove("EXPO_CLEAR_CACHE")
            .env_remove("NODE_OPTIONS")
            .env_remove("KRONOS_DAEMON_ENGINE");
        let exit = supervise_build(&mut command, &run, Duration::from_secs(5400))
            .await
            .unwrap_or_else(|error| {
                panic!(
                    "private build capture failed; retain {}: {error}",
                    run.root.display()
                )
            });
        assert_eq!(
            exit.exit_code,
            0,
            "private Tauri build failed; retain {}",
            run.root.display()
        );
        assert!(!exit.timed_out && exit.leader_reaped && exit.tree_empty);
        assert!(exit.job_assigned_before_resume);
        let executable = target.join("release/paseo-desktop.exe");
        assert!(
            executable.is_file(),
            "private executable missing: {}",
            executable.display()
        );
        println!("BUILD_CAPTURE_ROOT={}", run.root.display());
        println!("BUILD_CAPTURE_EXE={}", executable.display());
    }

    #[cfg(feature = "ipc-acceptance")]
    #[test]
    #[ignore = "owned output-capture probe only; supervisor launches it under a Job"]
    fn owned_build_probe() {
        println!("BUILD_CAPTURE_PROBE");
    }

    #[tokio::test]
    async fn test_supervisor_owns_descendants_before_resume() {
        // Resuming outside the owned Job would let this immediate descendant escape cleanup.
        let fixture = Fixture::new();
        let run = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let retained_root = fixture.retain();
        let environment: Vec<_> = std::env::vars_os().collect();
        let exit = supervise(&mut command(&run), &run, Duration::from_secs(10)).await;
        assert!(exit.is_ok(), "{exit:?}");
        let exit = exit.unwrap();
        assert_eq!(exit.exit_code, 0);
        assert!(exit.leader_reaped && exit.tree_empty && !exit.timed_out);
        let receipt_path = run.resource_plan().reports.join("supervisor-exit.json");
        assert!(receipt_path.is_file(), "missing native cleanup receipt");
        let receipt: serde_json::Value =
            serde_json::from_slice(&std::fs::read(receipt_path).unwrap()).unwrap();
        let launch: serde_json::Value = serde_json::from_slice(
            &std::fs::read(run.resource_plan().reports.join("launch.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["childPid"], launch["childPid"]);
        assert_eq!(receipt["runId"], run.run_id);
        assert_eq!(receipt["leaderReaped"], true);
        assert_eq!(receipt["treeEmpty"], true);
        assert_eq!(receipt["jobAssignedBeforeResume"], true);
        assert_eq!(
            std::fs::read_to_string(run.root.join("owned-before-descendant")).unwrap(),
            "true"
        );
        assert_descendant_terminated(&run.root);
        assert!(
            std::env::vars_os().collect::<Vec<_>>() == environment,
            "parent environment changed"
        );
        std::fs::remove_dir_all(retained_root).unwrap();
    }

    #[tokio::test]
    async fn test_supervisor_deadline_is_wall_clock() {
        // Missing timeout must not let a 20 s leader/descendant outlive the 1 s budget.
        let fixture = Fixture::new();
        let mut run = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        run.case_id = AcceptanceCase::Backpressure;
        let mut descriptor: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&fixture.descriptor).unwrap()).unwrap();
        descriptor["caseId"] = serde_json::json!("backpressure");
        std::fs::write(&fixture.descriptor, descriptor.to_string()).unwrap();
        let retained_root = fixture.retain();
        let start = Instant::now();
        let exit = supervise(&mut command(&run), &run, Duration::from_secs(1)).await;
        assert!(exit.is_ok(), "{exit:?}");
        let exit = exit.unwrap();
        assert!(exit.timed_out);
        assert!(exit.leader_reaped);
        assert!(exit.tree_empty);
        assert!(start.elapsed() <= Duration::from_secs(7));
        assert_descendant_terminated(&run.root);
        crate::println!(
            "timed_out={} leader_reaped={} tree_empty={} elapsed_ms={}",
            exit.timed_out,
            exit.leader_reaped,
            exit.tree_empty,
            start.elapsed().as_millis()
        );
        std::fs::remove_dir_all(retained_root).unwrap();
    }

    #[test]
    fn test_supervisor_reap_failure_retains_home() {
        // Suppressing a cleanup error must not permit the driver to delete unconfirmed state.
        let fixture = Fixture::new();
        std::fs::write(fixture.root.join("home/sentinel"), "retain").unwrap();
        let run = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let result = confirmed_exit(
            &run,
            OwnedExit {
                exit_code: 0,
                leader_reaped: true,
                tree_empty: true,
                timed_out: false,
            },
            Err("injected unconfirmed descendant reap".into()),
        );
        assert!(result.is_err(), "unconfirmed exit was accepted");
        let sentinel = run.root.join("home/sentinel");
        let retained_root = fixture.retain();
        assert!(sentinel.is_file(), "fixture deleted unconfirmed state");
        crate::println!(
            "injected-only unconfirmed state retained: {}",
            retained_root.display()
        );
        assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "retain");
    }

    #[tokio::test]
    async fn test_acceptance_resources_handoff_in_owned_child() {
        let fixture = Fixture::new();
        std::fs::create_dir_all(fixture.root.join("workspace")).unwrap();
        std::fs::write(fixture.root.join("scenario.js"), "void 0;").unwrap();
        let run = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let retained_root = fixture.retain();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "desktop_acceptance::supervisor::tests::owned_resources_child_entry",
                "--nocapture",
            ])
            .current_dir(run.resource_plan().workspace);
        let exit = supervise(&mut command, &run, Duration::from_secs(10))
            .await
            .unwrap();
        assert_eq!(
            exit.exit_code,
            0,
            "owned resource initialization failed; retain {}",
            run.root.display()
        );
        assert!(exit.leader_reaped && exit.tree_empty && !exit.timed_out);
        assert_eq!(
            std::fs::read_to_string(run.root.join("reports/resources.txt")).unwrap(),
            "owned-rings-shared;pipe-claim-held"
        );
        std::fs::remove_dir_all(retained_root).unwrap();
    }

    #[tokio::test]
    async fn test_acceptance_child_bootstrap_installs_owned_startup() {
        // The supervised child must reach real desktop startup with owned resources,
        // never the previous "startup is not wired" rejection.
        let fixture = Fixture::new();
        std::fs::create_dir_all(fixture.root.join("workspace")).unwrap();
        std::fs::write(fixture.root.join("scenario.js"), "void 0;").unwrap();
        let run = AcceptanceContext::from_descriptor(&fixture.descriptor).unwrap();
        let retained_root = fixture.retain();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--ignored",
                "--exact",
                "desktop_acceptance::supervisor::tests::owned_bootstrap_child_entry",
                "--nocapture",
            ])
            .current_dir(run.resource_plan().workspace);
        let exit = supervise(&mut command, &run, Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(
            exit.exit_code,
            0,
            "supervised child bootstrap failed; retain {}",
            run.root.display()
        );
        assert!(exit.leader_reaped && exit.tree_empty && !exit.timed_out);
        assert_eq!(
            std::fs::read_to_string(run.root.join("reports/bootstrap.txt")).unwrap(),
            "continued;owned-pipe"
        );
        std::fs::remove_dir_all(retained_root).unwrap();
    }

    #[test]
    #[ignore = "owned finite subprocess only; never run in parent test process"]
    fn owned_bootstrap_child_entry() {
        let descriptor = std::env::var_os("KRONOS_IPC_ACCEPTANCE_DESCRIPTOR").unwrap();
        let arguments: Vec<std::ffi::OsString> = vec![
            "paseo-desktop".into(),
            "--ipc-acceptance-child".into(),
            descriptor,
        ];
        let action =
            super::super::bootstrap(&arguments).expect("supervised child bootstrap failed");
        assert!(matches!(action, super::super::BootAction::Continue));
        let context = super::super::active().expect("bootstrap did not install the owned context");
        assert!(!super::super::os_integrations_enabled());
        assert_eq!(super::super::scenario_source(), "void 0;");
        let plan = context.resource_plan();
        let mut config: tauri::utils::config::Config =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        super::super::configure_startup(&mut config).unwrap();
        assert_eq!(config.identifier, plan.identifier);
        assert_eq!(
            config.app.app_directories_override,
            Some(tauri::utils::config::AppDirectoriesOverride::Root(
                plan.home.clone()
            ))
        );
        assert!(config.app.windows.is_empty());
        assert_eq!(
            super::super::acceptance_pipe_name().as_deref(),
            Some(plan.pipe.as_str())
        );
        assert_ne!(plan.pipe, r"\\.\pipe\kronos-ipc");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let server = super::super::acceptance_pipe_server(&plan.pipe)
                .unwrap()
                .expect("owned pipe lease was not handed to the listener");
            assert!(
                super::super::acceptance_pipe_server(&plan.pipe).is_err(),
                "a live listener lease was reopened"
            );
            assert!(
                super::super::acceptance_pipe_server(r"\\.\pipe\kronos-ipc").is_err(),
                "the canonical pipe was claimable during acceptance"
            );
            drop(server);
            assert!(
                super::super::acceptance_pipe_server(&plan.pipe)
                    .unwrap()
                    .is_some(),
                "a confirmed owned drop was not re-claimable"
            );
        });
        std::fs::write(plan.reports.join("bootstrap.txt"), "continued;owned-pipe").unwrap();
    }

    #[test]
    #[ignore = "owned finite subprocess only; never run in parent test process"]
    fn owned_resources_child_entry() {
        let descriptor = std::env::var_os("KRONOS_IPC_ACCEPTANCE_DESCRIPTOR").unwrap();
        let run = AcceptanceContext::from_descriptor(std::path::Path::new(&descriptor)).unwrap();
        run.validate_environment(
            std::env::var_os("KRONOS_HOME").as_deref(),
            std::env::var_os("PASEO_HOME").as_deref(),
        )
        .unwrap();
        child_handshake(&run).unwrap();
        run.claim_resources().unwrap();
        let plan = run.resource_plan();
        super::super::install_context(run).unwrap();
        assert!(!super::super::os_integrations_enabled());
        let first = crate::kernel::server::get_shm_ring();
        let second = crate::kernel::server::get_shm_ring();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert!(crate::kernel::shm::ShmRingBuffer::new_exclusive(
            &plan.daemon_mapping,
            2 * 1024 * 1024
        )
        .is_err());
        assert!(crate::kernel::shm::ShmRingBuffer::new_exclusive(
            &plan.telemetry_mapping,
            64 * 1024
        )
        .is_err());
        crate::kernel::shm::record_global_shm_metric(1, 1);
        assert_eq!(crate::kernel::shm::get_global_shm_stats().0, 1);
        assert!(crate::kernel::server::PipeLease::claim(&plan.pipe).is_err());
        std::fs::write(
            super::super::active().unwrap().root.join("scenario.js"),
            "changed after immutable load",
        )
        .unwrap();
        assert_eq!(super::super::scenario_source(), "void 0;");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            use std::os::windows::io::IntoRawHandle;
            let handle = super::super::active().unwrap().claim_pipe().unwrap();
            let server = unsafe {
                tokio::net::windows::named_pipe::NamedPipeServer::from_raw_handle(
                    handle.into_raw_handle(),
                )
            }
            .unwrap();
            assert!(
                super::super::active().unwrap().claim_pipe().is_err(),
                "live listener lease was reopened"
            );
            drop(server);
            drop(
                super::super::active()
                    .unwrap()
                    .claim_pipe()
                    .expect("exclusive reacquire after confirmed owned drop"),
            );
        });
        std::fs::write(
            plan.reports.join("resources.txt"),
            "owned-rings-shared;pipe-claim-held",
        )
        .unwrap();
    }

    #[test]
    #[ignore = "owned finite subprocess only; never run in parent test process"]
    fn owned_child_entry() {
        let descriptor = std::env::var_os("KRONOS_IPC_ACCEPTANCE_DESCRIPTOR").unwrap();
        let run = AcceptanceContext::from_descriptor(Path::new(&descriptor)).unwrap();
        run.validate_environment(
            std::env::var_os("KRONOS_HOME").as_deref(),
            std::env::var_os("PASEO_HOME").as_deref(),
        )
        .unwrap();
        child_handshake(&run).unwrap();
        let mut assigned = 0;
        unsafe {
            use windows_sys::Win32::System::{JobObjects::IsProcessInJob, Threading::GetCurrentProcess};
            assert_ne!(
                IsProcessInJob(GetCurrentProcess(), std::ptr::null_mut(), &mut assigned),
                0
            );
        }
        assert_ne!(assigned, 0);
        std::fs::write(run.root.join("owned-before-descendant"), "true").unwrap();
        let mut descendant = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "desktop_acceptance::supervisor::tests::owned_descendant_entry",
                "--ignored",
                "--nocapture",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        std::fs::write(run.root.join("descendant.pid"), descendant.id().to_string()).unwrap();
        if run.case_id == AcceptanceCase::Backpressure {
            std::thread::sleep(Duration::from_secs(20));
        }
        // Natural leader exit deliberately leaves its finite descendant to supervisor cleanup.
        let _ = descendant.try_wait().unwrap();
    }

    #[test]
    #[ignore = "owned finite descendant only"]
    fn owned_descendant_entry() {
        std::thread::sleep(Duration::from_secs(20));
    }
}
