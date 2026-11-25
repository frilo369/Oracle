//! Stable public GitHub releases only. Download/verification never blocks the UI.
use crate::shell::Desktop;
use semver::Version;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::windows::{fs::OpenOptionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager};

const API: &str = "https://api.github.com/repos/frilo369/Oracle/releases/latest";
const DOWNLOAD: &str = "https://github.com/frilo369/Oracle/releases/download";
const MAX_SETUP: u64 = 100 * 1024 * 1024;

#[derive(Default)]
pub struct Updater {
    busy: AtomicBool,
    scenario: Mutex<String>,
    revision: AtomicU64,
}

pub fn initial(root: &Path) -> Value {
    let path = root.join("update-result.json");
    let result = fs::File::open(&path).ok().and_then(|file| {
        let mut bytes = Vec::new();
        file.take(4096).read_to_end(&mut bytes).ok()?;
        (bytes.len() < 4096)
            .then(|| serde_json::from_slice::<Value>(&bytes).ok())
            .flatten()
    });
    if result.is_some() {
        let _ = fs::remove_file(path);
    }
    match result {
        Some(v) if v["ok"] == true => {
            json!({"revision":0,"status":"current","busy":false,"message":format!("Oracle {} is installed.", env!("CARGO_PKG_VERSION"))})
        }
        Some(_) => {
            json!({"revision":0,"status":"error","busy":false,"message":"The update failed. Check for updates to retry; see Oracle-Setup.log in your temporary folder for details."})
        }
        None => {
            json!({"revision":0,"status":"idle","busy":false,"message":"Updates install automatically at startup, outside a match."})
        }
    }
}

fn status(app: &AppHandle, kind: &str, message: impl Into<String>) {
    let state = app.state::<Desktop>();
    let update = json!({"revision":state.updater.revision.fetch_add(1,Ordering::AcqRel)+1,"status":kind,"message":message.into(),"busy":state.updater.busy.load(Ordering::Acquire)});
    if let Ok(mut view) = state.view.lock() {
        view["update"] = update.clone();
    }
    if let Some(w) = app.get_webview_window("dashboard") {
        let _ = w.emit("oracle-update", update);
    }
}

fn trusted_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "api.github.com"
                    | "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
}

fn http() -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent(concat!("Oracle/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(180))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 5 || !trusted_url(attempt.url()) {
                attempt.error("Untrusted update redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Could not prepare the update connection.".into())
}

#[derive(Debug)]
struct Release {
    version: Version,
    name: String,
    url: String,
    size: u64,
    hash: String,
}
fn release(doc: &Value, current: &Version) -> Result<Option<Release>, String> {
    if doc["draft"] != false || doc["prerelease"] != false {
        return Err("No stable release is available.".into());
    }
    let tag = doc["tag_name"]
        .as_str()
        .ok_or("The release has no version.")?;
    let version = Version::parse(tag.strip_prefix('v').ok_or("Invalid release version.")?)
        .map_err(|_| "Invalid release version.")?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err("Only stable versions can be installed.".into());
    }
    if &version <= current {
        return Ok(None);
    }
    let label = version.to_string();
    let label = label.strip_suffix(".0").unwrap_or(&label);
    let name = format!("Oracle-{label}-Setup.exe");
    let asset = doc["assets"]
        .as_array()
        .ok_or("No installer was published.")?
        .iter()
        .find(|a| a["name"] == name && a["state"] == "uploaded")
        .ok_or("The release installer is not ready yet.")?;
    let url = format!("{DOWNLOAD}/{tag}/{name}");
    if asset["browser_download_url"] != url {
        return Err("The installer address is not from Oracle's repository.".into());
    }
    let size = asset["size"]
        .as_u64()
        .filter(|s| (2..=MAX_SETUP).contains(s))
        .ok_or("Invalid installer size.")?;
    let hash = asset["digest"]
        .as_str()
        .and_then(|s| s.strip_prefix("sha256:"))
        .filter(|h| h.len() == 64 && h.bytes().all(|c| c.is_ascii_hexdigit()))
        .ok_or("The release has no SHA-256 verification digest.")?;
    Ok(Some(Release {
        version,
        name,
        url,
        size,
        hash: hash.to_ascii_lowercase(),
    }))
}

