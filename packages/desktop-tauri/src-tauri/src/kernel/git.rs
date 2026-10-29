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
            if let (Ok(head_index), Ok(current_index)) = (repo.index_from_tree(&head_tree_id), repo.index_or_empty()) {
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
        let platform = repo.status(gix::progress::Discard).map_err(|e| e.to_string())?;
        let iter = platform.into_index_worktree_iter(Vec::new()).map_err(|e| e.to_string())?;
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
}
