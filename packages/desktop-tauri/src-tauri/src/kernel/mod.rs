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
