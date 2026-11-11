use super::*;
use serde_json::json;

#[test]
fn codex_completed_items_preserve_reasoning_and_answer_without_replaying_snapshots() {
    let mut output = ProviderOutput::default();
    let mut live = Vec::new();
    for event in [
        json!({"type":"item.started","item":{"id":"r1","type":"reasoning","text":"Plan"}}),
        json!({"type":"item.updated","item":{"id":"r1","type":"reasoning","text":"Plan carefully"}}),
        json!({"type":"item.completed","item":{"id":"r1","type":"reasoning","text":"Plan carefully"}}),
        json!({"type":"item.completed","item":{"id":"a1","type":"agent_message","text":"Answer"}}),
        json!({"type":"item.completed","item":{"id":"r2","type":"reasoning","text":"Check"}}),
        json!({"type":"item.completed","item":{"id":"a2","type":"agent_message","text":"Final"}}),
    ] {
        live.extend(output.ingest("codex", &event));
    }
    assert_eq!(
        output
            .rows
            .iter()
            .map(|r| (r.kind, r.text.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (OutputKind::Reasoning, "Plan carefully"),
            (OutputKind::Assistant, "Answer"),
            (OutputKind::Reasoning, "Check"),
            (OutputKind::Assistant, "Final"),
        ]
    );
    assert_eq!(
        live.iter()
            .map(|v| v["text"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Plan", " carefully", "Answer", "Check", "Final"]
    );
}

#[test]
fn opencode_reasoning_parts_are_not_discarded_or_duplicated() {
    let mut output = ProviderOutput::default();
    let event = json!({"type":"reasoning","sessionID":"s","part":{"id":"p1","messageID":"m1","type":"reasoning","text":"Consider"}});
    assert_eq!(output.ingest("opencode", &event)[0]["text"], "Consider");
    assert!(output.ingest("opencode", &event).is_empty());
    output.ingest(
        "opencode",
        &json!({"type":"text","part":{"id":"p2","messageID":"m1","type":"text","text":"OK"}}),
    );
    assert_eq!(output.rows.len(), 2);
    assert_eq!(output.rows[0].kind, OutputKind::Reasoning);
    assert_eq!(output.rows[0].text, "Consider");
    assert_eq!(output.rows[1].text, "OK");
}

#[test]
fn pi_final_content_reconciles_each_block_and_keeps_following_messages() {
    for provider in ["pi", "omp"] {
        let mut output = ProviderOutput::default();
        output.ingest(provider, &json!({"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":"Think"}}));
        output.ingest(provider, &json!({"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":0,"delta":" more"}}));
        output.ingest(provider, &json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":1,"delta":"Par"}}));
        let suffix = output.ingest(provider, &json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Think more"},{"type":"text","text":"Partial"},{"type":"text","text":" second"}]}}));
        assert_eq!(
            suffix
                .iter()
                .map(|v| v["text"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["tial", " second"]
        );
        output.ingest(provider, &json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Next"},{"type":"text","text":"Followup"}]}}));
        assert_eq!(
            output
                .rows
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Think more", "Partial", " second", "Next", "Followup"]
        );
    }
}

#[test]
fn claude_partial_thinking_and_final_blocks_do_not_duplicate_or_leak_signatures() {
    let mut output = ProviderOutput::default();
    output.ingest(
        "claude",
        &json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"m1"}}}),
    );
    let first = output.ingest("claude", &json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Plan"}}}));
    output.ingest("claude", &json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"signature_delta","signature":"secret"}}}));
    let second = output.ingest("claude", &json!({"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"OK"}}}));
    assert!(output.ingest("claude", &json!({"type":"assistant","message":{"id":"m1","content":[{"type":"thinking","thinking":"Plan","signature":"secret"},{"type":"text","text":"OK"}]}})).is_empty());
    assert_eq!(first[0]["type"], "reasoning");
    assert_eq!(second[0]["type"], "assistant_message");
    assert_ne!(first[0]["messageId"], second[0]["messageId"]);
    assert_eq!(output.rows.len(), 2);
    assert_eq!(output.rows[0].text, "Plan");
}

#[test]
fn live_deltas_do_not_claim_committed_row_sequences_or_repeat_cumulative_text() {
    let mut output = ProviderOutput::default();
    let a = output.ingest("pi", &json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"A"}}));
    let b = output.ingest("pi", &json!({"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"B"}}));
    let a = stream_message("agent", "pi", "turn-1", &a[0]);
    let b = stream_message("agent", "pi", "turn-1", &b[0]);
    assert_eq!(a["message"]["payload"]["event"]["item"]["text"], "A");
    assert_eq!(b["message"]["payload"]["event"]["item"]["text"], "B");
    assert!(a["message"]["payload"].get("seq").is_none());
    assert!(b["message"]["payload"].get("epoch").is_none());
    assert_eq!(
        a["message"]["payload"]["event"]["item"]["messageId"],
        b["message"]["payload"]["event"]["item"]["messageId"]
    );
    assert_eq!(a["message"]["payload"]["event"]["turnId"], "turn-1");
    let next = ProviderOutput::default();
    assert_ne!(output.identity, next.identity);
}

#[test]
fn cli_thinking_options_use_native_arguments_and_default_variant_is_omitted() {
    for provider in ["omp", "pi"] {
        let args = provider_arguments(
            provider,
            Some("provider/model"),
            Some("high"),
            Path::new("."),
            "Prompt",
            None,
            Some(Path::new("isolated")),
        )
        .unwrap();
        assert!(args.windows(2).any(|pair| pair == ["--thinking", "high"]));
    }
    let args = provider_arguments(
        "opencode",
        None,
        Some("default"),
        Path::new("."),
        "Prompt",
        None,
        None,
    )
    .unwrap();
    assert!(args.iter().any(|arg| arg == "--thinking"));
    assert!(args.iter().any(|arg| arg == "--format"));
    assert!(!args.iter().any(|arg| arg == "--variant"));
    let low_args = provider_arguments(
        "opencode",
        None,
        Some("low"),
        Path::new("."),
        "Prompt",
        None,
        None,
    )
    .unwrap();
    assert!(!low_args.iter().any(|arg| arg == "--variant"));
    let opencode_high = provider_arguments(
        "opencode",
        None,
        Some("high"),
        Path::new("."),
        "Prompt",
        None,
        None,
    )
    .unwrap();
    assert!(opencode_high
        .windows(2)
        .any(|pair| pair == ["--variant", "high"]));
    let args = provider_arguments(
        "codex",
        None,
        Some("xhigh"),
        Path::new("."),
        "Prompt",
        None,
        None,
    )
    .unwrap();
    assert!(args
        .windows(2)
        .any(|pair| pair == ["-c", "model_reasoning_effort=\"xhigh\""]));
    let args = provider_arguments(
        "claude",
        Some("model"),
        Some("high"),
        Path::new("."),
        "Prompt",
        None,
        None,
    )
    .unwrap();
    assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
    assert!(args.iter().any(|arg| arg == "--include-partial-messages"));
}

#[test]
fn errors_keep_real_partial_reasoning_available_for_persistence() {
    let mut output = ProviderOutput::default();
    output.ingest(
        "codex",
        &json!({"type":"item.completed","item":{"id":"r","type":"reasoning","text":"Partial"}}),
    );
    output.ingest(
        "codex",
        &json!({"type":"turn.failed","error":{"message":"Failure"}}),
    );
    assert_eq!(output.rows[0].text, "Partial");
    assert_eq!(output.error.as_deref(), Some("Failure"));
}

#[test]
fn successful_pi_omp_retries_do_not_report_a_recovered_error() {
    for provider in ["pi", "omp"] {
        let mut output = ProviderOutput::default();
        output.ingest(provider, &json!({"type":"message_end","message":{"role":"assistant","content":[],"errorMessage":"timeout"}}));
        assert_eq!(output.error.as_deref(), Some("timeout"));
        output.ingest(provider, &json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"Recovered answer"}]}}));
        output.ingest(provider, &json!({"type":"auto_retry_end","success":true}));
        assert!(output.error.is_none());
        assert_eq!(output.rows[0].text, "Recovered answer");
        output.ingest(provider, &json!({"type":"message_end","message":{"role":"assistant","content":[],"errorMessage":"Final failure"}}));
        assert_eq!(output.error.as_deref(), Some("Final failure"));
    }
}

