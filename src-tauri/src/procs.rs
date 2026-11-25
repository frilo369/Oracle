//! Read-only process discovery for League's local connection.
use windows_sys::Win32::{
    Foundation::{CloseHandle, GetLastError, ERROR_NO_MORE_FILES, INVALID_HANDLE_VALUE},
    System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    },
};
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub name: String,
}
/// Every running process with its number and lower-case executable name.
pub fn list() -> std::io::Result<Vec<ProcessEntry>> {
    let mut out = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) == 0 {
            let error = std::io::Error::last_os_error();
            CloseHandle(snapshot);
            return Err(error);
        }
        loop {
            let len = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            out.push(ProcessEntry {
                pid: entry.th32ProcessID,
                name: String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase(),
            });
            if Process32NextW(snapshot, &mut entry) == 0 {
                let code = GetLastError();
                CloseHandle(snapshot);
                if code != ERROR_NO_MORE_FILES {
                    return Err(std::io::Error::from_raw_os_error(code as i32));
                }
                break;
            }
        }
    }
    Ok(out)
}