fn verify_download(
    mut source: impl Read,
    file: &mut impl Write,
    release: &Release,
) -> Result<(), String> {
    let mut buffer = [0u8; 64 * 1024];
    let mut size = 0u64;
    let mut hash = Sha256::new();
    let mut magic = [0u8; 2];
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|_| "The download was interrupted. Check for updates to retry.")?;
        if count == 0 {
            break;
        }
        if size < 2 {
            let copied = count.min(2 - size as usize);
            magic[size as usize..size as usize + copied].copy_from_slice(&buffer[..copied]);
            if size + count as u64 >= 2 && &magic != b"MZ" {
                return Err("The download is not a Windows installer.".into());
            }
        }
        size += count as u64;
        if size > release.size || size > MAX_SETUP {
            return Err("The installer exceeds its published size.".into());
        }
        file.write_all(&buffer[..count])
            .map_err(|_| "Could not save the update. Check free disk space.")?;
        hash.update(&buffer[..count]);
    }
    if size != release.size || format!("{:x}", hash.finalize()) != release.hash {
        return Err(
            "Installer verification failed. Nothing was installed; check for updates to retry."
                .into(),
        );
    }
    Ok(())
}

fn installed(exe: &Path, local: &Path) -> bool {
    // Windows paths may contain either separator, different casing, or a verbatim
    // prefix. Comparing their display strings misclassified real installed copies.
    let expected = local.join("oracle").join("bin").join("oracle.exe");
    let exe = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
    let expected = expected.canonicalize().unwrap_or(expected);
    exe.components()
        .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase())
        .eq(expected
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_ascii_lowercase()))
}

fn safe_to_install(view: &Value, game_running: bool) -> bool {
    !game_running
        && view["guideTarget"].is_null()
        && view["guideCapture"] != true
        && !matches!(
            view["phase"].as_str(),
            Some("ChampSelect" | "GameStart" | "InProgress" | "Reconnect")
        )
}

// Delete only our own flat download folders; never recursively remove user files or links.
fn cleanup(root: &Path) {
    use std::os::windows::fs::MetadataExt;
    let Ok(entries) = fs::read_dir(root.join("updates")) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !entry.file_name().to_string_lossy().starts_with("download-")
            || !meta.is_dir()
            || meta.file_attributes() & 0x400 != 0
        {
            continue;
        }
        if let Ok(files) = fs::read_dir(entry.path()) {
            for file in files.flatten() {
                let name = file.file_name();
                let name = name.to_string_lossy();
                if name.starts_with("Oracle-") && name.ends_with("-Setup.exe") {
                    let _ = fs::remove_file(file.path());
                }
            }
        }
        let _ = fs::remove_dir(entry.path());
    }
}

pub fn startup(app: AppHandle) {
    let state = app.state::<Desktop>();
    let fixture_startup =
        state.preview && std::env::args().any(|a| a == "--fixture-update-startup");
    if state.isolated && !fixture_startup {
        return;
    }
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(5));
        let state = app.state::<Desktop>();
        if !state.is_closing() && !std::env::args().any(|a| a == "--skip-update-once") {
            if state.preview {
                crate::support::event(&state.root, "update-startup", json!({"checked":true}));
            }
            request(app);
        }
    });
}

pub fn request(app: AppHandle) {
    let state = app.state::<Desktop>();
    if state.is_closing()
        || state
            .updater
            .busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    status(&app, "checking", "Checking for updates…");
    std::thread::spawn(move || {
        let result = run(&app);
        let state = app.state::<Desktop>();
        state.updater.busy.store(false, Ordering::Release);
        match result {
            Err(error) => status(&app, "error", error),
            Ok(()) => {
                let previous = state.view.lock().unwrap()["update"].clone();
                status(
                    &app,
                    previous["status"].as_str().unwrap_or("idle"),
                    previous["message"].as_str().unwrap_or(""),
                );
            }
        }
    });
}

