pub mod generated_types;
pub mod protocol;
pub mod server;

pub mod git;
pub mod shm;
pub mod vfs;
pub mod ast;
pub mod search;
pub mod crdt;
pub mod cli;
pub mod ioring;
pub mod mcp;
pub mod speech;
pub mod rkyv_shm;
pub mod telemetry;
pub mod watcher;
pub mod browser_proxy;
pub mod auth;
pub mod access;
pub mod timeline;
pub mod config_store;
pub mod file_service;
pub mod terminal;
pub mod services;
pub mod schedules;
pub mod relay_crypto;
pub mod watchdog;
pub mod repo_map;
pub mod tunnel;
pub mod workspace_tools;
pub mod sqlite_timeline;
pub mod lsp;
pub mod notes_store;
pub mod ci_repair;

#[cfg(test)]
mod protocol_test;
#[cfg(test)]
pub mod server_test;
#[cfg(test)]
pub mod shm_test;
#[cfg(test)]
pub mod vfs_test;
#[cfg(test)]
pub mod search_test;
#[cfg(test)]
pub mod crdt_test;
#[cfg(test)]
pub mod cli_test;
#[cfg(test)]
pub mod ioring_test;
#[cfg(test)]
pub mod mcp_test;
#[cfg(test)]
pub mod speech_test;
#[cfg(test)]
pub mod rkyv_test;
#[cfg(test)]
pub mod watcher_test;
#[cfg(test)]
pub mod ast_test;
#[cfg(test)]
pub mod lsp_test;
