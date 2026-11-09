use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use std::path::PathBuf;

pub fn get_schedules_dir() -> PathBuf {
    #[cfg(test)]
    {
        static TEST_SCHED_DIR: std::sync::LazyLock<tempfile::TempDir> = std::sync::LazyLock::new(|| {
            tempfile::tempdir().expect("Create isolated schedule test dir")
        });
        let p = TEST_SCHED_DIR.path().join("schedules");
        let _ = std::fs::create_dir_all(&p);
        p
    }
    #[cfg(not(test))]
    {
        let p = crate::get_data_dir().join("schedules");
        let _ = std::fs::create_dir_all(&p);
        p
    }
}

pub fn to_schedule_summary(schedule: &Value) -> Value {
    let mut val = schedule.clone();
    if let Some(obj) = val.as_object_mut() {
        obj.remove("runs");
    }
    val
}

pub fn list_schedules() -> Vec<Value> {
    let dir = get_schedules_dir();
    let mut list = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(val) = serde_json::from_str::<Value>(&content) {
                        if val.get("id").is_some() {
                            list.push(val);
                        }
                    }
                }
            }
        }
    }
    list.sort_by(|a, b| {
        let a_created = a.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");
        let b_created = b.get("createdAt").and_then(|v| v.as_str()).unwrap_or("");
        b_created.cmp(a_created)
    });
    list
}

pub fn load_schedule(id: &str) -> Option<Value> {
    if id.is_empty() {
        return None;
    }
    let path = get_schedules_dir().join(format!("{id}.json"));
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn save_schedule(id: &str, schedule: &Value) -> std::io::Result<()> {
    let dir = get_schedules_dir();
    let path = dir.join(format!("{id}.json"));
    let bytes = serde_json::to_vec_pretty(schedule)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    crate::kernel::config_store::atomic_write(&path, &bytes)
}

pub fn delete_schedule(id: &str) -> bool {
    if id.is_empty() {
        return false;
    }
    let path = get_schedules_dir().join(format!("{id}.json"));
    if path.exists() {
        let _ = std::fs::remove_file(path);
        true
    } else {
        false
    }
}

fn generate_schedule_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:08x}", (now.as_nanos() & 0xffff_ffff) as u32)
}

pub fn calculate_next_run(cadence: &Value, from: DateTime<Utc>) -> Option<String> {
    let cadence_type = cadence.get("type").and_then(|v| v.as_str()).unwrap_or("every");
    let next_dt = match cadence_type {
        "every" => {
            let every_ms = cadence.get("everyMs").and_then(|v| v.as_i64()).unwrap_or(3600000);
            from + Duration::milliseconds(every_ms.max(1000))
        }
        "hourly" => {
            let target_minute = cadence.get("atMinute").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let current_minute = chrono::Timelike::minute(&from);
            let current_second = chrono::Timelike::second(&from);
            let minutes_to_add = if current_minute < target_minute {
                target_minute - current_minute
            } else {
                60 - current_minute + target_minute
            };
            from + Duration::minutes(minutes_to_add as i64) - Duration::seconds(current_second as i64)
        }
        "daily" => {
            let at_time = cadence.get("atTime").and_then(|v| v.as_str()).unwrap_or("00:00");
            let mut parts = at_time.split(':');
            let target_hour: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
            let target_minute: u32 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);

            let cur_h = chrono::Timelike::hour(&from);
            let cur_m = chrono::Timelike::minute(&from);
            let cur_s = chrono::Timelike::second(&from);

            let cur_total_m = cur_h * 60 + cur_m;
            let target_total_m = target_hour * 60 + target_minute;

            let minutes_to_add = if target_total_m > cur_total_m {
                target_total_m - cur_total_m
            } else {
                (24 * 60 - cur_total_m) + target_total_m
            };
            from + Duration::minutes(minutes_to_add as i64) - Duration::seconds(cur_s as i64)
        }
        _ => from + Duration::hours(1),
    };
    Some(next_dt.to_rfc3339_opts(chrono::SecondsFormat::Millis, true))
}

