use std::borrow::Cow;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatusSummary {
    pub current_branch: Option<String>,
    pub is_dirty: bool,
    pub modified_count: usize,
    pub untracked_count: usize,
    pub staged_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeEntry {
    pub path: String,
    pub head: String,
    pub branch: Option<String>,
    pub is_bare: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDiffFile {
    pub path: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointInfo {
    pub turn_id: String,
    pub ref_name: String,
    pub timestamp: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticCommitSuggestion {
    pub commit_type: String,
    pub scope: Option<String>,
    pub subject: String,
    pub body: String,
    pub full_message: String,
    pub files_changed: Vec<String>,
}

pub enum RepoRef<'a> {
    Path(Cow<'a, Path>),
    Repo(&'a gix::Repository),
    OwnedRepo(gix::Repository),
}

impl<'a> From<&'a Path> for RepoRef<'a> {
    fn from(p: &'a Path) -> Self {
        RepoRef::Path(Cow::Borrowed(p))
    }
}

impl<'a> From<&'a PathBuf> for RepoRef<'a> {
    fn from(p: &'a PathBuf) -> Self {
        RepoRef::Path(Cow::Borrowed(p.as_path()))
    }
}

impl<'a> From<&'a str> for RepoRef<'a> {
    fn from(s: &'a str) -> Self {
        RepoRef::Path(Cow::Borrowed(Path::new(s)))
    }
}

impl From<PathBuf> for RepoRef<'static> {
    fn from(p: PathBuf) -> Self {
        RepoRef::Path(Cow::Owned(p))
    }
}

impl<'a> From<&'a gix::Repository> for RepoRef<'a> {
    fn from(r: &'a gix::Repository) -> Self {
        RepoRef::Repo(r)
    }
}

impl From<gix::Repository> for RepoRef<'static> {
    fn from(r: gix::Repository) -> Self {
        RepoRef::OwnedRepo(r)
    }
}

pub struct GitEngine;
impl GitEngine {
    pub fn discover(path: impl AsRef<Path>) -> Result<gix::Repository, String> {
        gix::discover(path).map_err(|e| e.to_string())
    }