#[test]
fn opencode_cancelled_input_request_does_not_fail_successful_turn() {
    let mut output = ProviderOutput::default();
    // Simulate OpenCode emitting an error event for a cancelled interactive prompt (e.g. Web Search)
    output.ingest(
        "opencode",
        &json!({
            "type": "error",
            "error": {
                "message": "Cancelled a request for input this run cannot answer: Web Search"
            }
        }),
    );
    // Should NOT record the cancelled prompt as a fatal error
    assert!(output.error.is_none());

    // Model recovers and emits reasoning and assistant message
    output.ingest(
        "opencode",
        &json!({
            "type": "reasoning",
            "part": {
                "id": "thk_1",
                "text": "The web search was cancelled. Let me fetch news directly."
            }
        }),
    );
    output.ingest(
        "opencode",
        &json!({
            "type": "text",
            "part": {
                "id": "msg_1",
                "text": "Here is the NVIDIA news summary."
            }
        }),
    );

    assert!(output.has_assistant_message());
    assert!(output.error.is_none());
}

#[test]
fn persisted_reasoning_answer_and_followup_have_distinct_canonical_rows() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let mut first = manager.send_message(&agent.id, "First prompt").unwrap();
    let mut output = ProviderOutput::default();
    output.ingest("codex", &json!({"type":"item.completed","item":{"id":"r1","type":"reasoning","text":"First thought"}}));
    output.ingest("codex", &json!({"type":"item.completed","item":{"id":"a1","type":"agent_message","text":"First answer"}}));
    persist_output(&manager, &agent.id, &output).unwrap();
    first.finish(None).unwrap();
    let mut second = manager.send_message(&agent.id, "Followup prompt").unwrap();
    let mut followup = ProviderOutput::default();
    followup.ingest("codex", &json!({"type":"item.completed","item":{"id":"r1","type":"reasoning","text":"Second thought"}}));
    followup.ingest("codex", &json!({"type":"item.completed","item":{"id":"a1","type":"agent_message","text":"Second answer"}}));
    persist_output(&manager, &agent.id, &followup).unwrap();
    second.finish(None).unwrap();
    let reloaded = crate::agent::manager::AgentManager::new(directory.path());
    let entries: Vec<Value> = reloaded
        .get_timeline(&agent.id)
        .iter()
        .enumerate()
        .map(|(index, item)| {
            crate::kernel::server::timeline_item_to_entry("codex", item, (index + 1) as u64)
        })
        .collect();
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry["item"]["text"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "First prompt",
            "First thought",
            "First answer",
            "Followup prompt",
            "Second thought",
            "Second answer"
        ]
    );
    assert_eq!(
        entries
            .iter()
            .map(|entry| entry["seqEnd"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5, 6]
    );
    assert_eq!(entries[1]["item"]["type"], "reasoning");
    assert_eq!(entries[4]["item"]["type"], "reasoning");
    assert_ne!(
        entries[2]["item"]["messageId"],
        entries[5]["item"]["messageId"]
    );
}

