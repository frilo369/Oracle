use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

pub fn defaults() -> Value {
    json!({"guideHotkey":"F1","assetColour":"#ae1236","theme":"umbra","launchInTray":true,"trayDefault13":true,"startWithWindows":false,"lastGuide":{"champion":103,"mode":"rift"}})
}
pub fn valid(key: &str, value: &Value) -> bool {
    match key {
        "guideHotkey" => {
            value.is_null()
                || value
                    .as_str()
                    .is_some_and(|s| crate::guide_keys::Binding::parse(s).is_some())
        }
        "assetColour" => value.as_str().is_some_and(|s| {
            s.len() == 7 && s.starts_with('#') && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
        }),
        "theme" => value == "umbra",
        "launchInTray" | "startWithWindows" | "trayDefault13" => value.is_boolean(),
        "lastGuide" => value.as_object().is_some_and(|m| {
            m.len() == 2
                && value["champion"]
                    .as_u64()
                    .is_some_and(|id| id > 0 && id < 10000)
                && value["mode"]
                    .as_str()
                    .is_some_and(|m| matches!(m, "rift" | "aram" | "arena" | "mayhem"))
        }),
        "favorites" => value.as_array().is_some_and(|a| {
            a.len() <= 200
                && a.iter()
                    .all(|v| v.as_u64().is_some_and(|id| id > 0 && id < 10000))
        }),
        "notes" => value.as_object().is_some_and(|m| {
            m.len() <= 200
                && m.iter().all(|(k, v)| {
                    k.parse::<u64>().is_ok_and(|id| id > 0 && id < 10000)
                        && v.as_str().is_some_and(|s| s.len() <= 8000)
                })
        }),
        _ => false,
    }
}
pub fn load(root: &Path) -> (Value, Option<String>) {
    let path = root.join("settings.json");
    let mut result = defaults();
    match fs::read(&path) {
        Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
            Ok(doc) if doc.is_object() => {
                for (key,value) in doc.as_object().unwrap() { if valid(key,value) { result[key] = value.clone(); } }
                // One-time adoption of the owner's new tray default. A later opt-out is kept.
                if doc["trayDefault13"] != true { result["launchInTray"]=json!(true);result["trayDefault13"]=json!(true); }
                (result,None)
            }
            _ => (result, Some("Settings could not be read. The original file is preserved; save settings to replace it.".into())),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (result,None),
        Err(_) => (result,Some("Settings could not be read. Check access to Oracle's data folder.".into())),
    }
}
pub fn save(path: &Path, value: &Value) -> Result<(), String> {
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let folder = path.parent().ok_or("Missing settings folder")?;
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let temporary = folder.join(format!(
        ".oracle-{}-{}.tmp",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let write = || -> Result<(), String> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        drop(file);
        let wide = |p: &Path| {
            use std::os::windows::ffi::OsStrExt;
            p.as_os_str()
                .encode_wide()
                .chain(Some(0))
                .collect::<Vec<_>>()
        };
        if unsafe {
            MoveFileExW(
                wide(&temporary).as_ptr(),
                wide(path).as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    };
    let result = write();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
pub struct StartupChange {
    before: Option<winreg::RegValue<'static>>,
    after: Option<winreg::RegValue<'static>>,
}
impl StartupChange {
    pub fn rollback(self) -> Result<(), String> {
        use winreg::{enums::HKEY_CURRENT_USER, RegKey};
        let run = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                winreg::enums::KEY_READ | winreg::enums::KEY_WRITE,
            )
            .map_err(|e| e.to_string())?;
        let current = startup_value(&run)?;
        if current != self.after {
            return Err(
                "Windows startup changed during saving; the newer entry was preserved".into(),
            );
        }
        match self.before {
            Some(value) => run.set_raw_value("Oracle", &value),
            None => match run.delete_value("Oracle") {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                result => result,
            },
        }
        .map_err(|e| e.to_string())
    }
}
fn startup_value(run: &winreg::RegKey) -> Result<Option<winreg::RegValue<'static>>, String> {
    match run.get_raw_value("Oracle") {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}
fn owns_startup(current: &str, executable: &Path) -> bool {
    current.eq_ignore_ascii_case(&format!("\"{}\" --tray", executable.display()))
}
pub fn startup(enabled: bool) -> Result<StartupChange, String> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let (run, _) = RegKey::predef(HKEY_CURRENT_USER)
        .create_subkey(r"Software\Microsoft\Windows\CurrentVersion\Run")
        .map_err(|e| e.to_string())?;
    let before = startup_value(&run)?;
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    if enabled {
        run.set_value("Oracle", &format!("\"{}\" --tray", executable.display()))
            .map_err(|e| e.to_string())?;
    } else if run
        .get_value::<String, _>("Oracle")
        .ok()
        .is_some_and(|current| owns_startup(&current, &executable))
    {
        match run.delete_value("Oracle") {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }?;
    }
    Ok(StartupChange {
        before,
        after: startup_value(&run)?,
    })
}

#[test]
fn startup_removal_only_owns_this_executable() {
    let exe = Path::new(r"C:\Oracle\oracle.exe");
    assert!(owns_startup(r#""c:\oracle\Oracle.exe" --tray"#, exe));
    assert!(!owns_startup(r#""C:\Other\oracle.exe" --tray"#, exe));
    assert!(!owns_startup(
        r#""C:\Oracle\oracle.exe" --tray --extra"#,
        exe
    ));
}

#[test]
fn settings_validate_and_replace_atomically() {
    let dir = tempfile::tempdir().unwrap();
    let mut value = defaults();
    assert!(valid("guideHotkey", &json!("Ctrl+Shift+KeyG")));
    assert!(!valid("guideHotkey", &json!("Ctrl+Ctrl+F1")));
    assert!(!valid("assetColour", &json!("red; background:url(x)")));
    assert!(!valid("notes", &json!({"../oops":"bad"})));
    save(&dir.path().join("settings.json"), &value).unwrap();
    value["guideHotkey"] = json!("F8");
    save(&dir.path().join("settings.json"), &value).unwrap();
    assert_eq!(load(dir.path()).0["guideHotkey"], "F8");
    fs::write(dir.path().join("settings.json"), b"broken").unwrap();
    assert!(load(dir.path()).1.is_some());
    assert_eq!(
        fs::read(dir.path().join("settings.json")).unwrap(),
        b"broken"
    );
}

#[test]
fn tray_default_migrates_once_and_retired_notes_stay_preserved() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(load(dir.path()).0["launchInTray"], true);
    let legacy = json!({"guideHotkey":"F8","launchInTray":false,"notes":{"103":"Legacy 🤍"},"favorites":[103]});
    save(&dir.path().join("settings.json"), &legacy).unwrap();
    let (mut migrated, error) = load(dir.path());
    assert!(error.is_none());
    assert_eq!(migrated["launchInTray"], true);
    assert_eq!(migrated["guideHotkey"], "F8");
    assert_eq!(migrated["notes"], legacy["notes"]);
    assert_eq!(migrated["favorites"], legacy["favorites"]);
    migrated["launchInTray"] = json!(false);
    save(&dir.path().join("settings.json"), &migrated).unwrap();
    assert_eq!(
        load(dir.path()).0["launchInTray"],
        false,
        "a deliberate opt-out survives restart"
    );
    assert!(valid("lastGuide", &json!({"champion":103,"mode":"arena"})));
    assert!(!valid("lastGuide", &json!({"champion":0,"mode":"unknown"})));
}