    pub fn is_repo(path: impl AsRef<Path>) -> bool {
        Self::discover(path).is_ok()
    }
    pub fn current_branch<'a>(target: impl Into<RepoRef<'a>>) -> Result<Option<String>, String> {
        match target.into() {
            RepoRef::Path(p) => {
                let repo = Self::discover(p.as_ref())?;
                Self::current_branch_for_repo(&repo)
            }
            RepoRef::Repo(r) => Self::current_branch_for_repo(r),
            RepoRef::OwnedRepo(r) => Self::current_branch_for_repo(&r),
        }
    }

    pub fn is_dirty<'a>(target: impl Into<RepoRef<'a>>) -> Result<bool, String> {
        match target.into() {
            RepoRef::Path(p) => {
                let repo = Self::discover(p.as_ref())?;
                Self::is_dirty_repo(&repo)
            }
            RepoRef::Repo(r) => Self::is_dirty_repo(r),
            RepoRef::OwnedRepo(r) => Self::is_dirty_repo(&r),
        }
    }

    pub fn status_summary<'a>(target: impl Into<RepoRef<'a>>) -> Result<GitStatusSummary, String> {
        match target.into() {
            RepoRef::Path(p) => {
                let repo = Self::discover(p.as_ref())?;
                Self::status_summary_repo(&repo)
            }
            RepoRef::Repo(r) => Self::status_summary_repo(r),
            RepoRef::OwnedRepo(r) => Self::status_summary_repo(&r),
        }
    }

    pub fn current_branch_repo(repo: &gix::Repository) -> Result<Option<String>, String> {
        Self::current_branch_for_repo(repo)
    }
    pub fn current_branch_for_repo(repo: &gix::Repository) -> Result<Option<String>, String> {
        let head = repo.head().map_err(|e| e.to_string())?;
        Ok(head.referent_name().map(|name| name.shorten().to_string()))
    }
    pub fn is_dirty_repo(repo: &gix::Repository) -> Result<bool, String> {
        repo.is_dirty().map_err(|e| e.to_string())
    }
    pub fn status_summary_repo(repo: &gix::Repository) -> Result<GitStatusSummary, String> {
        let current_branch = Self::current_branch_for_repo(repo)?;
        let is_dirty = repo.is_dirty().map_err(|e| e.to_string())?;
        let mut modified_count = 0;
        let mut untracked_count = 0;
        let mut staged_count = 0;
        if let Ok(head_tree_id) = repo.head_tree_id() {
            if let (Ok(head_index), Ok(current_index)) =
                (repo.index_from_tree(&head_tree_id), repo.index_or_empty())
            {
                let head_map: std::collections::HashMap<_, _> = head_index
                    .entries()
                    .iter()
                    .map(|e| (e.path(&head_index), (e.id, e.mode)))
                    .collect();
                let curr_map: std::collections::HashMap<_, _> = current_index
                    .entries()
                    .iter()
                    .map(|e| (e.path(&current_index), (e.id, e.mode)))
                    .collect();

                for (path, meta) in &curr_map {
                    if head_map.get(path) != Some(meta) {
                        staged_count += 1;
                    }
                }
                for path in head_map.keys() {
                    if !curr_map.contains_key(path) {
                        staged_count += 1;
                    }
                }
            }
        } else if let Ok(current_index) = repo.index_or_empty() {
            staged_count = current_index.entries().len();
        }
        let platform = repo
            .status(gix::progress::Discard)
            .map_err(|e| e.to_string())?;
        let iter = platform
            .into_index_worktree_iter(Vec::new())
            .map_err(|e| e.to_string())?;
        for item in iter {
            let item = item.map_err(|e| e.to_string())?;
            if let Some(summary) = item.summary() {
                use gix::status::index_worktree::iter::Summary;
                match summary {
                    Summary::Modified | Summary::Removed | Summary::TypeChange => {
                        modified_count += 1;
                    }
                    Summary::Added => {
                        untracked_count += 1;
                    }
                    Summary::Conflict => {
                        modified_count += 1;
                    }
                    _ => {}
                }
            }
        }

        Ok(GitStatusSummary {
            current_branch,
            is_dirty: is_dirty || untracked_count > 0 || modified_count > 0,
            modified_count,
            untracked_count,
            staged_count,
        })
    }

    pub async fn git_exec(cwd: &Path, args: &[&str]) -> Result<String, String> {
        let mut cmd = tokio::process::Command::new("git");
        cmd.args(args).current_dir(cwd);
        #[cfg(windows)]
        cmd.creation_flags(0x08000000);
        let output = cmd
            .output()
            .await
            .map_err(|e| format!("Failed to spawn git: {e}"))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if err.is_empty() {
                format!(
                    "git {:?} failed with exit code {:?}",
                    args,
                    output.status.code()
                )
            } else {
                err
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    pub async fn get_diff(cwd: &Path, staged: bool) -> Result<String, String> {
        if staged {
            Self::git_exec(cwd, &["diff", "--cached"]).await
        } else {
            Self::git_exec(cwd, &["diff"]).await
        }
    }

    pub async fn list_branches(cwd: &Path) -> Result<Vec<String>, String> {
        let out = Self::git_exec(cwd, &["branch", "--format=%(refname:short)"]).await?;
        Ok(out
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect())
    }

    pub async fn create_branch(cwd: &Path, branch: &str) -> Result<(), String> {
        Self::git_exec(cwd, &["checkout", "-b", branch])
            .await
            .map(|_| ())
    }

    pub async fn checkout(cwd: &Path, target: &str) -> Result<(), String> {
        Self::git_exec(cwd, &["checkout", target]).await.map(|_| ())
    }

    pub async fn commit(cwd: &Path, message: &str) -> Result<String, String> {
        Self::git_exec(cwd, &["commit", "-m", message]).await
    }

    pub async fn worktree_add(
        repo: &Path,
        path: &Path,
        branch: &str,
        create_branch: bool,
    ) -> Result<(), String> {
        let path_str = path.to_str().ok_or("Invalid worktree path")?;
        if create_branch {
            Self::git_exec(repo, &["worktree", "add", "-b", branch, path_str])
                .await
                .map(|_| ())
        } else {
            Self::git_exec(repo, &["worktree", "add", path_str, branch])
                .await
                .map(|_| ())
        }
    }

    pub async fn worktree_remove(repo: &Path, path: &Path) -> Result<(), String> {
        let path_str = path.to_str().ok_or("Invalid worktree path")?;
        Self::git_exec(repo, &["worktree", "remove", "--force", path_str])
            .await
            .map(|_| ())
    }

    pub async fn worktree_list(repo: &Path) -> Result<Vec<WorktreeEntry>, String> {
        let out = Self::git_exec(repo, &["worktree", "list", "--porcelain"]).await?;
        let mut worktrees = Vec::new();
        let mut cur_path = String::new();
        let mut cur_head = String::new();
        let mut cur_branch = None;
        let mut cur_bare = false;

        for line in out.lines() {
            let line = line.trim();
            if line.starts_with("worktree ") {
                if !cur_path.is_empty() {
                    worktrees.push(WorktreeEntry {
                        path: cur_path,
                        head: cur_head,
                        branch: cur_branch,
                        is_bare: cur_bare,
                    });
                    cur_head = String::new();
                    cur_branch = None;
                    cur_bare = false;
                }
                cur_path = line["worktree ".len()..].to_string();
            } else if line.starts_with("HEAD ") {
                cur_head = line["HEAD ".len()..].to_string();
            } else if line.starts_with("branch ") {
                let full = &line["branch ".len()..];
                let short = full.strip_prefix("refs/heads/").unwrap_or(full);
                cur_branch = Some(short.to_string());
            } else if line == "bare" {
                cur_bare = true;
            }
        }

        if !cur_path.is_empty() {
            worktrees.push(WorktreeEntry {
                path: cur_path,
                head: cur_head,
                branch: cur_branch,
                is_bare: cur_bare,
            });
        }

        Ok(worktrees)
    }

    /// Captures the current working tree state into a shadow ref `refs/kronos/checkpoints/<turn_id>`
    pub async fn create_checkpoint(repo: &Path, turn_id: &str) -> Result<String, String> {
        let clean_id = turn_id.replace(['/', '\\', ' '], "-");
        let ref_name = format!("refs/kronos/checkpoints/{}", clean_id);
        let commit_res = Self::git_exec(
            repo,
            &[
                "stash",
                "create",
                &format!("Kronos turn checkpoint {}", clean_id),
            ],
        )
        .await?;
        let hash = commit_res.trim();
        if !hash.is_empty() {
            let _ = Self::git_exec(repo, &["update-ref", &ref_name, hash]).await;
        }
        Ok(ref_name)
    }

    /// Restores the working tree state from the shadow checkpoint
    pub async fn restore_checkpoint(repo: &Path, turn_id: &str) -> Result<(), String> {
        let clean_id = turn_id.replace(['/', '\\', ' '], "-");
        let ref_name = format!("refs/kronos/checkpoints/{}", clean_id);
        if Self::git_exec(repo, &["checkout", &ref_name, "--", "."])
            .await
            .is_ok()
        {
            return Ok(());
        }
        // Fallback: check if any checkpoint matches prefix or get the latest checkpoint
        let refs = Self::git_exec(
            repo,
            &[
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname)",
                "refs/kronos/checkpoints/",
            ],
        )
        .await
        .unwrap_or_default();
        if let Some(target_ref) = refs
            .lines()
            .find(|r| r.contains(&clean_id))
            .or_else(|| refs.lines().next())
        {
            Self::git_exec(repo, &["checkout", target_ref.trim(), "--", "."])
                .await
                .map(|_| ())
        } else {
            Err(format!("Checkpoint '{turn_id}' not found"))
        }
    }

    /// Creates an automated conventional semantic git commit from the current workspace changes.
    pub async fn create_semantic_commit(
        repo: &Path,
        scope: Option<&str>,
        description: &str,
    ) -> Result<String, String> {
        // Stage all modified & untracked changes
        let _ = Self::git_exec(repo, &["add", "-A"]).await?;

        // Determine conventional commit type based on status
        let status = Self::status_summary(repo).unwrap_or(GitStatusSummary {
            current_branch: None,
            is_dirty: true,
            modified_count: 1,
            untracked_count: 0,
            staged_count: 1,
        });

        if !status.is_dirty && status.staged_count == 0 {
            return Ok("Working tree is clean; no commit created".to_string());
        }

        let desc_lower = description.to_lowercase();
        let commit_type = if desc_lower.contains("fix") || desc_lower.contains("bug") || desc_lower.contains("error") {
            "fix"
        } else if desc_lower.contains("test") {
            "test"
        } else if desc_lower.contains("refactor") {
            "refactor"
        } else if desc_lower.contains("doc") || desc_lower.contains("readme") {
            "docs"
        } else {
            "feat"
        };

        let formatted_msg = if let Some(s) = scope {
            format!("{commit_type}({s}): {description}")
        } else {
            format!("{commit_type}: {description}")
        };

        Self::commit(repo, &formatted_msg).await
    }

    /// Creates an isolated git worktree branch for background agent execution.
    pub async fn create_agent_worktree(
        repo: &Path,
        branch_name: &str,
        target_dir: &Path,
    ) -> Result<String, String> {
        let dir_str = target_dir.to_str().ok_or("Invalid worktree path")?;
        Self::git_exec(repo, &["worktree", "add", "-b", branch_name, dir_str, "HEAD"]).await
    }

    /// Removes an isolated agent worktree and prunes tracking references.
    pub async fn remove_agent_worktree(repo: &Path, target_dir: &Path) -> Result<String, String> {
        let dir_str = target_dir.to_str().ok_or("Invalid worktree path")?;
        let res = Self::git_exec(repo, &["worktree", "remove", "--force", dir_str]).await?;
        let _ = Self::git_exec(repo, &["worktree", "prune"]).await;
        Ok(res)
    }

    /// Merges an agent worktree branch into the active branch.
    pub async fn merge_agent_branch(repo: &Path, branch_name: &str) -> Result<String, String> {
        Self::git_exec(repo, &["merge", "--no-ff", "-m", &format!("merge: agent branch {branch_name}"), branch_name]).await
    }

    /// Lists modified, added, and untracked workspace files with old/new text snapshots for multi-file review.
    pub async fn list_diff_files(repo: &Path) -> Result<Vec<WorkspaceDiffFile>, String> {
        let porcelain = Self::git_exec(repo, &["status", "--porcelain"]).await?;
        let mut results = Vec::new();
        for line in porcelain.lines() {
            if line.len() < 4 {
                continue;
            }
            let code = &line[..2];
            let rel = line[3..].trim().trim_matches('"').to_string();
            let status = if code.contains('?') || code.contains('A') {
                "added"
            } else if code.contains('D') {
                "deleted"
            } else {
                "modified"
            };
            let old_text = if status == "added" {
                String::new()
            } else {
                Self::git_exec(repo, &["show", &format!("HEAD:{rel}")])
                    .await
                    .unwrap_or_default()
            };
            let new_text = if status == "deleted" {
                String::new()
            } else {
                tokio::fs::read_to_string(repo.join(&rel))
                    .await
                    .unwrap_or_default()
            };
            let (additions, deletions) = count_diff_stats(&old_text, &new_text);
            results.push(WorkspaceDiffFile {
                path: rel,
                status: status.to_string(),
                additions,
                deletions,
                old_text,
                new_text,
            });
            if results.len() >= 100 {
                break;
            }
        }
        Ok(results)
    }

    /// Reverts selected workspace files back to HEAD (or removes untracked files).
    pub async fn revert_files(repo: &Path, paths: &[String]) -> Result<usize, String> {
        let mut reverted = 0usize;
        for rel in paths {
            let trimmed = rel.trim();
            if trimmed.is_empty() || trimmed.contains("..") {
                continue;
            }
            let tracked = Self::git_exec(repo, &["ls-files", "--error-unmatch", trimmed]).await.is_ok();
            if tracked {
                Self::git_exec(repo, &["checkout", "HEAD", "--", trimmed]).await?;
                reverted += 1;
            } else {
                let target = repo.join(trimmed);
                if target.is_file() {
                    tokio::fs::remove_file(&target)
                        .await
                        .map_err(|e| format!("Failed to remove untracked '{trimmed}': {e}"))?;
                    reverted += 1;
                }
            }
        }
        Ok(reverted)
    }

    /// Lists captured shadow turn checkpoints under `refs/kronos/checkpoints/`.
    pub async fn list_checkpoints(repo: &Path) -> Result<Vec<CheckpointInfo>, String> {
        let out = Self::git_exec(
            repo,
            &[
                "for-each-ref",
                "--sort=-committerdate",
                "--format=%(refname)|%(committerdate:iso-strict)|%(subject)",
                "refs/kronos/checkpoints/",
            ],
        )
        .await
        .unwrap_or_default();

        let mut list = Vec::new();
        for line in out.lines() {
            let mut parts = line.splitn(3, '|');
            let ref_name = parts.next().unwrap_or("").trim();
            if ref_name.is_empty() {
                continue;
            }
            let timestamp = parts.next().unwrap_or("").trim().to_string();
            let summary = parts.next().unwrap_or("Checkpoint").trim().to_string();
            let turn_id = ref_name
                .strip_prefix("refs/kronos/checkpoints/")
                .unwrap_or(ref_name)
                .to_string();
            list.push(CheckpointInfo {
                turn_id,
                ref_name: ref_name.to_string(),
                timestamp,
                summary,
            });
        }
        Ok(list)
    }

    /// Analyzes dirty/staged changes to synthesize a Conventional Commit suggestion.
    pub async fn suggest_semantic_commit(repo: &Path) -> Result<SemanticCommitSuggestion, String> {
        let porcelain = Self::git_exec(repo, &["status", "--porcelain"]).await?;
        let mut files = Vec::new();
        let mut adds = 0usize;
        let mut dels = 0usize;
        let mut mods = 0usize;
        for line in porcelain.lines() {
            if line.len() < 4 {
                continue;
            }
            let code = &line[..2];
            let file = line[3..].trim().trim_matches('"').to_string();
            if code.contains('?') || code.contains('A') {
                adds += 1;
            } else if code.contains('D') {
                dels += 1;
            } else {
                mods += 1;
            }
            files.push(file);
        }

        if files.is_empty() {
            return Err("Working tree is clean; nothing to commit".to_string());
        }

        let diff_sample = Self::git_exec(repo, &["diff", "--stat"]).await.unwrap_or_default();
        let suggestion = build_semantic_commit_suggestion(&files, adds, dels, mods, &diff_sample);
        Ok(suggestion)
    }
}

