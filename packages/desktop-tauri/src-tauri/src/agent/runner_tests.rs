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
    assert_eq!(output.rows.iter().map(|r| (r.kind, r.text.as_str())).collect::<Vec<_>>(), vec![
        (OutputKind::Reasoning, "Plan carefully"), (OutputKind::Assistant, "Answer"),
        (OutputKind::Reasoning, "Check"), (OutputKind::Assistant, "Final"),
    ]);
    assert_eq!(live.iter().map(|v| v["text"].as_str().unwrap()).collect::<Vec<_>>(), vec!["Plan", " carefully", "Answer", "Check", "Final"]);
}

#[test]
fn opencode_reasoning_parts_are_not_discarded_or_duplicated() {
    let mut output = ProviderOutput::default();
    let event = json!({"type":"reasoning","sessionID":"s","part":{"id":"p1","messageID":"m1","type":"reasoning","text":"Consider"}});
    assert_eq!(output.ingest("opencode", &event)[0]["text"], "Consider");
    assert!(output.ingest("opencode", &event).is_empty());
    output.ingest("opencode", &json!({"type":"text","part":{"id":"p2","messageID":"m1","type":"text","text":"OK"}}));
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
        assert_eq!(suffix.iter().map(|v| v["text"].as_str().unwrap()).collect::<Vec<_>>(), vec!["tial", " second"]);
        output.ingest(provider, &json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"thinking","thinking":"Next"},{"type":"text","text":"Followup"}]}}));
        assert_eq!(output.rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>(), vec!["Think more", "Partial", " second", "Next", "Followup"]);
    }
}

#[test]
fn claude_partial_thinking_and_final_blocks_do_not_duplicate_or_leak_signatures() {
    let mut output = ProviderOutput::default();
    output.ingest("claude", &json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"m1"}}}));
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
    assert_eq!(a["message"]["payload"]["event"]["item"]["messageId"], b["message"]["payload"]["event"]["item"]["messageId"]);
    assert_eq!(a["message"]["payload"]["event"]["turnId"], "turn-1");
    let next = ProviderOutput::default();
    assert_ne!(output.identity, next.identity);
}

#[test]
fn cli_thinking_options_use_native_arguments_and_default_variant_is_omitted() {
    for provider in ["omp", "pi"] {
        let args = provider_arguments(provider, Some("provider/model"), Some("high"), Path::new("."), "Prompt");
        assert!(args.windows(2).any(|pair| pair == ["--thinking", "high"]));
    }
    let args = provider_arguments("opencode", None, Some("default"), Path::new("."), "Prompt");
    assert!(args.iter().any(|arg| arg == "--thinking"));
    assert!(args.iter().any(|arg| arg == "--pure"));
    assert!(!args.iter().any(|arg| arg == "--variant"));
    let low_args = provider_arguments("opencode", None, Some("low"), Path::new("."), "Prompt");
    assert!(!low_args.iter().any(|arg| arg == "--variant"));
    let opencode_high = provider_arguments("opencode", None, Some("high"), Path::new("."), "Prompt");
    assert!(opencode_high.windows(2).any(|pair| pair == ["--variant", "high"]));
    let args = provider_arguments("codex", None, Some("xhigh"), Path::new("."), "Prompt");
    assert!(args.windows(2).any(|pair| pair == ["-c", "model_reasoning_effort=\"xhigh\""]));
    let args = provider_arguments("claude", Some("model"), Some("high"), Path::new("."), "Prompt");
    assert!(args.windows(2).any(|pair| pair == ["--effort", "high"]));
    assert!(args.iter().any(|arg| arg == "--include-partial-messages"));
}