#[test]
fn claude_single_block_final_frames_reconcile_streamed_content_indexes() {
    let mut output = ProviderOutput::default();
    output.ingest(
        "claude",
        &json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"m"}}}),
    );
    output.ingest("claude", &json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Think"}}}));
    output.ingest("claude", &json!({"type":"assistant","message":{"id":"m","content":[{"type":"thinking","thinking":"Think"}]}}));
    output.ingest("claude", &json!({"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"An"}}}));
    let suffix = output.ingest("claude", &json!({"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"Answer"}]}}));
    assert_eq!(suffix[0]["text"], "swer");
    assert_eq!(
        output
            .rows
            .iter()
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>(),
        vec!["Think", "Answer"]
    );
}

#[test]
fn test_provider_resolution_for_models() {
    use crate::agent::registry::resolve_provider_for_model;
    assert_eq!(
        resolve_provider_for_model("opencode/deepseek-v4.1-flash-free"),
        Some("opencode")
    );
    assert_eq!(
        resolve_provider_for_model("omniroute/kc/moonshotai/kimi-k2.6"),
        Some("opencode")
    );
    assert_eq!(resolve_provider_for_model("gpt-6-luna"), Some("codex"));
    assert_eq!(
        resolve_provider_for_model("claude-opus-4-6"),
        Some("claude")
    );
    assert_eq!(resolve_provider_for_model("pi/llama-3.3-70b"), Some("pi"));
    assert_eq!(resolve_provider_for_model("omp/custom-model"), Some("omp"));
    assert_eq!(
        resolve_provider_for_model("google-antigravity/gemini-3.8-flash"),
        Some("omp")
    );
}

#[test]
fn test_set_agent_model_switches_provider() {
    let dir = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(dir.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    assert_eq!(agent.provider, "codex");

    let updated = manager
        .set_agent_model(&agent.id, "opencode/deepseek-v4.1-flash-free")
        .unwrap();
    assert_eq!(updated.provider, "opencode");
    assert_eq!(updated.config.as_ref().unwrap()["provider"], "opencode");
    assert_eq!(
        updated.config.as_ref().unwrap()["model"],
        "opencode/deepseek-v4.1-flash-free"
    );

    let updated2 = manager.set_agent_model(&agent.id, "gpt-6-luna").unwrap();
    assert_eq!(updated2.provider, "codex");
    assert_eq!(updated2.config.as_ref().unwrap()["provider"], "codex");
}

#[test]
fn overlap_is_rejected_without_a_prompt_and_cancel_retains_ownership_until_cleanup() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let other = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let mut turn = manager.send_message(&agent.id, "accepted").unwrap();
    assert!(manager.send_message(&agent.id, "rejected").is_err());
    assert_eq!(manager.get_timeline(&agent.id).len(), 1);
    let independent = manager.send_message(&other.id, "independent").unwrap();
    manager.cancel_agent(&agent.id).unwrap();
    assert!(turn.is_cancelled());
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Running
    );
    assert!(manager.send_message(&agent.id, "still rejected").is_err());
    assert!(manager.set_agent_model(&agent.id, "another-model").is_err());
    assert!(manager.delete_agent(&agent.id).is_err());
    turn.finish(None).unwrap();
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Closed
    );
    let replacement = manager.send_message(&agent.id, "next").unwrap();
    drop(turn);
    assert!(manager.send_message(&agent.id, "overlap").is_err());
    drop(replacement);
    drop(independent);
}

#[test]
fn observed_handle_survives_restart_and_provider_switch_discards_it() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let mut turn = manager.send_message(&agent.id, "first").unwrap();
    turn.persist_provider_handle("thread-123").unwrap();
    turn.finish(None).unwrap();
    let reloaded = crate::agent::manager::AgentManager::new(directory.path());
    let agent = reloaded.get_agent(&agent.id).unwrap();
    assert_eq!(
        provider_handle(&agent).unwrap().as_deref(),
        Some("thread-123")
    );
    let args = provider_arguments(
        "codex",
        Some("chosen"),
        Some("high"),
        Path::new("."),
        "second",
        Some("thread-123"),
        None,
    )
    .unwrap();
    assert_eq!(&args[..2], &["exec", "resume"]);
    assert!(args.iter().any(|arg| arg == "thread-123"));
    assert!(!args
        .iter()
        .any(|arg| arg == "--last" || arg == "--continue"));
    let switched = reloaded
        .set_agent_model_with_provider(&agent.id, "new-model", Some("opencode"))
        .unwrap();
    assert_eq!(provider_handle(&switched).unwrap(), None);
}

#[test]
fn provider_identity_uses_only_root_observed_sessions() {
    assert_eq!(
        observed_provider_handle(
            "codex",
            &json!({"type":"thread.started","thread_id":"thread-1"})
        ),
        Some("thread-1")
    );
    assert_eq!(
        observed_provider_handle(
            "claude",
            &json!({"type":"system","subtype":"init","session_id":"session-1"})
        ),
        Some("session-1")
    );
    assert_eq!(
        observed_provider_handle(
            "claude",
            &json!({"type":"assistant","parent_tool_use_id":"child","session_id":"child-session"})
        ),
        None
    );
    assert_eq!(
        observed_provider_handle(
            "opencode",
            &json!({"type":"text","sessionID":"ses_1","part":{"text":"ok"}})
        ),
        Some("ses_1")
    );
}

#[test]
fn admission_failure_releases_reservation_and_does_not_leave_a_prompt() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let blocker = directory.path().join("agents").join("timelines");
    std::fs::write(&blocker, b"not a directory").unwrap();
    assert!(manager.send_message(&agent.id, "must not survive").is_err());
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Idle
    );
    assert_eq!(manager.get_agent(&agent.id).unwrap().title, None);
    std::fs::remove_file(blocker).unwrap();
    let mut accepted = manager
        .send_message(&agent.id, "accepted after repair")
        .unwrap();
    assert_eq!(manager.get_timeline(&agent.id).len(), 1);
    accepted.finish(None).unwrap();
}

#[tokio::test]
async fn stderr_completion_is_bounded_when_a_pipe_writer_remains_open() {
    let (reader, _writer) = tokio::io::duplex(128);
    let task = tokio::spawn(drain_stderr(reader));
    let bytes = tokio::time::timeout(Duration::from_secs(2), finish_stderr(task))
        .await
        .expect("An inherited stderr handle must not block turn completion");
    assert!(bytes.is_empty());
}

fn node_provider(script: &str) -> Command {
    let mut command = Command::new(
        resolve_command("node")
            .expect("Node is required by the repository's controlled child smoke"),
    );
    command.args(["-e", script, "--"]);
    command
}

#[tokio::test]
async fn real_child_persists_session_and_resumes_after_manager_restart() {
    let directory = tempfile::tempdir().unwrap();
    let argument_log = directory.path().join("arguments.jsonl");
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent(
            "codex",
            directory.path().to_str().unwrap(),
            None,
            None,
            Some(json!({"model":"chosen-model","thinkingOptionId":"high"})),
        )
        .unwrap();
    let script = r#"
        const fs = require('fs');
        const args = process.argv.slice(1);
        fs.appendFileSync(process.env.PASEO_CONTROLLED_ARGUMENTS, JSON.stringify(args) + '\n');
        console.log(JSON.stringify({type:'thread.started',thread_id:'controlled-thread'}));
        console.log(JSON.stringify({type:'item.completed',item:{id:'answer',type:'agent_message',text:'controlled answer'}}));
    "#;
    let turn = manager.send_message(&agent.id, "first").unwrap();
    let mut command = node_provider(script);
    command.env("PASEO_CONTROLLED_ARGUMENTS", &argument_log);
    tokio::time::timeout(
        Duration::from_secs(10),
        run_reserved_turn(turn, Some(command)),
    )
    .await
    .unwrap();
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Idle
    );
    let reloaded = crate::agent::manager::AgentManager::new(directory.path());
    let turn = reloaded.send_message(&agent.id, "second").unwrap();
    let mut command = node_provider(script);
    command.env("PASEO_CONTROLLED_ARGUMENTS", &argument_log);
    tokio::time::timeout(
        Duration::from_secs(10),
        run_reserved_turn(turn, Some(command)),
    )
    .await
    .unwrap();
    let invocations: Vec<Vec<String>> = std::fs::read_to_string(argument_log)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(invocations.len(), 2);
    assert_eq!(&invocations[0][..2], &["exec", "--json"]);
    assert_eq!(&invocations[1][..3], &["exec", "resume", "--json"]);
    assert!(invocations[1]
        .windows(2)
        .any(|pair| pair == ["-m", "chosen-model"]));
    assert!(invocations[1]
        .windows(2)
        .any(|pair| pair == ["-c", "model_reasoning_effort=\"high\""]));
    assert_eq!(
        &invocations[1][invocations[1].len() - 3..],
        &["controlled-thread", "--", "second"][..]
    );
    assert_eq!(
        provider_handle(&reloaded.get_agent(&agent.id).unwrap())
            .unwrap()
            .as_deref(),
        Some("controlled-thread")
    );
    assert_eq!(reloaded.get_timeline(&agent.id).len(), 4);
}

