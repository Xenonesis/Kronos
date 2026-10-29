pub mod generated_types;
pub mod protocol;
pub mod server;

pub mod git;
pub mod shm;
pub mod vfs;

#[cfg(test)]
mod protocol_test;
#[cfg(test)]
pub mod server_test;
#[cfg(test)]
pub mod shm_test;
#[cfg(test)]
pub mod vfs_test;
