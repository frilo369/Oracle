pub(crate) fn guide_appearance(settings: &serde_json::Value) -> (&str, &str) {
    (
        settings["assetColour"]
            .as_str()
            .filter(|s| crate::settings::valid("assetColour", &(*s).into()))
            .unwrap_or("#ae1236"),
        "umbra",
    )
}
pub(crate) fn guide_script(settings: &serde_json::Value) -> String {
    let (colour, theme) = guide_appearance(settings);
    format!(
        r#"(()=>{{const c={};const t={};try{{localStorage.setItem('umbra-guide-appearance',JSON.stringify({{t,c}}))}}catch{{}}window.UmbraAppearance?.apply(t,c);const mix=a=>'#'+c.slice(1).match(/../g).map(x=>Math.round(parseInt(x,16)*(1-a)+255*a).toString(16).padStart(2,'0')).join('');const p=document.documentElement;p.dataset.theme=t;p.style.setProperty('--accent',c);p.style.setProperty('--silver',c.toLowerCase()==='#ae1236'?'#cabfb1':mix(.60));p.style.setProperty('--shine',c.toLowerCase()==='#ae1236'?'#fff3e7':mix(.92));}})()"#,
        serde_json::json!(colour),
        serde_json::json!(theme)
    )
}
pub fn current_icon(app: &tauri::AppHandle) -> Option<tauri::image::Image<'_>> {
    app.default_window_icon().cloned()
}
