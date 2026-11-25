// Owner-requested parity with Umbra's opening/focus path. Replaces only the three native modules.
// Use deliberately: preserve Oracle's UI/parser edits separately. Run cargo fmt afterward.
import {readFileSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {join} from 'node:path';
const root=fileURLToPath(new URL('../',import.meta.url));
const reference=process.argv.slice(2).find(arg=>arg!=='--check')||String.raw`C:\Users\fryil\Documents\Rust-Friloverse-Skins`;
const pending=[];
function replaceOnce(source,find,replacement){if(source.split(find).length!==2)throw Error('Reference changed near '+find.slice(0,75)+'; review the Oracle adapter.');return source.replace(find,replacement);}
for(const file of ['guide_keys.rs','switcher.rs','match_guide.rs']) {
  let source=readFileSync(join(reference,'src-tauri/src',file),'utf8').replaceAll('\r\n','\n');
  if(file==='switcher.rs'){
    source=replaceOnce(source,'fn game_window() -> HWND {', `pub(crate) fn game_window() -> HWND {
    // Disposable acceptance only: never confuse a stand-in with the player's real game.
    if std::env::args().any(|a|a=="--fixture") {
        let title:Vec<_>="Oracle test game".encode_utf16().chain(Some(0)).collect();
        let fixture=unsafe{windows_sys::Win32::UI::WindowsAndMessaging::FindWindowW(std::ptr::null(),title.as_ptr())};
        if !fixture.is_null() && image_path(process(fixture)).is_some_and(|p|p.file_name().is_some_and(|n|n.eq_ignore_ascii_case("game-fixture.exe"))){return fixture;}
    }`);
    source=replaceOnce(source,'        *origin = Some(seen);','        *origin = Some(seen);\n        trace_origin("guide-origin",serde_json::json!({"handle":seen.0,"pid":seen.1}));');
    // A return retry runs on a new Rust worker, unlike the reference's watcher (which already
    // owns a helper window). AttachThreadInput needs both threads to have message queues.
    source=replaceOnce(source,'        let other = GetWindowThreadProcessId(current, std::ptr::null_mut());',`        let mut queue_message=windows_sys::Win32::UI::WindowsAndMessaging::MSG::default();
        windows_sys::Win32::UI::WindowsAndMessaging::PeekMessageW(&mut queue_message,std::ptr::null_mut(),0,0,windows_sys::Win32::UI::WindowsAndMessaging::PM_NOREMOVE);
        let other = GetWindowThreadProcessId(current, std::ptr::null_mut());`);
    source=replaceOnce(source,'            SMTO_BLOCK | SMTO_ABORTIFHUNG,\n            50,\n            &mut ignored,\n        ) == 0\n        {\n            return false;', '            SMTO_BLOCK | SMTO_ABORTIFHUNG,\n            50,\n            &mut ignored,\n        ) == 0\n        {\n            trace_origin("guide-focus",serde_json::json!({"stage":"unresponsive","error":windows_sys::Win32::Foundation::GetLastError()}));\n            return false;');
    source=replaceOnce(source,'            if AttachThreadInput(own, other, 1) == 0 {\n                return false;', '            if AttachThreadInput(own, other, 1) == 0 {\n                trace_origin("guide-focus",serde_json::json!({"stage":"attach","error":windows_sys::Win32::Foundation::GetLastError(),"own":own,"other":other}));\n                return false;');
    source=replaceOnce(source,'            SetForegroundWindow(hwnd);\n            AttachThreadInput(own, other, 0);',`            let target=GetWindowThreadProcessId(hwnd,std::ptr::null_mut());
            let linked=target!=0 && target!=own && target!=other && AttachThreadInput(own,target,1)!=0;
            SetForegroundWindow(hwnd);
            if linked {AttachThreadInput(own,target,0);}
            AttachThreadInput(own, other, 0);`);
    source=replaceOnce(source,'        } else {\n            SetForegroundWindow(hwnd);\n        }',`        } else {
            let target=GetWindowThreadProcessId(hwnd,std::ptr::null_mut());
            let linked=target!=0 && target!=own && AttachThreadInput(own,target,1)!=0;
            SetForegroundWindow(hwnd);
            if linked {AttachThreadInput(own,target,0);}
        }`);
    source=replaceOnce(source,'        let own = GetCurrentThreadId();',`        let own = GetCurrentThreadId();
        if GetWindowThreadProcessId(hwnd,std::ptr::null_mut())!=own && SendMessageTimeoutW(hwnd,WM_NULL,0,0,SMTO_BLOCK|SMTO_ABORTIFHUNG,50,&mut ignored)==0 { return false; }`);
    source=replaceOnce(source,'        GetForegroundWindow() == hwnd','        let focused=GetForegroundWindow()==hwnd;\n        trace_origin("guide-focus",serde_json::json!({"stage":"final","focused":focused,"own":own,"other":other,"front":GetForegroundWindow() as usize,"target":hwnd as usize}));\n        focused');
    source=replaceOnce(source,'                        *previous = Some((w.handle, process(hwnd)));','                        *previous = Some((w.handle, process(hwnd)));\n                        trace_origin("guide-previous",serde_json::json!({"handle":w.handle,"pid":process(hwnd)}));');
    // Oracle's independent WebView can lack foreground permission on close. Reuse the
    // reference's bounded focus/input-attachment routine, including its delayed game restore.
    source=replaceOnce(source,'        SetForegroundWindow(hwnd) != 0',`        let result=focus(hwnd);
        trace_origin("guide-return",serde_json::json!({"handle":handle,"pid":pid,"front":GetForegroundWindow() as usize,"frontPid":process(GetForegroundWindow()),"game":game_window() as usize,"result":result}));
        if !result {
            std::thread::spawn(move||{
                std::thread::sleep(Duration::from_millis(80));
                let hwnd=handle as HWND;
                if IsWindow(hwnd)!=0 && process(hwnd)==pid && GetForegroundWindow()!=hwnd {
                    let focused=focus(hwnd);
                    trace_origin("guide-return-retry",serde_json::json!({"focused":focused}));
                }
            });
        }
        true`);
    source=replaceOnce(source,'!matches!(w.kind, Kind::Own | Kind::Switcher)','!matches!(w.kind, Kind::Own | Kind::Guide | Kind::Switcher)');
    source+=`\nfn trace_origin(kind:&str,value:serde_json::Value){if std::env::args().any(|a|a=="--fixture"){if let Some(root)=std::env::var_os("ORACLE_DATA_DIR"){crate::support::event(&std::path::PathBuf::from(root),kind,value);}}}\n`;
  }
  if(file==='match_guide.rs'){
    source=source.replaceAll('Umbra','Oracle');
    const marker='    let label = label_for(&mode);\n    let existing = app.get_webview_window(label);';
    if(!source.includes(marker))throw Error('Reference open() changed: review the manual-guide adapter.');
    source=replaceOnce(source,marker,`    open_target(app,id,&mode,&champions)
}
pub fn manual_open(app:&AppHandle,id:u64,mode:&str)->Result<(),String> {
    let _guard=OPENING.lock().unwrap_or_else(|p|p.into_inner());
    let champions=app.state::<Desktop>().view.lock().map_err(|_|"State unavailable")?["champions"].clone();
    open_target(app,id,mode,&champions)
}
fn open_target(app:&AppHandle,id:u64,mode:&str,champions:&serde_json::Value)->Result<(),String> {
    let label=label_for(mode);
    let existing=app.get_webview_window(label);`);
    source=replaceOnce(source,'        if result.is_ok() {\n            *registered = Some(binding.clone());\n        }',`        if result.is_ok() { *registered=Some(binding.clone()); }
        if let Ok(mut view)=state.view.lock(){ view["shortcutStatus"]=json!(if result.is_ok(){"ready"}else{"conflict"}); }`);
    // A fixture-only trace proves WM_SETHOTKEY activation is exercised, not just key polling.
    source=replaceOnce(source,'    if unsafe { GetForegroundWindow() } == hwnd {\n        if !shortcut_enabled(app) {',`    if unsafe { GetForegroundWindow() } == hwnd {
        crate::support::event(&app.state::<Desktop>().root,"guide-activation",json!({"foreground":true}));
        if !shortcut_enabled(app) {`);
    source=replaceOnce(source,'    } else if matches!(current, GuideShortcut::NoMatch) {','    } else if matches!(current, GuideShortcut::NoMatch) {\n        if let Ok(mut view)=state.view.lock(){view["shortcutStatus"]=json!("waiting");}');
  }
  pending.push([join(root,'src-tauri/src',file),source]);
}
if(!process.argv.includes('--check'))for(const [path,source] of pending)writeFileSync(path,source);
console.log(process.argv.includes('--check')?'Reference adapters validated without writing files.':'Native guide modules synchronized with the reference.');
