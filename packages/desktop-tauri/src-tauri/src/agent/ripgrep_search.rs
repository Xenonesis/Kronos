//! Kronos Global Workspace Search & Batch Replace Engine
//!
//! Provides ultra-fast case-aware workspace text search and transactional batch replacements
//! with match previews.

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchMatch {
    pub file_path: String,
    pub relative_path: String,
    pub line_number: usize,
    pub line_text: String,
    pub match_start: usize,
    pub match_end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileMatchesGroup {
    pub relative_path: String,
    pub full_path: String,
    pub match_count: usize,
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSearchQuery {
    pub cwd: String,
    pub query: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_filter: Option<String>,
    pub max_results: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBatchReplaceRequest {
    pub cwd: String,
    pub query: String,
    pub replacement: String,
    pub is_regex: bool,
    pub case_sensitive: bool,
    pub whole_word: bool,
    pub include_filter: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceBatchReplaceResult {
    pub files_modified: usize,
    pub total_replacements: usize,
    pub modified_paths: Vec<String>,
}

/// Recursively traverses workspace directory skipping standard noise dirs.
fn walk_files(dir: &Path, files: &mut Vec<PathBuf>, include_ext: Option<&str>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if file_name.starts_with('.')
                || file_name == "node_modules"
                || file_name == "target"
                || file_name == "dist"
                || file_name == "build"
            {
                continue;
            }

            if path.is_dir() {
                walk_files(&path, files, include_ext);
            } else if path.is_file() {
                if let Some(ext) = include_ext {
                    if !file_name.ends_with(ext) {
                        continue;
                    }
                }
                files.push(path);
            }
        }
    }
}

/// Finds all occurrence ranges of `query` in `line`, supporting regex, case sensitivity, and whole-word matching.
pub fn find_matches_in_line_with_options(
    line: &str,
    query: &str,
    is_regex: bool,
    case_sensitive: bool,
    whole_word: bool,
) -> Result<Vec<(usize, usize)>, String> {
    if query.is_empty() || line.is_empty() {
        return Ok(Vec::new());
    }

    let is_word_boundary = |start: usize, end: usize| -> bool {
        if !whole_word {
            return true;
        }
        let before_ok = start == 0
            || line[..start]
                .chars()
                .last()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_');
        let after_ok = end >= line.len()
            || line[end..]
                .chars()
                .next()
                .is_none_or(|c| !c.is_alphanumeric() && c != '_');
        before_ok && after_ok
    };

    if is_regex {
        let mut pattern = if whole_word {
            format!(r"\b(?:{query})\b")
        } else {
            query.to_string()
        };
        if !case_sensitive {
            pattern = format!("(?i){pattern}");
        }
        let re = regex::Regex::new(&pattern).map_err(|e| format!("Invalid regex pattern: {e}"))?;
        let ranges = re
            .find_iter(line)
            .filter(|m| m.start() < m.end() && is_word_boundary(m.start(), m.end()))
            .map(|m| (m.start(), m.end()))
            .collect();
        return Ok(ranges);
    }

    if case_sensitive {
        let mut matches = Vec::new();
        let mut offset = 0;
        while let Some(pos) = line[offset..].find(query) {
            let start = offset + pos;
            let end = start + query.len();
            if is_word_boundary(start, end) {
                matches.push((start, end));
            }
            offset = end.max(offset + 1);
            if offset >= line.len() {
                break;
            }
        }
        return Ok(matches);
    }

    let escaped = regex::escape(query);
    let re = regex::Regex::new(&format!("(?i){escaped}"))
        .map_err(|e| format!("Failed building case-insensitive search: {e}"))?;
    let matches = re
        .find_iter(line)
        .filter(|m| is_word_boundary(m.start(), m.end()))
        .map(|m| (m.start(), m.end()))
        .collect();
    Ok(matches)
}

/// Finds all occurrence ranges of `query` in `text` respecting case and whole_word rules.
pub fn find_matches_in_line(
    line: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    find_matches_in_line_with_options(line, query, false, case_sensitive, whole_word)
        .unwrap_or_default()
}

/// Executes a fast multi-file search across the workspace.
pub fn search_workspace(req: &WorkspaceSearchQuery) -> Result<Vec<FileMatchesGroup>, String> {
    let root = Path::new(&req.cwd);
    if !root.is_dir() {
        return Err(format!("Directory not found: {}", req.cwd));
    }

    if req.query.is_empty() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    walk_files(root, &mut files, req.include_filter.as_deref());

    let max_results = req.max_results.unwrap_or(500);
    let mut total_matches = 0;
    let mut groups = Vec::new();

    // Batch-read workspace files via Windows Kernel IoRing / Rayon parallel pool
    let loaded_files = crate::kernel::ioring::IoRingBatchReader::read_files_lossy(&files, 2 * 1024 * 1024);

    for (file_path, raw_bytes) in loaded_files {
        if total_matches >= max_results {
            break;
        }
        if let Ok(content) = String::from_utf8(raw_bytes) {
            let relative_path = file_path
                .strip_prefix(root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");

            let mut file_matches = Vec::new();

            for (idx, line) in content.lines().enumerate() {
                let line_matches = find_matches_in_line_with_options(
                    line,
                    &req.query,
                    req.is_regex,
                    req.case_sensitive,
                    req.whole_word,
                )?;

                for (start, end) in line_matches {
                    file_matches.push(SearchMatch {
                        file_path: file_path.to_string_lossy().to_string(),
                        relative_path: relative_path.clone(),
                        line_number: idx + 1,
                        line_text: line.to_string(),
                        match_start: start,
                        match_end: end,
                    });

                    total_matches += 1;
                    if total_matches >= max_results {
                        break;
                    }
                }
                if total_matches >= max_results {
                    break;
                }
            }

            if !file_matches.is_empty() {
                groups.push(FileMatchesGroup {
                    match_count: file_matches.len(),
                    relative_path,
                    full_path: file_path.to_string_lossy().to_string(),
                    matches: file_matches,
                });
            }
        }
    }

    Ok(groups)
}

/// Performs transactional batch replacements across all matching files in the workspace.
pub fn batch_replace_workspace(
    req: &WorkspaceBatchReplaceRequest,
) -> Result<WorkspaceBatchReplaceResult, String> {
    let root = Path::new(&req.cwd);
    if !root.is_dir() {
        return Err(format!("Directory not found: {}", req.cwd));
    }

    if req.query.is_empty() {
        return Ok(WorkspaceBatchReplaceResult {
            files_modified: 0,
            total_replacements: 0,
            modified_paths: Vec::new(),
        });
    }

    let mut files = Vec::new();
    walk_files(root, &mut files, req.include_filter.as_deref());

    let mut files_modified = 0;
    let mut total_replacements = 0;
    let mut modified_paths = Vec::new();

    for file_path in files {
        if let Ok(content) = fs::read_to_string(&file_path) {
            let mut line_replaced = Vec::new();
            let mut file_count = 0;

            for line in content.lines() {
                let matches = find_matches_in_line_with_options(
                    line,
                    &req.query,
                    req.is_regex,
                    req.case_sensitive,
                    req.whole_word,
                )?;

                if matches.is_empty() {
                    line_replaced.push(line.to_string());
                } else {
                    file_count += matches.len();
                    let mut new_line = String::new();
                    let mut last = 0;
                    for (start, end) in matches {
                        new_line.push_str(&line[last..start]);
                        new_line.push_str(&req.replacement);
                        last = end;
                    }
                    new_line.push_str(&line[last..]);
                    line_replaced.push(new_line);
                }
            }

            if file_count > 0 {
                let mut joined = line_replaced.join("\n");
                if content.ends_with('\n') {
                    joined.push('\n');
                }
                if fs::write(&file_path, joined.as_bytes()).is_ok() {
                    files_modified += 1;
                    total_replacements += file_count;
                    let rel = file_path
                        .strip_prefix(root)
                        .unwrap_or(&file_path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    modified_paths.push(rel);
                }
            }
        }
    }

    Ok(WorkspaceBatchReplaceResult {
        files_modified,
        total_replacements,
        modified_paths,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_matches_case_sensitive() {
        let line = "Hello world, hello universe";
        let matches = find_matches_in_line(line, "hello", true, false);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (13, 18));

        let matches_ci = find_matches_in_line(line, "hello", false, false);
        assert_eq!(matches_ci.len(), 2);
    }

    #[test]
    fn test_find_matches_whole_word() {
        let line = "console.log(catalog)";
        let matches = find_matches_in_line(line, "log", false, true);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0], (8, 11));
    }
}