pub fn execute_schedule_run(id: &str) -> Result<Value, String> {
    let mut schedule = load_schedule(id).ok_or_else(|| format!("Schedule {id} not found"))?;
    let now = Utc::now();
    let now_str = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let run_id = generate_schedule_id();

    let prompt = schedule.get("prompt").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let target = schedule.get("target").cloned().unwrap_or(Value::Null);

    let mut agent_id_val = Value::Null;
    if target.get("type").and_then(|v| v.as_str()) == Some("new-agent") {
        if let Some(config) = target.get("config") {
            let provider = config.get("provider").and_then(|v| v.as_str()).unwrap_or("codex");
            let cwd = config.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
            let agent_mgr = crate::kernel::server::get_agent_manager();
            let sched_name = schedule.get("name").and_then(|v| v.as_str()).unwrap_or("Task");
            if let Ok(agent) = agent_mgr.create_agent(
                provider,
                cwd,
                Some(format!("Scheduled: {sched_name}")),
                None,
                None,
            ) {
                let aid = agent.id.clone();
                agent_id_val = serde_json::json!(aid);
                let _ = agent_mgr.send_message(&aid, &prompt);
            }
        }
    }

    let finish_str = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let run_record = serde_json::json!({
        "id": run_id,
        "scheduleId": id,
        "startedAt": now_str,
        "finishedAt": finish_str,
        "status": "succeeded",
        "agentId": agent_id_val,
        "output": format!("Executed scheduled job: {}", prompt),
        "error": serde_json::Value::Null
    });

    let mut runs = schedule.get("runs").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    runs.push(run_record);
    if runs.len() > 50 {
        runs.drain(0..(runs.len() - 50));
    }

    let next_run = calculate_next_run(schedule.get("cadence").unwrap_or(&Value::Null), Utc::now());

    schedule["runs"] = serde_json::json!(runs);
    schedule["lastRunAt"] = serde_json::json!(now_str);
    schedule["updatedAt"] = serde_json::json!(finish_str);
    schedule["nextRunAt"] = match next_run {
        Some(nr) => serde_json::json!(nr),
        None => Value::Null,
    };

    if let Some(max_runs) = schedule.get("maxRuns").and_then(|v| v.as_u64()) {
        if (runs.len() as u64) >= max_runs {
            schedule["status"] = serde_json::json!("ended");
        }
    }

    save_schedule(id, &schedule).map_err(|e| format!("Failed to save schedule: {e}"))?;
    Ok(schedule)
}

pub fn start_scheduler_engine() {
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    tokio::spawn(async {
        println!("[scheduler] Active background scheduler engine initialized");
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            check_and_run_due_schedules();
        }
    });
}

pub fn check_and_run_due_schedules() {
    let now = Utc::now();
    let now_iso = now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let schedules = list_schedules();

    for sched in schedules {
        let status = sched.get("status").and_then(|v| v.as_str()).unwrap_or("");
        if status != "active" {
            continue;
        }
        let id = match sched.get("id").and_then(|v| v.as_str()) {
            Some(i) => i,
            None => continue,
        };

        if let Some(exp) = sched.get("expiresAt").and_then(|v| v.as_str()) {
            if !exp.is_empty() && exp <= now_iso.as_str() {
                let mut updated = sched.clone();
                updated["status"] = serde_json::json!("ended");
                updated["updatedAt"] = serde_json::json!(now_iso);
                let _ = save_schedule(id, &updated);
                continue;
            }
        }

        let is_due = match sched.get("nextRunAt").and_then(|v| v.as_str()) {
            Some(ts) if !ts.is_empty() => ts <= now_iso.as_str(),
            _ => {
                let next = calculate_next_run(sched.get("cadence").unwrap_or(&Value::Null), now);
                let mut updated = sched.clone();
                updated["nextRunAt"] = match next {
                    Some(n) => serde_json::json!(n),
                    None => Value::Null,
                };
                let _ = save_schedule(id, &updated);
                false
            }
        };

        if is_due {
            println!("[scheduler] Autonomous schedule execution triggered for {id}");
            let _ = execute_schedule_run(id);
        }
    }
}

