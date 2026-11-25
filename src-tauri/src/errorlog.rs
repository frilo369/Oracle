pub fn note(root: &std::path::Path, context: &str, message: &str) {
    // Only caller-supplied non-credential messages. One bounded file, no request URLs/tokens.
    let _ = std::fs::write(root.join("last-error.txt"), format!("{context}: {message}"));
}
