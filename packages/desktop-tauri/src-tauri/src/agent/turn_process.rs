use std::time::Duration;
use tokio::process::{Child, Command};

/// One tree per foreground turn, never the desktop-wide terminal job.
pub(super) struct ProcessTree {
    #[cfg(windows)]
    job: Option<crate::process_job::JobSupervisor>,
    #[cfg(windows)]
    resumed: bool,
    #[cfg(unix)]
    group: Option<i32>,
}

impl ProcessTree {
    /// Capture group ownership in the same synchronous operation as spawn, before
    /// any persistence or other fallible startup work can run.
    pub fn spawn(command: &mut Command) -> Result<(Self, Child), String> {
        let tree = Self::prepare(command)?;
        let child = command
            .spawn()
            .map_err(|error| format!("Failed to spawn provider: {error}"))?;
        #[cfg(unix)]
        let mut tree = tree;
        #[cfg(unix)]
        {
            tree.group = child.id().map(|id| id as i32);
        }
        Ok((tree, child))
    }

    fn prepare(command: &mut Command) -> Result<Self, String> {
        #[cfg(windows)]
        {
            // Assign before any provider code executes, so even immediate descendants are owned.
            command.creation_flags(0x08000000 | 0x00000004); // NO_WINDOW | SUSPENDED
            Ok(Self {
                job: Some(crate::process_job::JobSupervisor::new()?),
                resumed: false,
            })
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.as_std_mut().process_group(0);
            Ok(Self { group: None })
        }
        #[cfg(not(any(windows, unix)))]
        {
            let _ = command;
            Err("Provider process-tree ownership is unsupported on this platform".into())
        }
    }

    pub fn attach(&mut self, child: &Child) -> Result<(), String> {
        #[cfg(windows)]
        {
            let handle = child
                .raw_handle()
                .ok_or("Provider process handle unavailable")?;
            self.job
                .as_ref()
                .ok_or("Provider job unavailable")?
                .assign_process_raw(handle)?;
            self.resume_handle(handle)?;
        }
        #[cfg(unix)]
        {
            // spawn already captured the live process group before fallible setup.
            let _ = child;
        }
        Ok(())
    }

    #[cfg(windows)]
    fn resume_handle(&mut self, handle: *mut std::ffi::c_void) -> Result<(), String> {
        // Tokio retains the process handle, but not the initial thread handle.
        #[link(name = "ntdll")]
        extern "system" {
            fn NtResumeProcess(process: *mut std::ffi::c_void) -> i32;
        }
        let status = unsafe { NtResumeProcess(handle) };
        if status < 0 {
            return Err(format!(
                "Failed to resume owned provider process: NTSTATUS {status:#x}"
            ));
        }
        self.resumed = true;
        Ok(())
    }

    #[cfg(windows)]
    pub fn was_suspended(&self) -> bool {
        !self.resumed
    }

    pub fn terminate(&mut self) -> Result<(), String> {
        #[cfg(windows)]
        if let Some(job) = &self.job {
            job.terminate()?;
        }
        #[cfg(unix)]
        if let Some(group) = self.group {
            extern "C" {
                fn kill(pid: i32, signal: i32) -> i32;
            }
            if unsafe { kill(-group, 9) } != 0 {
                let error = std::io::Error::last_os_error();
                if error.raw_os_error() != Some(3) {
                    return Err(format!(
                        "Failed to terminate provider process group: {error}"
                    ));
                }
            }
        }
        Ok(())
    }