pub fn create_schedule(msg: &Value) -> std::io::Result<Value> {
    let id = generate_schedule_id();
    let now = crate::agent::types::now_iso();
    let prompt = msg.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
    let name = msg.get("name").cloned().unwrap_or(Value::Null);

    let cadence = msg.get("cadence").cloned().unwrap_or_else(|| {
        serde_json::json!({
            "type": "every",
            "everyMs": 3600000
        })
    });

    let mut target = msg.get("target").cloned().unwrap_or_else(|| {
        serde_json::json!({
            "type": "new-agent",
            "config": {
                "provider": "codex",
                "cwd": "."
            }
        })
    });

    if target.get("type").and_then(|v| v.as_str()) == Some("self") {
        target["type"] = serde_json::json!("agent");
    }

    let max_runs = msg.get("maxRuns").cloned().unwrap_or(Value::Null);
    let expires_at = msg.get("expiresAt").cloned().unwrap_or(Value::Null);
    let initial_next = calculate_next_run(&cadence, Utc::now());
    let run_on_create = msg.get("runOnCreate").and_then(|v| v.as_bool()).unwrap_or(false);

    let mut stored = serde_json::json!({
        "id": id,
        "name": name,
        "prompt": prompt,
        "cadence": cadence,
        "target": target,
        "status": "active",
        "createdAt": now,
        "updatedAt": now,
        "nextRunAt": initial_next,
        "lastRunAt": Value::Null,
        "pausedAt": Value::Null,
        "expiresAt": expires_at,
        "maxRuns": max_runs,
        "runs": []
    });

    save_schedule(&id, &stored)?;

    if run_on_create {
        if let Ok(updated) = execute_schedule_run(&id) {
            stored = updated;
        }
    }

    Ok(stored)
}

pub fn pause_schedule(id: &str) -> Result<Value, String> {
    let mut schedule = load_schedule(id).ok_or_else(|| format!("Schedule {id} not found"))?;
    let now = crate::agent::types::now_iso();
    schedule["status"] = serde_json::json!("paused");
    schedule["pausedAt"] = serde_json::json!(now);
    schedule["updatedAt"] = serde_json::json!(now);
    save_schedule(id, &schedule).map_err(|e| format!("Failed to save schedule: {e}"))?;
    Ok(to_schedule_summary(&schedule))
}

pub fn resume_schedule(id: &str) -> Result<Value, String> {
    let mut schedule = load_schedule(id).ok_or_else(|| format!("Schedule {id} not found"))?;
    let now = crate::agent::types::now_iso();
    schedule["status"] = serde_json::json!("active");
    schedule["pausedAt"] = Value::Null;
    schedule["updatedAt"] = serde_json::json!(now);
    save_schedule(id, &schedule).map_err(|e| format!("Failed to save schedule: {e}"))?;
    Ok(to_schedule_summary(&schedule))
}

pub fn update_schedule(id: &str, patch: &Value) -> Result<Value, String> {
    let mut schedule = load_schedule(id).ok_or_else(|| format!("Schedule {id} not found"))?;
    let now = crate::agent::types::now_iso();
    schedule["updatedAt"] = serde_json::json!(now);

    if let Some(name) = patch.get("name") {
        schedule["name"] = name.clone();
    }
    if let Some(prompt) = patch.get("prompt").and_then(|v| v.as_str()) {
        schedule["prompt"] = serde_json::json!(prompt);
    }
    if let Some(cadence) = patch.get("cadence") {
        schedule["cadence"] = cadence.clone();
        schedule["nextRunAt"] = match calculate_next_run(cadence, Utc::now()) {
            Some(nr) => serde_json::json!(nr),
            None => Value::Null,
        };
    }
    if let Some(max_runs) = patch.get("maxRuns") {
        schedule["maxRuns"] = max_runs.clone();
    }
    if let Some(expires_at) = patch.get("expiresAt") {
        schedule["expiresAt"] = expires_at.clone();
    }
    if let Some(new_agent_cfg) = patch.get("newAgentConfig") {
        if let Some(target) = schedule.get_mut("target") {
            if target.get("type").and_then(|v| v.as_str()) == Some("new-agent") {
                if let Some(config) = target.get_mut("config") {
                    if let Some(obj) = new_agent_cfg.as_object() {
                        for (k, v) in obj {
                            config[k] = v.clone();
                        }
                    }
                }
            }
        }
    }

    save_schedule(id, &schedule).map_err(|e| format!("Failed to save schedule: {e}"))?;
    Ok(schedule)
}
