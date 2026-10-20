//! Windows child containment. A process starts suspended so it cannot create
//! descendants before it belongs to the job. Closing the job kills the tree.
use std::os::windows::{
    io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::CommandExt,
};
use windows_sys::Win32::{
    Foundation::{HANDLE, INVALID_HANDLE_VALUE},
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
        },
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
            SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        },
        Threading::{OpenThread, ResumeThread, CREATE_SUSPENDED, THREAD_SUSPEND_RESUME},
    },
};

fn last_error(operation: &str) -> String {
    format!("{operation}: {}", std::io::Error::last_os_error())
}

// Every accepted handle is freshly allocated and owned by this module.
unsafe fn owned(handle: HANDLE, operation: &str) -> Result<OwnedHandle, String> {
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        Err(last_error(operation))
    } else {
        Ok(OwnedHandle::from_raw_handle(handle))
    }
}

pub fn prepare(command: &mut tokio::process::Command) {
    command.as_std_mut().creation_flags(CREATE_SUSPENDED);
}

pub struct ProcessTree {
    // OwnedHandle closes the job on every return path, including cancellation
    // and a dropped execution future. It is also Send, unlike a bare HANDLE.
    _job: OwnedHandle,
}

impl ProcessTree {
    /// Call immediately after spawning with `prepare` and `kill_on_drop(true)`.
    /// On failure the caller must drop the child, which is still suspended.
    pub fn attach(child: &tokio::process::Child) -> Result<Self, String> {
        let pid = child.id().ok_or("Child has no PID")?;
        let process = child.raw_handle().ok_or("Child has no process handle")?;
        // SAFETY: fresh handles have one owner; borrowed child handle remains
        // valid during this call. All structs use the Windows ABI and sizes.
        unsafe {
            let job = owned(
                CreateJobObjectW(std::ptr::null(), std::ptr::null()),
                "Create process job",
            )?;
            let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            if SetInformationJobObject(
                job.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const std::ffi::c_void,
                std::mem::size_of_val(&limits) as u32,
            ) == 0
            {
                return Err(last_error("Configure process job"));
            }
            if AssignProcessToJobObject(job.as_raw_handle(), process) == 0 {
                return Err(last_error("Attach process to job"));
            }
            let snapshot = owned(
                CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0),
                "Inspect suspended process thread",
            )?;
            let mut thread: THREADENTRY32 = std::mem::zeroed();
            thread.dwSize = std::mem::size_of_val(&thread) as u32;
            let mut found = Thread32First(snapshot.as_raw_handle(), &mut thread);
            while found != 0 {
                if thread.th32OwnerProcessID == pid {
                    let primary = owned(
                        OpenThread(THREAD_SUSPEND_RESUME, 0, thread.th32ThreadID),
                        "Open suspended process thread",
                    )?;
                    if ResumeThread(primary.as_raw_handle()) == u32::MAX {
                        return Err(last_error("Resume contained process"));
                    }
                    return Ok(Self { _job: job });
                }
                thread.dwSize = std::mem::size_of_val(&thread) as u32;
                found = Thread32Next(snapshot.as_raw_handle(), &mut thread);
            }
            Err("Could not find the suspended process thread".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{process::Stdio, time::Duration};
    use windows_sys::Win32::{
        Foundation::WAIT_OBJECT_0,
        System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE},
    };

    const FIXTURE: &str = "OPENSTREAMDECK_PROCESS_TREE_TEST_FIXTURE";
    const PID_FILE: &str = "OPENSTREAMDECK_PROCESS_TREE_TEST_PID_FILE";
    const TEST: &str = "process_tree::tests::subprocess_fixture";

    // Explicit env gate: ordinary CI executes this as a no-op. Only a test-owned
    // subprocess runs the fixture; it never starts a shell, agent or hardware.
    #[test]
    fn subprocess_fixture() {
        let Ok(mode) = std::env::var(FIXTURE) else {
            return;
        };
        if mode == "parent" {
            let mut child = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(FIXTURE, "descendant")
                .spawn()
                .unwrap();
            let path = std::env::var_os(PID_FILE).unwrap();
            std::fs::write(path, child.id().to_string()).unwrap();
            let _ = child.wait();
        } else if mode == "descendant" {
            loop {
                std::thread::sleep(Duration::from_secs(60));
            }
        } else {
            panic!("Unknown subprocess fixture mode");
        }
    }

    #[tokio::test]
    async fn dropping_job_terminates_parent_and_descendant() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("descendant.pid");
        let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", TEST, "--nocapture"])
            .env(FIXTURE, "parent")
            .env(PID_FILE, &pid_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        prepare(&mut command);
        let mut child = command.spawn().unwrap();
        let tree = ProcessTree::attach(&child).unwrap();
        let descendant = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if let Ok(text) = tokio::fs::read_to_string(&pid_file).await {
                    if let Ok(pid) = text.parse::<u32>() {
                        break pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("Fixture did not publish its descendant PID");
        // Acquire the handle while alive so PID reuse cannot yield false success.
        let descendant = unsafe {
            owned(
                OpenProcess(PROCESS_SYNCHRONIZE, 0, descendant),
                "Open fixture descendant",
            )
        }
        .unwrap();
        drop(tree);
        let status = tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(!status.success());
        assert_eq!(
            unsafe { WaitForSingleObject(descendant.as_raw_handle(), 5000) },
            WAIT_OBJECT_0
        );
    }
}