    pub async fn cleanup(&mut self, child: &mut Child) -> Result<(), String> {
        let termination = self.terminate();
        // The leader may already be reaped. start_kill is harmless in that case.
        let _ = child.start_kill();
        tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .map_err(|_| "Provider process did not exit within cleanup deadline".to_string())?
            .map_err(|e| format!("Failed to reap provider: {e}"))?;
        termination?;
        #[cfg(windows)]
        if let Some(job) = &self.job {
            tokio::time::timeout(Duration::from_secs(3), async {
                while job.active_processes()? != 0 {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                Ok::<(), String>(())
            })
            .await
            .map_err(|_| {
                "Provider descendants did not exit within cleanup deadline".to_string()
            })??;
        }
        #[cfg(unix)]
        {
            self.group = None;
        }
        Ok(())
    }
}

impl Drop for ProcessTree {
    fn drop(&mut self) {
        let _ = self.terminate();
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::agent::manager::AgentManager;
    use crate::agent::types::AgentStatus;
    use std::process::Stdio;

    enum FailurePhase {
        Assignment,
        Resume,
    }

    async fn failed_suspended_setup_restores_continuity_and_retries(phase: FailurePhase) {
        let directory = tempfile::tempdir().unwrap();
        let manager = AgentManager::new(directory.path());
        let agent = manager
            .create_agent(
                "codex",
                directory.path().to_str().unwrap(),
                None,
                None,
                None,
            )
            .unwrap();
        let mut turn = manager
            .send_message(&agent.id, "fails before provider execution")
            .unwrap();
        let prior = turn.agent.clone();
        let execution_marker = directory.path().join("provider-executed");
        let script = r#"
            require('fs').writeFileSync(process.env.PASEO_EXECUTION_MARKER, 'ran');
            console.log(JSON.stringify({type:'thread.started',thread_id:'recovered-thread'}));
            console.log(JSON.stringify({type:'item.completed',item:{id:'answer',type:'agent_message',text:'retry succeeded'}}));
        "#;
        let command = || {
            let mut command =
                Command::new(crate::agent::registry::resolve_command("node").unwrap());
            command
                .args(["-e", script, "--"])
                .env("PASEO_EXECUTION_MARKER", &execution_marker)
                .current_dir(directory.path())
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true);
            command
        };
        let (mut tree, mut child) = ProcessTree::spawn(&mut command()).unwrap();
        turn.mark_provider_started().unwrap();
        assert_eq!(
            manager
                .get_agent(&agent.id)
                .unwrap()
                .runtime_info
                .as_ref()
                .unwrap()["continuityRequired"],
            true
        );
        let error = match phase {
            // Exercise actual Win32 error returns without allowing the suspended child to run.
            FailurePhase::Assignment => tree
                .job
                .as_ref()
                .unwrap()
                .assign_process_raw(std::ptr::null_mut())
                .unwrap_err(),
            FailurePhase::Resume => {
                tree.job
                    .as_ref()
                    .unwrap()
                    .assign_process_raw(child.raw_handle().unwrap())
                    .unwrap();
                tree.resume_handle(std::ptr::null_mut()).unwrap_err()
            }
        };
        assert!(tree.was_suspended());
        let output = super::super::cleanup_failed_provider_start(
            &mut turn, &mut tree, &mut child, &prior, error,
        )
        .await;
        assert!(output.cleanup_confirmed);
        assert!(
            !execution_marker.exists(),
            "A suspended failed setup must not execute provider code"
        );
        assert_eq!(
            manager.get_agent(&agent.id).unwrap().runtime_info,
            prior.runtime_info
        );
        turn.finish(output.error.as_deref()).unwrap();
        let reloaded = AgentManager::try_new(directory.path()).unwrap();
        let retry = reloaded.send_message(&agent.id, "retry").unwrap();
        tokio::time::timeout(
            Duration::from_secs(10),
            super::super::run_reserved_turn(retry, Some(command())),
        )
        .await
        .unwrap();
        let recovered = reloaded.get_agent(&agent.id).unwrap();
        assert!(execution_marker.exists());
        assert_eq!(recovered.last_status, AgentStatus::Idle);
        assert_eq!(
            super::super::provider_handle(&recovered)
                .unwrap()
                .as_deref(),
            Some("recovered-thread")
        );
    }

    #[tokio::test]
    async fn windows_assignment_failure_restores_fresh_continuity_and_retries() {
        failed_suspended_setup_restores_continuity_and_retries(FailurePhase::Assignment).await;
    }

    #[tokio::test]
    async fn windows_resume_failure_restores_fresh_continuity_and_retries() {
        failed_suspended_setup_restores_continuity_and_retries(FailurePhase::Resume).await;
    }
}
