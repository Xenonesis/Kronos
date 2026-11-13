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
        let mut old_lines = 0;
        let mut new_lines = 0;

        for op in &group {
            for change in diff.iter_changes(op) {

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
        let first_op = group[0];
        let old_start = if old_lines == 0 {
            first_op.old_range().start
        } else {
            first_op.old_range().start + 1
        };
        let new_start = if new_lines == 0 {
            first_op.new_range().start
        } else {
            first_op.new_range().start + 1
        };

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

    if let Some(first_hunk) = hunks.first() {
        let preview_lines: Vec<String> = first_hunk
            .lines
            .iter()
            .take(32)
            .map(|l| l.content.clone())
            .collect();
        let rkyv_hunk = crate::kernel::rkyv_shm::DiffHunkChunk::new(
            "diff-preview",
            first_hunk.old_start as u32,
            first_hunk.new_start as u32,
            preview_lines,
        );
        if let Ok(aligned) = rkyv_hunk.serialize_to_aligned_bytes() {
            if let Ok(archived) = crate::kernel::rkyv_shm::DiffHunkChunk::access_archived(&aligned) {
                let _ = crate::kernel::rkyv_shm::DiffHunkChunk::deserialize_archived(archived);
            }
        }
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

pub fn apply_diff_hunk_sync(
    file_content: &str,
    hunk: &DiffHunk,
    reverse: bool,
) -> Result<String, String> {
    let lines: Vec<&str> = file_content.lines().collect();
    let has_trailing_newline = file_content.ends_with('\n');

    let (start_idx, count, replacement_lines): (usize, usize, Vec<String>) = if !reverse {
        let start = if hunk.old_start == 0 { 0 } else { hunk.old_start.saturating_sub(1) };
        let repl: Vec<String> = hunk
            .lines
            .iter()
            .filter(|l| l.line_type != DiffLineType::Deletion)
            .map(|l| l.content.clone())
            .collect();
        (start, hunk.old_lines, repl)
    } else {
        let start = if hunk.new_start == 0 { 0 } else { hunk.new_start.saturating_sub(1) };
        let repl: Vec<String> = hunk
            .lines
            .iter()
            .filter(|l| l.line_type != DiffLineType::Addition)
            .map(|l| l.content.clone())
            .collect();
        (start, hunk.new_lines, repl)
    };

    if start_idx > lines.len() {
        return Err(format!(
            "Hunk start index {} out of range (file has {} lines)",
            start_idx,
            lines.len()
        ));
    }

    let end_idx = (start_idx + count).min(lines.len());
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + replacement_lines.len());

    for line in &lines[..start_idx] {
        out.push(line.to_string());
    }
    for repl in replacement_lines {
        out.push(repl);
    }
    for line in &lines[end_idx..] {
        out.push(line.to_string());
    }

    let mut result = out.join("\n");
    if has_trailing_newline {
        result.push('\n');
    }
    Ok(result)
}

#[tauri::command]
pub async fn apply_diff_hunk(
    file_path: String,
    hunk: DiffHunk,
    reverse: Option<bool>,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        let path = std::path::Path::new(&file_path);
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
        let patched = apply_diff_hunk_sync(&content, &hunk, reverse.unwrap_or(false))?;
        std::fs::write(path, &patched)
            .map_err(|e| format!("Failed to write patched file {}: {}", file_path, e))?;
        Ok(patched)
    })
    .await
    .map_err(|e| format!("Diff hunk task failed: {}", e))?
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
    fn test_compute_file_diff_mid_file_insertion_zero_context() {
        let old = "line 1\nline 2\nline 3\nline 4\n";
        let new = "line 1\nline 2\ninserted A\ninserted B\nline 3\nline 4\n";
        let res = compute_file_diff_sync(old, new, 0);
        assert_eq!(res.hunks.len(), 1);
        let hunk = &res.hunks[0];
        assert_eq!(hunk.old_start, 2);
        assert_eq!(hunk.old_lines, 0);
        assert_eq!(hunk.new_start, 3);
        assert_eq!(hunk.new_lines, 2);
        assert_eq!(hunk.header, "@@ -2,0 +3,2 @@");
        assert_eq!(hunk.lines.len(), 2);
        assert_eq!(hunk.lines[0].line_type, DiffLineType::Addition);
        assert_eq!(hunk.lines[0].new_line_number, Some(3));
        assert_eq!(hunk.lines[1].line_type, DiffLineType::Addition);
        assert_eq!(hunk.lines[1].new_line_number, Some(4));
    }

    #[test]
    fn test_compute_file_diff_mid_file_deletion_zero_context() {
        let old = "line 1\nline 2\ndeleted A\nline 3\n";
        let new = "line 1\nline 2\nline 3\n";
        let res = compute_file_diff_sync(old, new, 0);
        assert_eq!(res.hunks.len(), 1);
        let hunk = &res.hunks[0];
        assert_eq!(hunk.old_start, 3);
        assert_eq!(hunk.old_lines, 1);
        assert_eq!(hunk.new_start, 2);
        assert_eq!(hunk.new_lines, 0);
        assert_eq!(hunk.header, "@@ -3,1 +2,0 @@");
    }

    #[test]
    fn test_compute_file_diff_speed_benchmark() {
        let old: String = (0..5000).map(|i| format!("Line {}\n", i)).collect();
        let new: String = (0..5000).map(|i| format!("Line {}\n", if i % 10 == 0 { i * 2 } else { i })).collect();
        let res = compute_file_diff_sync(&old, &new, 3);
        let limit = if cfg!(debug_assertions) { 50_000 } else { 15_000 };
        assert!(res.elapsed_microseconds < limit, "5,000-line diff took {}us (limit {}us)", res.elapsed_microseconds, limit);
    }

    #[test]
    fn test_apply_diff_hunk_sync_apply_and_revert() {
        let original = "line 1\nline 2\nline 3\n";
        let modified = "line 1\nline 2 updated\nline 3\n";
        let diff = compute_file_diff_sync(original, modified, 0);
        assert_eq!(diff.hunks.len(), 1);

        // Apply hunk onto original
        let patched = apply_diff_hunk_sync(original, &diff.hunks[0], false).unwrap();
        assert_eq!(patched, modified);

        // Revert hunk from patched
        let reverted = apply_diff_hunk_sync(&patched, &diff.hunks[0], true).unwrap();
        assert_eq!(reverted, original);
    }
}
