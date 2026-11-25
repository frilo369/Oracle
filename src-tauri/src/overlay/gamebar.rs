//! Fixed per-user package inbox. Only aggregate labels cross this boundary.
use super::{capture::Area, window::Label};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
pub const FAMILY: &str = "Frilo369.OracleWidget_9y1ac7xxt8800";
pub fn configured(path: Option<&Path>) -> bool {
    path.and_then(|p| std::fs::read(p.join("widget-status.json")).ok())
        .filter(|b| b.len() < 4096)
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .is_some_and(|s| s["schema"] == 1 && s["pinned"] == true && s["clickThrough"] == true)
}
pub fn activate() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    // A widget command launches its host, rather than opening Game Bar's home UI.
    // Never change the user's pin, click-through, keyboard or League settings.
    std::process::Command::new("explorer.exe")
        .arg("ms-gamebar://launch/activate/Frilo369.OracleWidget_9y1ac7xxt8800_App_OracleAugments")
        .creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|_| "Could not open Game Bar".into())
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn folder(root: &Path, isolated: bool) -> Option<PathBuf> {
    if isolated {
        return Some(root.join("gamebar"));
    }
    let path = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("Packages")
        .join(FAMILY)
        .join("LocalState");
    path.is_dir().then_some(path)
}
pub fn health(path: Option<&Path>) -> (&'static str, &'static str) {
    let Some(path) = path else {
        return ("setup", "Install the Oracle Game Bar companion");
    };
    let status = std::fs::read(path.join("widget-status.json"))
        .ok()
        .filter(|b| b.len() < 4096)
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let Some(status) = status.filter(|s| {
        s["schema"] == 1
            && s["at"]
                .as_u64()
                .is_some_and(|at| now().checked_sub(at).is_some_and(|age| age < 3000))
    }) else {
        return (
            "setup",
            "Open Oracle in Game Bar once, pin it and enable click-through",
        );
    };
    if status["pinned"] != true {
        return ("setup", "Pin Oracle in Game Bar");
    }
    if status["clickThrough"] != true {
        return (
            "setup",
            "Enable Game Bar click-through so League keeps mouse input",
        );
    }
    if status["visible"] != true {
        return ("setup", "Restore the Oracle Game Bar widget");
    }
    ("ready", "Arena + Mayhem · Game Bar ready")
}
pub fn publish(path: &Path, area: Option<Area>, labels: &[Label]) -> Result<(), String> {
    let a = area.unwrap_or(Area {
        x: 0,
        y: 0,
        w: 0,
        h: 0,
    });
    crate::settings::save(
        &path.join("overlay.json"),
        &json!({"schema":1,"at":now(),"visible":!labels.is_empty()&&area.is_some(),"screenX":a.x,"screenY":a.y,"width":a.w,"height":a.h,"labels":labels.iter().map(|l|json!({"x":l.x,"y":l.y,"text":l.text})).collect::<Vec<_>>()}),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_or_unpinned_widgets_are_not_reported_ready() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(health(None).0, "setup");
        let mut value =
            json!({"schema":1,"at":now(),"pinned":false,"clickThrough":true,"visible":true});
        crate::settings::save(&root.path().join("widget-status.json"), &value).unwrap();
        assert_eq!(health(Some(root.path())).1, "Pin Oracle in Game Bar");
        value["pinned"] = json!(true);
        crate::settings::save(&root.path().join("widget-status.json"), &value).unwrap();
        assert_eq!(health(Some(root.path())).0, "ready");
        assert!(configured(Some(root.path())));
        value["at"] = json!(1);
        crate::settings::save(&root.path().join("widget-status.json"), &value).unwrap();
        assert_eq!(health(Some(root.path())).0, "setup");
        assert!(
            configured(Some(root.path())),
            "setup acknowledgement survives host suspension"
        );
    }
}
