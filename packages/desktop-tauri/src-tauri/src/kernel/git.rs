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
}