#[cfg(windows)]
#[tokio::test]
async fn real_windows_cancel_kills_descendant_and_preserves_partial_output() {
    use windows_sys::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
    use windows_sys::Win32::System::Threading::{OpenProcess, WaitForSingleObject};
    let directory = tempfile::tempdir().unwrap();
    let pid_file = directory.path().join("descendant.pid");
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent(
            "codex",
            directory.path().to_str().unwrap(),
            None,
            None,
            None,
        )
        .unwrap();
    let script = r#"
        const child = require('child_process').spawn(process.execPath, ['-e','setInterval(()=>{},1000)'], {stdio:'inherit'});
        require('fs').writeFileSync(process.env.PASEO_CONTROLLED_PID, String(child.pid));
        console.log(JSON.stringify({type:'thread.started',thread_id:'cancel-thread'}));
        console.log(JSON.stringify({type:'item.completed',item:{id:'partial',type:'agent_message',text:'partial answer'}}));
        setInterval(()=>{},1000);
    "#;
    let mut command = node_provider(script);
    command.env("PASEO_CONTROLLED_PID", &pid_file);
    let turn = manager
        .send_message(&agent.id, "start controlled child")
        .unwrap();
    let running = tokio::spawn(run_reserved_turn(turn, Some(command)));
    tokio::time::timeout(Duration::from_secs(5), async {
        while !pid_file.exists()
            || provider_handle(&manager.get_agent(&agent.id).unwrap())
                .unwrap_or(None)
                .is_none()
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let durable = crate::agent::manager::AgentManager::try_new(directory.path()).unwrap();
    assert_eq!(
        provider_handle(&durable.get_agent(&agent.id).unwrap())
            .unwrap()
            .as_deref(),
        Some("cancel-thread")
    );
    let pid: u32 = std::fs::read_to_string(pid_file).unwrap().parse().unwrap();
    let handle = unsafe { OpenProcess(0x00100000, 0, pid) };
    assert!(!handle.is_null());
    manager.cancel_agent(&agent.id).unwrap();
    assert!(manager
        .send_message(&agent.id, "overlap during cancellation")
        .is_err());
    tokio::time::timeout(Duration::from_secs(8), running)
        .await
        .unwrap()
        .unwrap();
    let exit = unsafe { WaitForSingleObject(handle, 2000) };
    unsafe {
        CloseHandle(handle);
    }
    assert_eq!(
        exit, WAIT_OBJECT_0,
        "Owned descendants must terminate, not merely the provider leader"
    );
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Closed
    );
    assert!(manager.get_timeline(&agent.id).iter().any(|item|
        matches!(item, crate::agent::types::AgentTimelineItem::AssistantMessage { text, .. } if text == "partial answer")));
    assert!(manager.send_message(&agent.id, "after cleanup").is_ok());
}

#[tokio::test]
async fn real_child_omp_and_pi_resume_the_observed_private_file() {
    for provider in ["omp", "pi"] {
        let directory = tempfile::tempdir().unwrap();
        let argument_log = directory.path().join("arguments.jsonl");
        let manager = crate::agent::manager::AgentManager::new(directory.path());
        let agent = manager
            .create_agent(
                provider,
                directory.path().to_str().unwrap(),
                None,
                None,
                None,
            )
            .unwrap();
        let script = r#"
            const fs = require('fs'), path = require('path'), args = process.argv.slice(1);
            fs.appendFileSync(process.env.PASEO_CONTROLLED_ARGUMENTS, JSON.stringify(args) + '\n');
            const index = args.indexOf('--session-dir');
            if(index >= 0) fs.writeFileSync(path.join(args[index+1], 'observed.jsonl'), JSON.stringify({type:'session',id:'private-session'})+'\n');
            console.log(JSON.stringify({type:'message_end',message:{role:'assistant',content:[{type:'text',text:'private answer'}]}}));
        "#;
        let turn = manager.send_message(&agent.id, "first").unwrap();
        let mut command = node_provider(script);
        command.env("PASEO_CONTROLLED_ARGUMENTS", &argument_log);
        tokio::time::timeout(
            Duration::from_secs(10),
            run_reserved_turn(turn, Some(command)),
        )
        .await
        .unwrap();
        let reloaded = crate::agent::manager::AgentManager::new(directory.path());
        let saved = reloaded.get_agent(&agent.id).unwrap();
        assert_eq!(saved.last_status, AgentStatus::Idle);
        let handle = provider_handle(&saved).unwrap().unwrap();
        assert!(Path::new(&handle).is_file());
        assert!(Path::new(&handle)
            .starts_with(directory.path().join("agents").join("provider-sessions")));
        let turn = reloaded.send_message(&agent.id, "second").unwrap();
        let mut command = node_provider(script);
        command.env("PASEO_CONTROLLED_ARGUMENTS", &argument_log);
        tokio::time::timeout(
            Duration::from_secs(10),
            run_reserved_turn(turn, Some(command)),
        )
        .await
        .unwrap();
        let invocations: Vec<Vec<String>> = std::fs::read_to_string(argument_log)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let flag = if provider == "omp" {
            "--resume"
        } else {
            "--session"
        };
        assert!(invocations[1]
            .windows(2)
            .any(|pair| pair[0] == flag && pair[1] == handle));
        assert!(!invocations[1]
            .iter()
            .any(|arg| arg == "--session-dir" || arg == "--continue"));
        assert_eq!(
            reloaded.get_agent(&agent.id).unwrap().last_status,
            AgentStatus::Idle
        );
    }
}

#[tokio::test]
async fn missing_observed_identity_fails_continuation_without_launching_a_fresh_session() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent(
            "codex",
            directory.path().to_str().unwrap(),
            None,
            None,
            None,
        )
        .unwrap();
    let turn = manager.send_message(&agent.id, "first").unwrap();
    run_reserved_turn(turn, Some(node_provider("console.log(JSON.stringify({type:'item.completed',item:{id:'answer',type:'agent_message',text:'partial'}}))"))).await;
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Error
    );
    let reloaded = crate::agent::manager::AgentManager::new(directory.path());
    let turn = reloaded
        .send_message(&agent.id, "must not start fresh")
        .unwrap();
    let launch_marker = directory.path().join("unexpected-launch");
    let mut command = node_provider(
        "require('fs').writeFileSync(process.env.PASEO_UNEXPECTED_LAUNCH,'launched')",
    );
    command.env("PASEO_UNEXPECTED_LAUNCH", &launch_marker);
    run_reserved_turn(turn, Some(command)).await;
    assert!(!launch_marker.exists());
    let record = reloaded.get_agent(&agent.id).unwrap();
    assert!(record
        .last_error
        .unwrap()
        .contains("explicit session handle"));
}