#[test]
fn errors_keep_real_partial_reasoning_available_for_persistence() {
    let mut output = ProviderOutput::default();
    output.ingest("codex", &json!({"type":"item.completed","item":{"id":"r","type":"reasoning","text":"Partial"}}));
    output.ingest("codex", &json!({"type":"turn.failed","error":{"message":"Failure"}}));
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
fn persisted_reasoning_answer_and_followup_have_distinct_canonical_rows() {
    let directory = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(directory.path());
    let agent = manager.create_agent("codex", ".", None, None, None).unwrap();
    manager.send_message(&agent.id, "First prompt").unwrap();
    let mut output = ProviderOutput::default();
    output.ingest("codex", &json!({"type":"item.completed","item":{"id":"r1","type":"reasoning","text":"First thought"}}));
    output.ingest("codex", &json!({"type":"item.completed","item":{"id":"a1","type":"agent_message","text":"First answer"}}));
    persist_output(&manager, &agent.id, &output).unwrap();
    manager.send_message(&agent.id, "Followup prompt").unwrap();
    let mut followup = ProviderOutput::default();
    followup.ingest("codex", &json!({"type":"item.completed","item":{"id":"r1","type":"reasoning","text":"Second thought"}}));
    followup.ingest("codex", &json!({"type":"item.completed","item":{"id":"a1","type":"agent_message","text":"Second answer"}}));
    persist_output(&manager, &agent.id, &followup).unwrap();
    let reloaded = crate::agent::manager::AgentManager::new(directory.path());
    let entries: Vec<Value> = reloaded.get_timeline(&agent.id).iter().enumerate()
        .map(|(index, item)| crate::kernel::server::timeline_item_to_entry("codex", item, (index + 1) as u64))
        .collect();
    assert_eq!(entries.iter().map(|entry| entry["item"]["text"].as_str().unwrap()).collect::<Vec<_>>(),
        vec!["First prompt", "First thought", "First answer", "Followup prompt", "Second thought", "Second answer"]);
    assert_eq!(entries.iter().map(|entry| entry["seqEnd"].as_u64().unwrap()).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(entries[1]["item"]["type"], "reasoning");
    assert_eq!(entries[4]["item"]["type"], "reasoning");
    assert_ne!(entries[2]["item"]["messageId"], entries[5]["item"]["messageId"]);
}

#[test]
fn claude_single_block_final_frames_reconcile_streamed_content_indexes() {
    let mut output = ProviderOutput::default();
    output.ingest("claude", &json!({"type":"stream_event","event":{"type":"message_start","message":{"id":"m"}}}));
    output.ingest("claude", &json!({"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Think"}}}));
    output.ingest("claude", &json!({"type":"assistant","message":{"id":"m","content":[{"type":"thinking","thinking":"Think"}]}}));
    output.ingest("claude", &json!({"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"An"}}}));
    let suffix = output.ingest("claude", &json!({"type":"assistant","message":{"id":"m","content":[{"type":"text","text":"Answer"}]}}));
    assert_eq!(suffix[0]["text"], "swer");
    assert_eq!(output.rows.iter().map(|row| row.text.as_str()).collect::<Vec<_>>(), vec!["Think", "Answer"]);
}

#[test]
fn test_provider_resolution_for_models() {
    use crate::agent::registry::resolve_provider_for_model;
    assert_eq!(resolve_provider_for_model("opencode/deepseek-v4.1-flash-free"), Some("opencode"));
    assert_eq!(resolve_provider_for_model("omniroute/kc/moonshotai/kimi-k2.6"), Some("opencode"));
    assert_eq!(resolve_provider_for_model("gpt-6-luna"), Some("codex"));
    assert_eq!(resolve_provider_for_model("claude-opus-4-6"), Some("claude"));
    assert_eq!(resolve_provider_for_model("pi/llama-3.3-70b"), Some("pi"));
    assert_eq!(resolve_provider_for_model("omp/custom-model"), Some("omp"));
    assert_eq!(resolve_provider_for_model("google-antigravity/gemini-3.8-flash"), Some("omp"));
}

#[test]
fn test_set_agent_model_switches_provider() {
    let dir = tempfile::tempdir().unwrap();
    let manager = crate::agent::manager::AgentManager::new(dir.path());
    let agent = manager.create_agent("codex", ".", None, None, None).unwrap();
    assert_eq!(agent.provider, "codex");

    let updated = manager.set_agent_model(&agent.id, "opencode/deepseek-v4.1-flash-free").unwrap();
    assert_eq!(updated.provider, "opencode");
    assert_eq!(updated.config.as_ref().unwrap()["provider"], "opencode");
    assert_eq!(updated.config.as_ref().unwrap()["model"], "opencode/deepseek-v4.1-flash-free");

    let updated2 = manager.set_agent_model(&agent.id, "gpt-6-luna").unwrap();
    assert_eq!(updated2.provider, "codex");
    assert_eq!(updated2.config.as_ref().unwrap()["provider"], "codex");
}
