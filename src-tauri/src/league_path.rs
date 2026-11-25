use std::{fs, path::PathBuf};
pub(crate) fn folder() -> PathBuf {
    folders_for(&crate::procs::list().unwrap_or_default())
        .into_iter()
        .find(|path| path.is_dir())
        .unwrap_or_else(|| r"C:\Riot Games\League of Legends".into())
}
pub(crate) fn is_client(name: &str) -> bool {
    matches!(name, "leagueclient.exe" | "leagueclientux.exe")
}
pub(crate) fn folders_for(processes: &[crate::procs::ProcessEntry]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(text) = fs::read_to_string(
        r"C:\ProgramData\Riot Games\Metadata\league_of_legends.live\league_of_legends.live.product_settings.yaml",
    ) {
        if let Some(path) = text.lines().find_map(|line| {
            line.trim()
                .strip_prefix("product_install_full_path:")
                .map(|p| p.trim().trim_matches(['\'', '"']))
        }) {
            out.push(path.into());
        }
    }
    for row in processes.iter().filter(|p| is_client(&p.name)) {
        if let Some(image) = crate::switcher::image_path(row.pid) {
            if let Some(folder) = image.parent() {
                out.push(folder.into());
                if let Some(parent) = folder.parent() {
                    out.push(parent.into());
                }
            }
        }
    }
    out.push(r"C:\Riot Games\League of Legends".into());
    out
}