#[tokio::test]
async fn spawn_failure_releases_admission_for_a_retry() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let missing_cwd = directory.path().join("does-not-exist");
    let agent = manager
        .create_agent("codex", missing_cwd.to_str().unwrap(), None, None, None)
        .unwrap();
    let turn = manager
        .send_message(&agent.id, "fails during setup")
        .unwrap();
    run_reserved_turn(turn, Some(node_provider("process.exit(0)"))).await;
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Error
    );
    assert!(manager.send_message(&agent.id, "retry").is_ok());
}

#[tokio::test]
async fn delete_waits_for_owned_turn_and_rejects_replacement_admission() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let mut turn = manager.send_message(&agent.id, "reserved").unwrap();
    let deleting_manager = manager.clone();
    let id = agent.id.clone();
    let deleting = tokio::spawn(async move { deleting_manager.delete_agent_and_wait(&id).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !turn.is_cancelled() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(manager.send_message(&agent.id, "replacement").is_err());
    assert!(manager.get_agent(&agent.id).is_some());
    turn.finish(None).unwrap();
    assert!(deleting.await.unwrap().unwrap());
    assert!(manager.get_agent(&agent.id).is_none());
}

#[test]
fn concurrent_admission_has_one_owner_and_one_durable_prompt() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|index| {
            let manager = manager.clone();
            let barrier = barrier.clone();
            let id = agent.id.clone();
            std::thread::spawn(move || {
                barrier.wait();
                manager.send_message(&id, &format!("prompt {index}"))
            })
        })
        .collect();
    let owners: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .filter_map(Result::ok)
        .collect();
    assert_eq!(owners.len(), 1);
    assert_eq!(manager.get_timeline(&agent.id).len(), 1);
}