fn count_diff_stats(old_text: &str, new_text: &str) -> (usize, usize) {
    let diff = similar::TextDiff::from_lines(old_text, new_text);
    let mut additions = 0usize;
    let mut deletions = 0usize;
    for change in diff.iter_all_changes() {
        match change.tag() {
            similar::ChangeTag::Insert => additions += 1,
            similar::ChangeTag::Delete => deletions += 1,
            similar::ChangeTag::Equal => {}
        }
    }
    (additions, deletions)
}

pub fn build_semantic_commit_suggestion(
    files: &[String],
    adds: usize,
    _dels: usize,
    mods: usize,
    _diff_stat: &str,
) -> SemanticCommitSuggestion {
    let all_tests = !files.is_empty()
        && files
            .iter()
            .all(|f| f.contains(".test.") || f.contains("_test.") || f.contains("/tests/"));
    let all_docs = !files.is_empty()
        && files
            .iter()
            .all(|f| f.ends_with(".md") || f.to_lowercase().contains("readme"));

    let commit_type = if all_tests {
        "test"
    } else if all_docs {
        "docs"
    } else if adds > 0 {
        "feat"
    } else if mods > 0 {
        "refactor"
    } else {
        "chore"
    }
    .to_string();

    // Infer common package/directory scope
    let scope = files
        .first()
        .and_then(|f| {
            let parts: Vec<&str> = f.split(['/', '\\']).collect();
            if parts.len() >= 2 && (parts[0] == "packages" || parts[0] == "src") {
                Some(parts[1].to_string())
            } else if parts.len() >= 2 {
                Some(parts[0].to_string())
            } else {
                None
            }
        });

    let file_names: Vec<String> = files
        .iter()
        .take(3)
        .filter_map(|f| f.split(['/', '\\']).next_back().map(String::from))
        .collect();
    let suffix = if files.len() > 3 {
        format!(" and {} more", files.len() - 3)
    } else {
        String::new()
    };
    let subject = format!(
        "update {}{}",
        file_names.join(", "),
        suffix
    );
    let body = files
        .iter()
        .take(15)
        .map(|f| format!("- {f}"))
        .collect::<Vec<_>>()
        .join("\n");

    let header = match &scope {
        Some(s) => format!("{commit_type}({s}): {subject}"),
        None => format!("{commit_type}: {subject}"),
    };
    let full_message = format!("{header}\n\n{body}");

    SemanticCommitSuggestion {
        commit_type,
        scope,
        subject,
        body,
        full_message,
        files_changed: files.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_commit_classifier_detects_test_and_docs_and_feat() {
        let test_res = build_semantic_commit_suggestion(
            &["packages/app/src/foo.test.ts".to_string()],
            1,
            0,
            0,
            "",
        );
        assert_eq!(test_res.commit_type, "test");
        assert_eq!(test_res.scope.as_deref(), Some("app"));

        let docs_res = build_semantic_commit_suggestion(&["missingfeatures.md".to_string()], 0, 0, 1, "");
        assert_eq!(docs_res.commit_type, "docs");

        let feat_res = build_semantic_commit_suggestion(
            &["packages/desktop-tauri/src-tauri/src/main.rs".to_string()],
            1,
            0,
            0,
            "",
        );
        assert_eq!(feat_res.commit_type, "feat");
        assert_eq!(feat_res.scope.as_deref(), Some("desktop-tauri"));
    }

    #[test]
    fn diff_stats_counts_insertions_and_deletions() {
        let (add, del) = count_diff_stats("a\nb\nc\n", "a\nx\ny\nc\n");
        assert_eq!(add, 2);
        assert_eq!(del, 1);
    }
}