fn run(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<Desktop>();
    if state.isolated && !state.preview {
        return Err("Updates are unavailable in this test copy.".into());
    }
    cleanup(&state.root);
    let scenario = state.updater.scenario.lock().unwrap().clone();
    let current = Version::parse(env!("CARGO_PKG_VERSION")).unwrap();
    let bytes = b"MZOracle update fixture";
    let client = if state.preview { None } else { Some(http()?) };
    let doc = if let Some(http) = &client {
        let response = http
            .get(API)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .timeout(Duration::from_secs(15))
            .send()
            .map_err(|_| "Could not reach GitHub. Check your connection and try again.")?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err("No Oracle release has been published yet.".into());
        }
        crate::guide::capped_json(response,1024*1024).map_err(|_| "Could not read the latest release. GitHub may be temporarily unavailable or rate limiting requests.")?
    } else {
        if scenario == "offline" {
            return Err("Could not reach GitHub. Check your connection and try again.".into());
        }
        fixture_release(
            &current,
            scenario != "current" && !scenario.is_empty(),
            bytes,
        )
    };
    let Some(release) = release(&doc, &current)? else {
        status(app, "current", format!("Oracle {current} is up to date."));
        return Ok(());
    };
    if !state.preview
        && !std::env::var_os("LOCALAPPDATA")
            .is_some_and(|p| std::env::current_exe().is_ok_and(|e| installed(&e, Path::new(&p))))
    {
        status(app,"available",format!("Oracle {} is available. Install the latest setup from github.com/frilo369/Oracle/releases to update this portable copy.", release.version));
        return Ok(());
    }
    status(
        app,
        "downloading",
        format!("Downloading Oracle {}…", release.version),
    );
    let folder = state.root.join("updates");
    fs::create_dir_all(&folder).map_err(|_| "Could not create the update folder.")?;
    let directory = tempfile::Builder::new()
        .prefix("download-")
        .tempdir_in(folder)
        .map_err(|_| "Could not prepare the update folder.")?;
    let path: PathBuf = directory.path().join(&release.name);
    // Download first, then reopen read-only and verify under a lock that denies writes/deletes.
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(1)
        .open(&path)
        .map_err(|_| "Could not create the installer download.")?;
    if let Some(http) = &client {
        let response = http
            .get(&release.url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .map_err(|_| "Could not download the installer. Check for updates to retry.")?;
        verify_download(response, &mut file, &release)?;
    } else {
        let payload: &[u8] = if scenario == "corrupt" {
            b"MZdamaged"
        } else {
            bytes
        };
        verify_download(payload, &mut file, &release)?;
    }
    file.sync_all()
        .map_err(|_| "Could not finish saving the update.".to_string())?;
    drop(file);
    let mut locked = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)
        .map_err(|_| "Could not lock the verified update.".to_string())?;
    verify_download(&mut locked, &mut std::io::sink(), &release)?;
    status(
        app,
        "waiting",
        format!(
            "Oracle {} is ready. Waiting for the match or shortcut recording to finish…",
            release.version
        ),
    );
    loop {
        if state.is_closing() {
            return Ok(());
        }
        let safe = safe_to_install(
            &state.view.lock().unwrap(),
            !state.preview && !crate::switcher::game_window().is_null(),
        );
        if safe {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    status(
        app,
        "installing",
        format!("Installing Oracle {} and restarting…", release.version),
    );
    if state.preview {
        crate::support::event(
            &state.root,
            "update-install",
            json!({"version":release.version.to_string(),"verified":true,"waitPid":std::process::id(),"fixture":true}),
        );
        status(
            app,
            "ready",
            "Update installer verified and launched in the disposable fixture.",
        );
    } else {
        let mut installer = Command::new(&path)
            .args([
                "/UPDATE".to_owned(),
                "/S".to_owned(),
                format!("/WAITPID={}", std::process::id()),
            ])
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| "Could not start the update installer. Nothing was changed.")?;
        std::thread::sleep(Duration::from_millis(250));
        if installer
            .try_wait()
            .map_err(|_| "Could not monitor the update installer.")?
            .is_some()
        {
            return Err("The installer could not start. Oracle will keep running.".into());
        }
        let _ = directory.keep();
        app.exit(0);
    }
    Ok(())
}

pub fn fixture(state: &Desktop, scenario: &str) -> Result<(), String> {
    if !state.preview || state.updater.busy.load(Ordering::Acquire) {
        return Err("Updater fixture unavailable.".into());
    }
    if !matches!(scenario, "current" | "available" | "offline" | "corrupt") {
        return Err("Unknown update fixture.".into());
    }
    *state.updater.scenario.lock().unwrap() = scenario.to_owned();
    Ok(())
}

