pub fn event(root: &std::path::Path, kind: &str, value: serde_json::Value) {
    if root.join("fixture-mode").exists() {
        use std::io::Write;
        if let Ok(mut log) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("guide-events.jsonl"))
        {
            let at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            let _ = writeln!(
                log,
                "{}",
                serde_json::json!({"at":at,"event":kind,"value":value})
            );
        }
        let _ = crate::settings::save(&root.join(format!("{kind}.json")), &value);
    }
}