#[test]
fn record_write_failure_releases_admission_without_a_prompt() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let records = directory.path().join("agents").join("records.json");
    let backup = directory.path().join("records.backup");
    std::fs::rename(&records, &backup).unwrap();
    std::fs::create_dir(&records).unwrap();
    assert!(manager
        .send_message(&agent.id, "must not be appended")
        .is_err());
    assert!(manager.get_timeline(&agent.id).is_empty());
    assert_eq!(manager.get_agent(&agent.id).unwrap().title, None);
    std::fs::remove_dir(&records).unwrap();
    std::fs::rename(backup, records).unwrap();
    assert!(manager
        .send_message(&agent.id, "retry after repairing disk")
        .is_ok());
}

#[test]
fn failed_completion_commit_does_not_publish_an_idle_status() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let mut turn = manager.send_message(&agent.id, "admitted").unwrap();
    let records = directory.path().join("agents").join("records.json");
    let backup = directory.path().join("records.backup");
    std::fs::rename(&records, &backup).unwrap();
    std::fs::create_dir(&records).unwrap();
    assert!(turn.finish(None).is_err());
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Running
    );
    assert!(manager
        .set_agent_status(&agent.id, AgentStatus::Idle)
        .is_err());
    assert_eq!(
        manager.get_agent(&agent.id).unwrap().last_status,
        AgentStatus::Running
    );
    std::fs::remove_dir(&records).unwrap();
    std::fs::rename(backup, records).unwrap();
    assert!(manager
        .send_message(&agent.id, "retry after repair")
        .is_ok());
}

