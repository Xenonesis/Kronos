use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};
use std::time::Instant;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiffLineType {
    Context,
    Addition,
    Deletion,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub line_type: DiffLineType,
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
    pub content: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileDiffResult {
    pub hunks: Vec<DiffHunk>,
    pub additions: usize,
    pub deletions: usize,
    pub elapsed_microseconds: u64,
}

pub fn compute_file_diff_sync(old_text: &str, new_text: &str, context_lines: usize) -> FileDiffResult {
    let start = Instant::now();
    let diff = TextDiff::from_lines(old_text, new_text);

    let mut hunks = Vec::new();
    let mut additions = 0;
    let mut deletions = 0;

    for group in diff.grouped_ops(context_lines) {
        if group.is_empty() {
            continue;
        }

        let mut hunk_lines = Vec::new();
        let mut old_start = 0;
        let mut old_lines = 0;
        let mut new_start = 0;
        let mut new_lines = 0;
        let mut first = true;

        for op in &group {
            for change in diff.iter_changes(op) {
                if first {
                    old_start = change.old_index().unwrap_or(0) + 1;
                    new_start = change.new_index().unwrap_or(0) + 1;
                    first = false;
                }

                let (line_type, old_no, new_no) = match change.tag() {
                    ChangeTag::Equal => {
                        old_lines += 1;
                        new_lines += 1;
                        (
                            DiffLineType::Context,
                            change.old_index().map(|i| i + 1),
                            change.new_index().map(|i| i + 1),
                        )
                    }
                    ChangeTag::Delete => {
                        old_lines += 1;
                        deletions += 1;
                        (
                            DiffLineType::Deletion,
                            change.old_index().map(|i| i + 1),
                            None,
                        )
                    }
                    ChangeTag::Insert => {
                        new_lines += 1;
                        additions += 1;
                        (
                            DiffLineType::Addition,
                            None,
                            change.new_index().map(|i| i + 1),
                        )
                    }
                };

                hunk_lines.push(DiffLine {
                    line_type,
                    old_line_number: old_no,
                    new_line_number: new_no,
                    content: change.value().trim_end_matches(['\r', '\n']).to_string(),
                });
            }
        }

        let header = format!("@@ -{},{} +{},{} @@", old_start, old_lines, new_start, new_lines);
        hunks.push(DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            header,
            lines: hunk_lines,
        });
    }

    FileDiffResult {
        hunks,
        additions,
        deletions,
        elapsed_microseconds: start.elapsed().as_micros() as u64,
    }
}

#[tauri::command]
pub async fn compute_file_diff(
    old_text: String,
    new_text: String,
    context_lines: Option<usize>,
) -> Result<FileDiffResult, String> {
    tokio::task::spawn_blocking(move || {
        Ok(compute_file_diff_sync(&old_text, &new_text, context_lines.unwrap_or(3)))
    })
    .await
    .map_err(|e| format!("Diff task failed: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_file_diff_basic() {
        let old = "fn main() {\n    println!(\"hello\");\n}\n";
        let new = "fn main() {\n    println!(\"world\");\n    return;\n}\n";
        let res = compute_file_diff_sync(old, new, 3);
        assert_eq!(res.additions, 2);
        assert_eq!(res.deletions, 1);
        assert!(!res.hunks.is_empty());
    }

    #[test]
    fn test_compute_file_diff_speed_benchmark() {
        let old: String = (0..5000).map(|i| format!("Line {}\n", i)).collect();
        let new: String = (0..5000).map(|i| format!("Line {}\n", if i % 10 == 0 { i * 2 } else { i })).collect();
        let res = compute_file_diff_sync(&old, &new, 3);
        assert!(res.elapsed_microseconds < 15_000, "5,000-line diff must execute in under 15ms");
    }
}