fn fixture_release(current: &Version, newer: bool, bytes: &[u8]) -> Value {
    let version = if newer {
        Version::new(current.major, current.minor + 1, 0)
    } else {
        current.clone()
    };
    let label = version.to_string();
    let label = label.strip_suffix(".0").unwrap_or(&label);
    let name = format!("Oracle-{label}-Setup.exe");
    json!({"draft":false,"prerelease":false,"tag_name":format!("v{version}"),"assets":[{"name":name,"state":"uploaded","size":bytes.len(),"digest":format!("sha256:{:x}",Sha256::digest(bytes)),"browser_download_url":format!("{DOWNLOAD}/v{version}/{name}")}]})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Read-only acceptance after the owner-authorized GitHub release is published"]
    fn live_github_release_download() {
        let client = http().unwrap();
        let response = client
            .get(API)
            .header("Accept", "application/vnd.github+json")
            .send()
            .unwrap();
        let doc = crate::guide::capped_json(response, 1024 * 1024).unwrap();
        let release = release(&doc, &Version::new(0, 0, 0)).unwrap().unwrap();
        assert_eq!(release.version.to_string(), env!("CARGO_PKG_VERSION"));
        let response = client
            .get(&release.url)
            .send()
            .unwrap()
            .error_for_status()
            .unwrap();
        verify_download(response, &mut std::io::sink(), &release).unwrap();
    }
    #[test]
    fn release_requires_newer_stable_version_and_exact_oracle_installer() {
        let current = Version::new(1, 8, 0);
        let bytes = b"MZtest";
        assert!(release(&fixture_release(&current, false, bytes), &current)
            .unwrap()
            .is_none());
        let good = fixture_release(&current, true, bytes);
        assert_eq!(
            release(&good, &current).unwrap().unwrap().version,
            Version::new(1, 9, 0)
        );
        for (pointer, value) in [
            ("/draft", json!(true)),
            ("/prerelease", json!(true)),
            ("/tag_name", json!("v1.9.0-beta.1")),
            (
                "/assets/0/browser_download_url",
                json!("https://example.com/evil.exe"),
            ),
            ("/assets/0/digest", json!(null)),
            ("/assets/0/size", json!(MAX_SETUP + 1)),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(release(&bad, &current).is_err(), "{pointer}");
        }
        assert!(release(&good, &Version::new(2, 0, 0)).unwrap().is_none());
    }
    #[test]
    fn verified_download_rejects_truncated_corrupt_oversized_and_non_executable_data() {
        let bytes = b"MZverified fixture";
        let current = Version::new(1, 8, 0);
        let r = release(&fixture_release(&current, true, bytes), &current)
            .unwrap()
            .unwrap();
        for data in [
            bytes.as_slice(),
            b"MZtruncated",
            b"MZverified fixturX",
            b"MZverified fixture extra",
            b"not an executable",
        ] {
            let mut file = tempfile::tempfile().unwrap();
            assert_eq!(verify_download(data, &mut file, &r).is_ok(), data == bytes);
        }
        struct OneByte<'a>(&'a [u8]);
        impl Read for OneByte<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                self.0.read(&mut buffer[..1])
            }
        }
        assert!(verify_download(OneByte(bytes), &mut std::io::sink(), &r).is_ok());
    }

    #[test]
    fn installer_outcome_is_shown_once_without_changing_preferences() {
        let root = tempfile::tempdir().unwrap();
        let result = root.path().join("update-result.json");
        fs::write(&result, br#"{"ok":false}"#).unwrap();
        assert_eq!(initial(root.path())["status"], "error");
        assert!(!result.exists());
        fs::write(&result, br#"{"ok":true}"#).unwrap();
        assert_eq!(initial(root.path())["status"], "current");
        assert_eq!(initial(root.path())["status"], "idle");
    }
    #[test]
    fn updates_wait_for_matches_and_capture_and_target_the_standard_install_only() {
        let view = json!({"guideTarget":null,"phase":"Offline","guideCapture":false});
        assert!(safe_to_install(&view, false));
        assert!(!safe_to_install(&view, true));
        for (key, value) in [
            ("guideTarget", json!({"id":1})),
            ("phase", json!("ChampSelect")),
            ("phase", json!("InProgress")),
            ("guideCapture", json!(true)),
        ] {
            let mut busy = view.clone();
            busy[key] = value;
            assert!(!safe_to_install(&busy, false));
        }
        let root = Path::new(r"C:\Users\fixture\AppData\Local");
        assert!(installed(
            Path::new(r"C:\Users\fixture\AppData\Local\oracle\bin\oracle.exe"),
            root
        ));
        assert!(installed(
            Path::new("c:/users/FIXTURE/appdata/local/ORACLE/bin/Oracle.EXE"),
            root
        ));
        assert!(installed(&root.join("oracle/bin/oracle.exe"), root));
        assert!(!installed(Path::new(r"C:\portable\Oracle.exe"), root));
        assert!(!installed(&root.join("oracle-copy/bin/oracle.exe"), root));
    }

    #[test]
    fn installed_detection_matches_real_backslash_and_verbatim_paths() {
        let root = tempfile::tempdir().unwrap();
        let bin = root.path().join("oracle").join("bin");
        fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("oracle.exe");
        fs::write(&exe, b"fixture").unwrap();
        let native = PathBuf::from(exe.to_string_lossy().replace('/', "\\"));
        assert!(installed(&native, root.path()));
        assert!(installed(&exe.canonicalize().unwrap(), root.path()));
    }
    #[test]
    fn update_redirects_stay_on_trusted_https_hosts() {
        for (url, ok) in [
            ("https://github.com/frilo369/Oracle", true),
            ("https://release-assets.githubusercontent.com/file", true),
            ("http://github.com/a", false),
            ("https://github.com.evil.test/a", false),
            ("https://user@github.com/a", false),
            ("https://github.com:444/a", false),
        ] {
            assert_eq!(trusted_url(&url.parse().unwrap()), ok, "{url}");
        }
    }
}
