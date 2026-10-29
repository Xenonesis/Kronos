#[cfg(windows)]
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
#[cfg(windows)]
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

pub struct JobSupervisor {
    #[cfg(windows)]
    job_handle: HANDLE,
}

unsafe impl Send for JobSupervisor {}
unsafe impl Sync for JobSupervisor {}

impl JobSupervisor {
    pub fn new() -> Result<Self, String> {
        #[cfg(windows)]
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() || job == INVALID_HANDLE_VALUE {
                return Err("Failed to create Win32 Job Object".to_string());
            }

            let mut info = std::mem::zeroed::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let res = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );

            if res == 0 {
                CloseHandle(job);
                return Err("Failed to set Job Object extended limit information".to_string());
            }

            Ok(Self { job_handle: job })
        }

        #[cfg(not(windows))]
        Ok(Self {})
    }

    #[cfg(windows)]
    pub fn assign_process_raw(&self, process_handle: HANDLE) -> Result<(), String> {
        unsafe {
            if AssignProcessToJobObject(self.job_handle, process_handle) == 0 {
                return Err("Failed to assign process to Job Object".to_string());
            }
            Ok(())
        }
    }

    #[cfg(windows)]
    pub fn assign_process(&self, process_handle: HANDLE) -> Result<(), String> {
        self.assign_process_raw(process_handle)
    }

    #[cfg(not(windows))]
    pub fn assign_process_raw(&self, _process_handle: *mut std::ffi::c_void) -> Result<(), String> {
        Ok(())
    }

    #[cfg(not(windows))]
    pub fn assign_process(&self, _process_handle: *mut std::ffi::c_void) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for JobSupervisor {
    fn drop(&mut self) {
        unsafe {
            if !self.job_handle.is_null() && self.job_handle != INVALID_HANDLE_VALUE {
                CloseHandle(self.job_handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_supervisor_creation() {
        let supervisor = JobSupervisor::new();
        assert!(supervisor.is_ok(), "JobSupervisor should initialize successfully on Windows");
    }

    #[test]
    fn test_assign_process_invalid_handle() {
        let supervisor = JobSupervisor::new().expect("JobSupervisor creation should succeed");
        #[cfg(windows)]
        {
            let res = supervisor.assign_process(std::ptr::null_mut());
            assert!(res.is_err(), "Assigning null handle should fail");
        }
    }
}