#[test]
fn fallible_manager_initialization_preserves_malformed_records() {
    let directory = tempfile::tempdir().unwrap();
    let agents = directory.path().join("agents");
    std::fs::create_dir(&agents).unwrap();
    let records = agents.join("records.json");
    std::fs::write(&records, b"malformed records").unwrap();
    assert!(crate::agent::manager::AgentManager::try_new(directory.path()).is_err());
    assert_eq!(std::fs::read(records).unwrap(), b"malformed records");
}

#[tokio::test]
async fn legacy_transcript_without_identity_does_not_start_a_fresh_provider_conversation() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let mut agent = manager
        .create_agent(
            "codex",
            directory.path().to_str().unwrap(),
            None,
            None,
            None,
        )
        .unwrap();
    agent.runtime_info = None; // A record written by the previous native runner.
    manager.storage().save_record(agent.clone()).unwrap();
    manager
        .append_assistant_message(&agent.id, "previous conversation")
        .unwrap();
    let marker = directory.path().join("must-not-launch");
    let mut command = node_provider(
        "require('fs').writeFileSync(process.env.PASEO_UNEXPECTED_LAUNCH,'launched')",
    );
    command.env("PASEO_UNEXPECTED_LAUNCH", &marker);
    let turn = manager.send_message(&agent.id, "followup").unwrap();
    run_reserved_turn(turn, Some(command)).await;
    assert!(!marker.exists());
    assert!(manager
        .get_agent(&agent.id)
        .unwrap()
        .last_error
        .unwrap()
        .contains("explicit session handle"));
}

