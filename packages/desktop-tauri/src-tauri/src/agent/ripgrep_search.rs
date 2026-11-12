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

/// Finds all occurrence ranges of `query` in `text` respecting case and whole_word rules.
pub fn find_matches_in_line(
    line: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    if query.is_empty() || line.is_empty() {
        return Vec::new();
    }

    let mut matches = Vec::new();
    let haystack = if case_sensitive {
        line.to_string()
    } else {
        line.to_lowercase()
    };
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    let mut offset = 0;
    while let Some(pos) = haystack[offset..].find(&needle) {
        let start = offset + pos;
        let end = start + needle.len();

        let is_valid = if whole_word {
            let before_ok = if start == 0 {
                true
            } else {
                let prev_char = line[..start].chars().last().unwrap_or(' ');
                !prev_char.is_alphanumeric() && prev_char != '_'
            };
            let after_ok = if end >= line.len() {
                true
            } else {
                let next_char = line[end..].chars().next().unwrap_or(' ');
                !next_char.is_alphanumeric() && next_char != '_'
            };
            before_ok && after_ok
        } else {
            true
        };

        if is_valid {
            matches.push((start, end));
        }

        offset = end;
        if offset >= haystack.len() {
            break;
        }
    }

    matches
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

    for file_path in files {
        if total_matches >= max_results {
            break;
        }

        if let Ok(content) = fs::read_to_string(&file_path) {
            let relative_path = file_path
                .strip_prefix(root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");

            let mut file_matches = Vec::new();

            for (idx, line) in content.lines().enumerate() {
                let line_matches = find_matches_in_line(
                    line,
                    &req.query,
                    req.case_sensitive,
                    req.whole_word,
                );

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
                let matches = find_matches_in_line(
                    line,
                    &req.query,
                    req.case_sensitive,
                    req.whole_word,
                );

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
                let joined = line_replaced.join("\n");
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
