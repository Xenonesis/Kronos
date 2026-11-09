use crate::agent::types::AgentTimelineItem;
use serde_json::{json, Value};

pub fn epoch(agent_id: &str) -> String {
    format!("native:{agent_id}")
}

fn is_turn_start(item: &AgentTimelineItem) -> bool {
    matches!(item, AgentTimelineItem::UserMessage { .. })
}

/// Project only the selected slice; sequence positions address append-only stored rows.
pub fn project_page(
    agent_id: &str,
    provider: &str,
    items: &[AgentTimelineItem],
    request: &Value,
) -> Value {
    let nested = request.get("payload");
    let field = |key: &str| request.get(key).or_else(|| nested.and_then(|p| p.get(key)));
    let epoch = epoch(agent_id);
    let total = items.len();
    let requested_direction = match field("direction").and_then(Value::as_str) {
        Some("before") => "before",
        Some("after") => "after",
        _ => "tail",
    };
    let cursor = field("cursor");
    let cursor_seq = cursor
        .and_then(|c| c["seq"].as_u64())
        .and_then(|n| usize::try_from(n).ok());
    let stale = cursor.is_some_and(|cursor| cursor["epoch"].as_str() != Some(epoch.as_str()));
    let direction = if stale { "tail" } else { requested_direction };
    let limit = match field("limit").and_then(Value::as_u64) {
        // The shared wire contract reserves zero for an explicit all-rows read.
        Some(0) => total.max(1),
        Some(n) => usize::try_from(n).unwrap_or(500).min(500),
        None => 40,
    };
    let (mut start, mut end) = match direction {
        "before" => {
            let end = cursor_seq.unwrap_or(total + 1).saturating_sub(1).min(total);
            (end.saturating_sub(limit), end)
        }
        "after" => {
            let start = cursor_seq.unwrap_or(0).min(total);
            (start, start.saturating_add(limit).min(total))
        }
        _ => (total.saturating_sub(limit), total),
    };
    // Returned backward cursors start at prompts, so subsequent pages meet
    // without splitting those turns. Forward catch-up remains incremental.
    if field("wholeTurns").and_then(Value::as_bool) == Some(true) && direction != "after" {
        while start > 0 && start < end && !is_turn_start(&items[start]) {
            start -= 1;
        }
        if direction == "before" && start < end {
            while end < total && !is_turn_start(&items[end]) {
                end += 1;
            }
        }
    }
    let mut turn_start = start.min(total.saturating_sub(1));
    while turn_start > 0 && !is_turn_start(&items[turn_start]) {
        turn_start -= 1;
    }
    let mut turn_id = match items.get(turn_start) {
        Some(AgentTimelineItem::UserMessage { id, .. }) => Some(id.as_str()),
        _ => None,
    };
    let entries: Vec<Value> = items[start..end]
        .iter()
        .enumerate()
        .map(|(offset, item)| {
            if let AgentTimelineItem::UserMessage { id, .. } = item {
                turn_id = Some(id);
            }
            let mut entry =
                super::server::timeline_item_to_entry(provider, item, (start + offset + 1) as u64);
            if let Some(turn_id) = turn_id {
                entry["turnId"] = json!(turn_id);
            }
            entry
        })
        .collect();
    json!({
        "direction": requested_direction,
        "projection": field("projection").and_then(Value::as_str).unwrap_or("projected"),
        "epoch": epoch,
        "reset": direction == "tail",
        "staleCursor": stale,
        "gap": false,
        "window": {"minSeq": if total == 0 { 0 } else { 1 }, "maxSeq": total, "nextSeq": total + 1},
        "startCursor": if start < end { Some(json!({"epoch":epoch,"seq":start + 1})) } else { None },
        "endCursor": if start < end { Some(json!({"epoch":epoch,"seq":end})) } else { None },
        "hasOlder": start > 0,
        "hasNewer": end < total,
        "mergeWindow": field("mergeWindow").and_then(Value::as_bool).unwrap_or(false),
        "entries": entries,
        "error": null,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn history(turns: usize) -> Vec<AgentTimelineItem> {
        (0..turns)
            .flat_map(|n| {
                [
                    AgentTimelineItem::UserMessage {
                        id: format!("u{n}"),
                        text: format!("prompt {n}"),
                        timestamp: "2026-11-07T00:00:00Z".into(),
                    },
                    AgentTimelineItem::AssistantMessage {
                        id: format!("a{n}"),
                        text: format!("answer {n}"),
                        timestamp: "2026-11-07T00:00:01Z".into(),
                    },
                ]
            })
            .collect()
    }

    #[test]
    fn pages_backward_and_forward_without_gaps_or_duplicate_rows() {
        let items = history(5);
        let tail = project_page("ag-test", "codex", &items, &json!({"limit":3}));
        assert_eq!(tail["startCursor"]["seq"], 8);
        assert_eq!(tail["entries"][0]["item"]["text"], "answer 3");
        assert_eq!(tail["hasOlder"], true);
        let older = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"direction":"before","cursor":tail["startCursor"],"limit":3}),
        );
        assert_eq!(older["startCursor"]["seq"], 5);
        assert_eq!(older["endCursor"]["seq"], 7);
        assert_eq!(older["reset"], false);
        let newer = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"direction":"after","cursor":older["endCursor"],"limit":2}),
        );
        assert_eq!(newer["startCursor"]["seq"], 8);
        assert_eq!(newer["endCursor"]["seq"], 9);
        assert_eq!(newer["hasNewer"], true);
        let last = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"direction":"after","cursor":newer["endCursor"],"limit":2}),
        );
        assert_eq!(last["entries"][0]["item"]["text"], "answer 4");
        assert_eq!(last["hasNewer"], false);
    }

    #[test]
    fn stale_epoch_resets_to_tail_and_empty_edges_are_explicit() {
        let items = history(3);
        let stale = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"direction":"after","cursor":{"epoch":"obsolete","seq":1},"limit":2}),
        );
        assert_eq!(stale["reset"], true);
        assert_eq!(stale["staleCursor"], true);
        assert_eq!(stale["startCursor"]["seq"], 5);
        let empty = project_page("ag-test", "codex", &[], &json!({}));
        assert_eq!(empty["window"]["nextSeq"], 1);
        assert_eq!(empty["entries"], json!([]));
        assert!(empty["startCursor"].is_null());
        let before = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"direction":"before","cursor":{"epoch":epoch("ag-test"),"seq":1},"limit":2}),
        );
        assert_eq!(before["entries"], json!([]));
        assert_eq!(before["hasOlder"], false);
    }

    #[test]
    fn whole_turn_tail_and_backward_pages_preserve_prompt_boundaries() {
        let items = history(4);
        let tail = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"limit":3,"wholeTurns":true}),
        );
        assert_eq!(tail["startCursor"]["seq"], 5);
        assert_eq!(tail["entries"][0]["turnId"], "u2");
        let older = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"direction":"before","cursor":tail["startCursor"],"limit":3,"wholeTurns":true}),
        );
        assert_eq!(older["startCursor"]["seq"], 1);
        assert_eq!(older["endCursor"]["seq"], 4);
        assert_eq!(older["hasOlder"], false);
    }
    #[test]
    fn optional_and_future_cursors_keep_window_requests_incremental() {
        let items = history(3);
        let first = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({"direction":"after","limit":2}),
        );
        assert_eq!(first["startCursor"]["seq"], 1);
        assert_eq!(first["reset"], false);
        assert_eq!(first["hasNewer"], true);
        let before = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({
                "direction":"before","cursor":{"epoch":epoch("ag-test"),"seq":26},"limit":2,"mergeWindow":true
            }),
        );
        assert_eq!(before["startCursor"]["seq"], 5);
        assert_eq!(before["endCursor"]["seq"], 6);
        assert_eq!(before["staleCursor"], false);
        assert_eq!(before["reset"], false);
        assert_eq!(before["mergeWindow"], true);
    }

    #[test]
    fn whole_turn_backward_window_includes_the_available_right_edge() {
        let items = history(3);
        let page = project_page(
            "ag-test",
            "codex",
            &items,
            &json!({
                "direction":"before","cursor":{"epoch":epoch("ag-test"),"seq":2},"limit":1,"wholeTurns":true
            }),
        );
        assert_eq!(page["entries"][0]["item"]["text"], "prompt 0");
        assert_eq!(page["entries"][1]["item"]["text"], "answer 0");
        assert_eq!(page["endCursor"]["seq"], 2);
        assert_eq!(page["hasNewer"], true);
    }
}