#[tokio::test]
async fn canceled_turn_emits_protocol_required_reason() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager
        .create_agent("codex", ".", None, None, None)
        .unwrap();
    let mut events = get_provider_push_tx().subscribe();
    let turn = manager
        .send_message(&agent.id, "cancel before scheduling")
        .unwrap();
    let turn_id = turn.token.clone();
    manager.cancel_agent(&agent.id).unwrap();
    run_reserved_turn(turn, None).await;
    let event = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let message: Value = serde_json::from_str(&events.recv().await.unwrap()).unwrap();
            let payload = &message["message"]["payload"];
            if payload["agentId"] == agent.id && payload["event"]["type"] == "turn_canceled" {
                break payload["event"].clone();
            }
        }
    })
    .await
    .unwrap();
    assert_eq!(event["provider"], "codex");
    assert_eq!(event["turnId"], turn_id);
    assert!(event["reason"]
        .as_str()
        .is_some_and(|reason| !reason.is_empty()));
}

#[cfg(unix)]
#[tokio::test]
async fn failed_start_commit_cleans_the_already_running_unix_descendant_group() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
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
        .send_message(&agent.id, "owned before persistence")
        .unwrap();
    let prior = turn.agent.clone();
    let ready = directory.path().join("descendant-ready");
    let mut command = node_provider(
        r#"
        const child = require('child_process').spawn(process.execPath,
            ['-e','setTimeout(()=>process.exit(0),10000)'], {stdio:'inherit'});
        require('fs').writeFileSync(process.env.PASEO_DESCENDANT_READY, String(child.pid));
        setInterval(()=>{},1000);
    "#,
    );
    command
        .env("PASEO_DESCENDANT_READY", &ready)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let (mut tree, mut child) = turn_process::ProcessTree::spawn(&mut command).unwrap();
    let mut stdout = child.stdout.take().unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !ready.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let records = directory.path().join("agents").join("records.json");
    let backup = directory.path().join("records.backup");
    std::fs::rename(&records, &backup).unwrap();
    std::fs::create_dir(&records).unwrap();
    let error = turn.mark_provider_started().unwrap_err();
    let output =
        cleanup_failed_provider_start(&mut turn, &mut tree, &mut child, &prior, error).await;
    assert!(output.cleanup_confirmed);
    let mut remaining = Vec::new();
    tokio::time::timeout(Duration::from_secs(1), stdout.read_to_end(&mut remaining))
        .await
        .expect("A surviving descendant would retain the inherited stdout pipe")
        .unwrap();
    std::fs::remove_dir(&records).unwrap();
    std::fs::rename(backup, records).unwrap();
    turn.finish(output.error.as_deref()).unwrap();
    assert!(manager
        .send_message(&agent.id, "retry after failed start commit")
        .is_ok());
}
