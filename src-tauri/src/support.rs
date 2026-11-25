pub fn event(root: &std::path::Path, kind: &str, value: serde_json::Value) {
    if root.join("fixture-mode").exists() {
        let _ = crate::settings::save(&root.join(format!("{kind}.json")), &value);
    }
}
