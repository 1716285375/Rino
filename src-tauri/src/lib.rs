mod domain;
mod engine;
mod keys;
mod monitor;
mod providers;
mod secrets;
mod storage;
use crate::domain::{AppSettings, CandidateSnapshot, HistoryRow, Metrics, ModelRecord, Rect, Translation, VocabRow};
use crate::engine::TranslationEngine;
use crate::storage::LocalStore;
use parking_lot::{Mutex, RwLock};
use std::{path::PathBuf, sync::{atomic::{AtomicU64, Ordering}, Arc}, thread, time::{Duration,Instant}};
use tauri::{Emitter, Manager, PhysicalPosition, State, WindowEvent};

/// Candidate key of the frame currently on screen (read by the key hook).
static HUD_KEY:Mutex<String>=Mutex::new(String::new());
/// Monitor thread handle, so a keystroke or click can cut the poll wait short.
static LOOP_THREAD:std::sync::OnceLock<thread::Thread>=std::sync::OnceLock::new();

/// Wake the monitor loop now (called from the key / mouse hook).
pub(crate) fn wake_monitor() { if let Some(t)=LOOP_THREAD.get() { t.unpark(); } }

fn park(d:Duration) { thread::park_timeout(d); }

/// Candidate bar rectangle of the frame on screen (physical pixels).
static LAST_BAR:Mutex<Option<Rect>>=Mutex::new(None);
/// While this deadline is in the future the loop polls every few ms.
static URGENT_UNTIL:Mutex<Option<Instant>>=Mutex::new(None);
/// Letters still in the composition (from TextEditPattern), if the app gives it.
static COMPOSITION_LETTERS:Mutex<Option<usize>>=Mutex::new(None);

/// Composition length, so the key hook can predict "backspace empties it".
pub(crate) fn composition_letters()->Option<usize> { *COMPOSITION_LETTERS.lock() }

/// Bar rect, for the mouse hook to tell "click on a candidate" from any click.
pub(crate) fn hud_bar_rect()->Option<Rect> { *LAST_BAR.lock() }

/// Probe fast for the next few milliseconds (a bar change is under way).
pub(crate) fn urgent_probe(ms:u64) { *URGENT_UNTIL.lock()=Some(Instant::now()+Duration::from_millis(ms)); }
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
struct RuntimeState {
    settings: RwLock<AppSettings>,
    storage: Arc<LocalStore>,
    engine: Arc<TranslationEngine>,
    snapshot: RwLock<Option<CandidateSnapshot>>,
    latest_translation: RwLock<Option<Translation>>,
    metrics: Mutex<Metrics>,
    generation: AtomicU64,
    // `tauri::async_runtime::spawn` hands back its own JoinHandle; the tokio
    // one cannot be stored here (they are distinct types).
    pending: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    // Last history row, so holding the same candidate does not spam the table.
    last_history: Mutex<Option<(String,Instant)>>,
}
#[tauri::command]
fn get_settings(state: State<'_, Arc<RuntimeState>>) -> AppSettings { state.settings.read().clone() }
#[tauri::command]
fn save_settings(settings: AppSettings, app: tauri::AppHandle, state: State<'_, Arc<RuntimeState>>) -> Result<(), String> {
    if settings.poll_active_ms < 30 || settings.poll_active_ms > 2000 || settings.poll_idle_ms < 80 || settings.poll_idle_ms > 5000 {
        return Err("轮询间隔须在允许范围内".into());
    }
    if !(12..=28).contains(&settings.font_size) || settings.max_options > 12 { return Err("显示参数超出范围".into()); }
    if !settings.providers.iter().any(|p| p.id == settings.provider_id && p.enabled)
        && settings.provider_id != "auto" && settings.provider_id != "offline" { return Err("指定的 Provider 不可用".into()); }
    state.storage.save_settings(&settings).map_err(|e| e.to_string())?;
    *state.settings.write() = settings.clone();
    let _=app.emit_to("main","rino:settings",&settings);
    let _=app.emit_to("hud","rino:settings",&settings);
    state.engine.reset();
    state.generation.fetch_add(1, Ordering::SeqCst);
    if let Some(task)=state.pending.lock().take(){task.abort();}
    Ok(())
}
#[tauri::command]
fn current_candidate(state: State<'_, Arc<RuntimeState>>) -> Option<CandidateSnapshot> { state.snapshot.read().clone() }
#[tauri::command]
fn runtime_metrics(state: State<'_, Arc<RuntimeState>>) -> Metrics { state.metrics.lock().clone() }
#[tauri::command]
fn provider_stats(state: State<'_,Arc<RuntimeState>>) -> serde_json::Value {
    let mut rows=Vec::new();
    for (id,s) in state.engine.stats() { rows.push(serde_json::json!({"id":id,"ok":s.ok,"fail":s.fail,"last_ms":s.last_ms,"ema_ms":s.ema_ms})); }
    serde_json::json!({"providers":rows})
}
// ---- window / lifetime ---------------------------------------------------
#[tauri::command]
fn show_main(app:tauri::AppHandle) {
    if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}
}
#[tauri::command]
fn open_settings(app:tauri::AppHandle) {
    if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}
    let _=app.emit_to("main","rino:navigate","settings");
}
#[tauri::command]
fn confirm_close(action:String, remember:bool, app:tauri::AppHandle, state:State<'_,Arc<RuntimeState>>) -> Result<(),String> {
    if remember {
        let snapshot={let mut cfg=state.settings.write();
            cfg.close_prompt=false;
            cfg.close_action=if action=="quit"{"quit".into()}else{"tray".into()};
            cfg.clone()};
        state.storage.save_settings(&snapshot).map_err(|e|e.to_string())?;
        let _=app.emit_to("main","rino:settings",&snapshot);
        let _=app.emit_to("hud","rino:settings",&snapshot);
    }
    if action=="quit" {app.exit(0);}
    else if let Some(w)=app.get_webview_window("main"){let _=w.hide();}
    Ok(())
}
// ---- input history -------------------------------------------------------
#[tauri::command]
fn list_history(limit:Option<u32>, state:State<'_,Arc<RuntimeState>>) -> Result<Vec<HistoryRow>,String> {
    state.storage.list_history(limit.unwrap_or(300)).map_err(|e|e.to_string())
}
#[tauri::command]
fn delete_history(id:i64, state:State<'_,Arc<RuntimeState>>) -> Result<(),String> {
    state.storage.delete_history(id).map_err(|e|e.to_string())
}
#[tauri::command]
fn clear_history(state:State<'_,Arc<RuntimeState>>) -> Result<(),String> {
    state.storage.clear_history().map_err(|e|e.to_string())
}#[tauri::command]
fn set_api_key(profile_id:String, key:String) -> Result<(),String> { secrets::set_key(&profile_id, &key) }
#[tauri::command]
fn has_api_key(profile_id:String) -> bool { secrets::has_key(&profile_id) }
#[tauri::command]
async fn translate_text(text:String, state:State<'_,Arc<RuntimeState>>) -> Result<Translation,String> {
    if text.chars().count() > 500 {return Err("文本不能超过 500 字符".into());}
    let config=state.settings.read().clone();
    let (t,_) = state.engine.translate(&state.storage, &config, &text).await.map_err(|e|e.to_string())?;
    Ok(t)
}
#[tauri::command]
async fn list_ollama_models(state:State<'_, Arc<RuntimeState>>) -> Result<Vec<ModelRecord>,String> {
    let settings=state.settings.read().clone();
    let host=settings.providers.iter().find(|p|p.id=="ollama").map(|p|p.base_url.as_str()).unwrap_or("http://127.0.0.1:11434");
    state.engine.provider.ollama_models(host).await.map_err(|e|e.to_string())
}
#[tauri::command]
async fn warmup_ollama_model(model:String, state:State<'_,Arc<RuntimeState>>) -> Result<(),String> {
    if model.len()>160 || model.contains('\n') || model.contains('\r') {return Err("无效模型 ID".into());}
    let settings=state.settings.read().clone();
    let host=settings.providers.iter().find(|p|p.id=="ollama").map(|p|p.base_url.as_str()).unwrap_or("http://127.0.0.1:11434");
    state.engine.provider.ollama_load(host, &model).await.map_err(|e|e.to_string())
}
#[tauri::command]
fn save_vocab(text:String,translation:String,target:String,state:State<'_,Arc<RuntimeState>>) -> Result<(),String>{
    state.storage.add_vocab(&text,&target,&translation).map_err(|e|e.to_string())
}
#[tauri::command]
fn list_vocab(due_only:bool,state:State<'_,Arc<RuntimeState>>) -> Result<Vec<VocabRow>,String>{
    state.storage.list_vocab(due_only).map_err(|e|e.to_string())
}
#[tauri::command]
fn review_vocab(text:String,target:String,quality:i32,state:State<'_,Arc<RuntimeState>>) -> Result<(),String>{
    state.storage.review(&text,&target,quality).map_err(|e|e.to_string())
}
#[tauri::command]
fn remove_vocab(text:String,target:String,state:State<'_,Arc<RuntimeState>>) -> Result<(),String>{
    state.storage.delete_vocab(&text,&target).map_err(|e|e.to_string())
}
#[tauri::command]
fn export_vocab(state:State<'_,Arc<RuntimeState>>) -> Result<String,String>{
    state.storage.export_csv().map_err(|e|e.to_string())
}
// Safe diagnostics: never expose raw input or UIA text in diagnostic logs.
#[tauri::command]
fn diagnostic_report(state:State<'_,Arc<RuntimeState>>) -> String {
    let m=state.metrics.lock().clone();
    let s=state.settings.read();
    format!("RINO 0.1\nWindows IME monitor\nStatus: {}\nSnapshots: {}\nTranslations: {}\nCache hits: {}\nErrors: {}\nPolling: {} ms (active) / {} ms (idle)\nProvider: {}\nLast translation: {} ms\nCandidate text and credentials are intentionally excluded.\n",m.last_reader_status,m.snapshots,m.translations,m.cache_hits,m.errors,s.poll_active_ms,s.poll_idle_ms,s.provider_id,m.last_translation_ms)
}
// Diagnostic trace. Bounds and statuses only: never candidate text.
static TRACE_START:std::sync::OnceLock<std::time::Instant>=std::sync::OnceLock::new();
fn trace(msg:&str){
    use std::io::Write;
    let start=TRACE_START.get_or_init(std::time::Instant::now);
    let path=std::env::temp_dir().join("rino_trace.log");
    if let Ok(m)=std::fs::metadata(&path){if m.len()>1_000_000{let _=std::fs::remove_file(&path);}}
    if let Ok(mut f)=std::fs::OpenOptions::new().create(true).append(true).open(&path){
        let _=writeln!(f,"+{:>7}ms {}",start.elapsed().as_millis(),msg);
    }
}
// Sub-pixel jitter between UIA reads must not make the pill twitch.
static LAST_HUD_POS:Mutex<Option<(i32,i32)>>=Mutex::new(None);
// Physical size of the pill currently on screen (CSS px * scale factor).
static HUD_PILL:Mutex<(i32,i32)>=Mutex::new((96,40));
// Candidate text -> pill size it rendered at. Repeats then show pre-sized.
static HUD_SIZE_CACHE:Mutex<Option<std::collections::HashMap<String,(i32,i32)>>>=Mutex::new(None);
fn position_hud(app:&tauri::AppHandle,snapshot:&CandidateSnapshot,gap:i32) {
    let Some(hud)=app.get_webview_window("hud") else {trace("hud window missing");return};
    let pill=*HUD_PILL.lock();
    place_hud(&hud,snapshot,gap,pill.0,pill.1);
}
// The HUD window is a fixed transparent canvas; the pill inside it is sized by
// CSS and drawn at a known inset. We therefore move the window so the pill's
// top-left lands exactly under the highlighted candidate cell. Resizing a
// WebView2 window programmatically leaves the page viewport stale on this box
// (the card gets clipped), so the canvas stays fixed.
fn place_hud(hud:&tauri::WebviewWindow,snapshot:&CandidateSnapshot,gap:i32,pill_w:i32,pill_h:i32) {
    let scale=hud.scale_factor().unwrap_or(1.0);
    let pad_x=(5.0*scale).round() as i32;
    let pad_y=(4.0*scale).round() as i32;
    // Anchor under the highlighted cell, not the whole panel: the pill belongs
    // beneath the character it explains.
    let cell=if snapshot.bounds.width()>0&&snapshot.bounds.height()>0 {snapshot.bounds} else {snapshot.window_bounds};
    let cx=(cell.left+cell.right)/2; let cy=(cell.top+cell.bottom)/2;
    let monitor_bounds=hud.available_monitors().ok().and_then(|monitors|monitors.into_iter().find(|m| {
        let p=m.position();let s=m.size();
        cx>=p.x && cx<p.x+s.width as i32 && cy>=p.y && cy<p.y+s.height as i32
    }).map(|m| {let p=m.position();let s=m.size();Rect{left:p.x,top:p.y,right:p.x+s.width as i32,bottom:p.y+s.height as i32}}));
    let (pill_x,pill_y)=match monitor_bounds {
        Some(monitor)=>monitor::hud_placement(cell,monitor,pill_w,pill_h,gap),
        // No owning monitor: never clamp onto the primary display.
        None=>(cell.left,cell.bottom+gap),
    };
    let (x,y)=(pill_x-pad_x,pill_y-pad_y);
    let mut last=LAST_HUD_POS.lock();
    let jitter=match *last {Some((lx,ly))=>(x-lx).abs()<3&&(y-ly).abs()<2,None=>false};
    if !jitter {
        if let Err(e)=hud.set_position(PhysicalPosition::new(x,y)) {trace(&format!("set_position failed: {e}"));}
        *last=Some((x,y));
    }
    drop(last);
    if let Err(e)=hud.show() {trace(&format!("show failed: {e}"));}
    *LAST_BAR.lock()=Some(snapshot.window_bounds);
    keys::set_visible(true);
    let visible=hud.is_visible().unwrap_or(false);
    trace(&format!("pos pill=({pill_x},{pill_y}) size={pill_w}x{pill_h} win=({x},{y}) cell=({},{},{},{}) monitor={} visible={visible}",
        cell.left,cell.top,cell.right,cell.bottom,if monitor_bounds.is_some(){"yes"}else{"none"}));
}
// The HUD webview reports the size its text needs. We keep the canvas fixed and
// only record the pill size, then re-anchor so the pill hugs the translation.
#[tauri::command]
fn hud_resize(app:tauri::AppHandle,width:f64,height:f64,chars:u32,state:State<'_,Arc<RuntimeState>>){
    let Some(hud)=app.get_webview_window("hud") else {return};
    let scale=hud.scale_factor().unwrap_or(1.0);
    let w=((width*scale).round() as i32).clamp(56,540);
    let h=((height*scale).round() as i32).clamp(30,220);
    *HUD_PILL.lock()=(w,h);
    if let Some(text)=state.snapshot.read().as_ref().map(|s|s.text.clone()) {
        let mut cache=HUD_SIZE_CACHE.lock();
        let map=cache.get_or_insert_with(std::collections::HashMap::new);
        if map.len()>128 {map.clear();}
        map.insert(text,(w,h));
    }
    trace(&format!("resize pill={w}x{h} chars={chars}"));
    let snapshot=state.snapshot.read().clone();
    let gap=state.settings.read().hud_gap;
    if let Some(snapshot)=snapshot {*LAST_BAR.lock()=Some(snapshot.window_bounds);place_hud(&hud,&snapshot,gap,w,h);}
}
fn hide_hud(app:&tauri::AppHandle) {
    keys::set_visible(false);
    *COMPOSITION_LETTERS.lock()=None;
    *LAST_BAR.lock()=None;
    *URGENT_UNTIL.lock()=None;
    trace("hide");
    if let Some(hud)=app.get_webview_window("hud") { let _=hud.hide(); }
    let _=app.emit_to("hud","rino:hide",());
    let _=app.emit_to("main","rino:hide",());
}

/// Key the pill is showing right now; the commit-key hook uses it as barrier.
pub(crate) fn current_hud_key()->String {
    HUD_KEY.lock().clone()
}

/// Hide path taken from the key hook: drop the frame, then hide the window.
pub(crate) fn hide_hud_now(app:&tauri::AppHandle) {
    if let Some(state)=app.try_state::<Arc<RuntimeState>>() { *state.snapshot.write()=None; }
    hide_hud(app);
}
fn monitor_loop(app:tauri::AppHandle, state:Arc<RuntimeState>) {
    let _=LOOP_THREAD.set(thread::current());
    // All UI Automation objects remain on this one thread (COM apartment boundary).
    let mut reader=match monitor::WindowsReader::new() {
        Ok(r)=>{trace("reader ready");r},
        Err(reason)=>{trace(&format!("reader init failed: {reason}"));state.metrics.lock().last_reader_status=reason;return;}
    };
    let mut previous_key=String::new();
    let mut hold_trace=Instant::now()-Duration::from_secs(5);
    let mut misses=0;
    let mut last_rect=Rect::default();
    let mut last_cell=Rect::default();
    loop {
        let cfg=state.settings.read().clone();
        if !cfg.enabled {
            if !previous_key.is_empty(){previous_key.clear();*state.snapshot.write()=None;hide_hud(&app);}
            park(Duration::from_millis(cfg.poll_idle_ms));continue;
        }
        // One empty poll does not prove the bar is gone: apps rebuild their
        // candidate window mid-keystroke and can miss for a single frame.
        // Re-probe once 30 ms later before calling it a dismissal.
        let sample=reader.probe();
        if let Some(sample)=&sample {
            if keys::barrier_matches(&sample.content_key()) {
                state.metrics.lock().last_reader_status=reader.status();
                park(Duration::from_millis(4));
                continue;
            }
        }
        // Distinguish "the panel repainted and we could not re-read it this
        // tick" from "the IME committed and the panel is gone". Only the second
        // case may hide the pill; the first one must simply keep the frame, or
        // the HUD blinks on every keystroke.
        // A bar that is definitely gone needs no second opinion.
        if sample.is_none() && reader.bar_gone() { misses=misses.max(2); }
        if sample.is_none() && reader.panel_alive() {
            state.metrics.lock().last_reader_status=reader.status();
            if hold_trace.elapsed()>Duration::from_millis(400) {
                hold_trace=Instant::now();
                trace(&format!("hold alive status={}",reader.status()));
            }
            thread::sleep(Duration::from_millis(cfg.poll_active_ms));
            continue;
        }
        if let Some(mut sample)=sample {
            misses=0;
            *COMPOSITION_LETTERS.lock()=reader.composition_letters();
            let key=sample.content_key();
            let changed=key!=previous_key;
            let moved=sample.window_bounds!=last_rect||sample.bounds!=last_cell;
            if changed {
                trace(&format!("present cell=({},{},{},{}) row=({},{},{},{}) n={} status={}",
                    sample.bounds.left,sample.bounds.top,sample.bounds.right,sample.bounds.bottom,
                    sample.window_bounds.left,sample.window_bounds.top,sample.window_bounds.right,sample.window_bounds.bottom,
                    sample.candidates.len(),reader.status()));
                *HUD_KEY.lock()=key.clone();
                previous_key=key;
                let generation=state.generation.fetch_add(1,Ordering::SeqCst)+1;
                if let Some(task)=state.pending.lock().take(){task.abort();}
                sample.revision=generation;
                last_rect=sample.window_bounds;
                last_cell=sample.bounds;
                *state.snapshot.write()=Some(sample.clone());
                *state.latest_translation.write()=None;
                // Show already sized when we have seen this word before; a
                // first-time word is sized by the HUD a couple of ms later.
                if let Some(size)=HUD_SIZE_CACHE.lock().as_ref().and_then(|m|m.get(&sample.text)).copied() {
                    *HUD_PILL.lock()=size;
                }
                state.metrics.lock().snapshots+=1;
                let _=app.emit_to("hud","rino:candidate",&sample);
                let _=app.emit_to("main","rino:candidate",&sample);
                position_hud(&app,&sample,cfg.hud_gap);
                // Cancel stale output through revision checks (avoid committing old inference).
                let cloned_state=Arc::clone(&state);
                let app2=app.clone();
                let text=sample.text.clone();
                // The loop below still needs cfg for its own timing; move a copy in.
                let cfg_task=cfg.clone();
                let task=tauri::async_runtime::spawn(async move {
                    let result=cloned_state.engine.translate(&cloned_state.storage,&cfg_task,&text).await;
                    if cloned_state.generation.load(Ordering::SeqCst)!=generation{return;}
                    match result {
                        Ok((translation,hit))=>{
                            let mut metrics=cloned_state.metrics.lock();metrics.translations+=1;if hit{metrics.cache_hits+=1;}metrics.last_translation_ms=translation.elapsed_ms;drop(metrics);
                            // Local-only input history (same word is not logged twice in a row).
                            let cfg=cloned_state.settings.read().clone();
                            let history_key=format!("{}|{}",translation.original,translation.target);
                            let should_log=cloned_state.last_history.lock().as_ref()
                                .map(|(k,t)| k!=&history_key || t.elapsed()>Duration::from_millis(1500)).unwrap_or(true);
                            if should_log && !translation.translated.trim().is_empty() {
                                *cloned_state.last_history.lock()=Some((history_key,Instant::now()));
                                let _=cloned_state.storage.add_history(&translation.original,&translation.translated,&cfg.source,&cfg.target,&translation.engine,translation.elapsed_ms);
                            }
                            *cloned_state.latest_translation.write()=Some(translation.clone());
                            let _=app2.emit_to("hud","rino:translation",&translation);
                            let _=app2.emit_to("main","rino:translation",&translation);
                        },
                        Err(e)=>{
                            cloned_state.metrics.lock().errors+=1;
                            // Generic UI error only. Do not record raw candidate content.
                            let _=app2.emit_to("hud","rino:translation-error",e.to_string());
                        }
                    }
                });
                *state.pending.lock()=Some(task);
            }else if moved {
                last_rect=sample.window_bounds;
                last_cell=sample.bounds;
                position_hud(&app,&sample,cfg.hud_gap);
            }
            state.metrics.lock().last_reader_status=reader.status();
            thread::sleep(Duration::from_millis(cfg.poll_active_ms));
            continue;
        }
        misses+=1;
        // Past the guard above, an empty read is a real dismissal: the pill
        // leaves in the same beat as the IME bar, no linger, no ghost frame.
        // Two ticks (50 ms) of agreement, because a single hit test can land
        // in a repaint gap and report the app instead of the bar.
        if !previous_key.is_empty() && misses<2 {
            if keys::object_hide_recent(Duration::from_millis(300)) {
                // The IME window really went away: take one quick second look
                // 18 ms later instead of waiting for two poll ticks, then trust
                // that answer. A repaint hides windows too, and only the probe
                // can tell the two cases apart.
                park(Duration::from_millis(18));
                if reader.probe().is_none() {
                    misses=2;
                } else {
                    misses=0;
                    trace("object-hide ignored: bar still readable");
                    state.metrics.lock().last_reader_status=reader.status();
                    continue;
                }
            } else {
                state.metrics.lock().last_reader_status=reader.status();
                park(Duration::from_millis(2));
                continue;
            }
        }
        if !previous_key.is_empty() {
            trace(&format!("miss #{} alive={} has_panel={} status={}",misses,reader.panel_alive(),reader.has_panel(),reader.status()));
            state.generation.fetch_add(1,Ordering::SeqCst);
            if let Some(task)=state.pending.lock().take(){task.abort();}
            previous_key.clear();
            *state.snapshot.write()=None;
            *state.latest_translation.write()=None;
            hide_hud(&app);
        }
        state.metrics.lock().last_reader_status=reader.status();
        // Stay on the fast cadence for a few polls after the bar vanished so the
        // next word appears without waiting a slow round trip.
        // While a pill is on screen we chase the bar: a commit must extinguish
        // the HUD in the same beat, so probe twice as often as the idle cadence.
        let urgent=URGENT_UNTIL.lock().map(|u|Instant::now()<u).unwrap_or(false);
        let interval=if !previous_key.is_empty() {
            if urgent {4} else {(cfg.poll_active_ms/3).clamp(10,20)}
        }
                     else if misses<4 {cfg.poll_active_ms} else {cfg.poll_idle_ms};
        park(Duration::from_millis(interval));
    }
}
pub fn run() {
    // Keep this plugin first: a second process must be rejected before any
    // window, tray icon or background monitor is created.
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app,_argv,_cwd| {
            if let Some(w)=app.get_webview_window("main"){
                let _=w.show();let _=w.unminimize();let _=w.set_focus();
            }
        }))
        .setup(|app| {
            let handle=app.handle().clone();
            let resource_dir=app.path().resource_dir().unwrap_or_else(|_|PathBuf::from(env!("CARGO_MANIFEST_DIR")));
            let resource_dir=if resource_dir.join("resources/lexicon.json").exists(){resource_dir.join("resources")}
                else {PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources")};
            let data_dir=app.path().app_local_data_dir().unwrap_or_else(|_|std::env::temp_dir().join("rino-data"));
            let store=Arc::new(LocalStore::new(&data_dir,&resource_dir)?);
            let cfg=store.load_settings();
            let state=Arc::new(RuntimeState{settings:RwLock::new(cfg),storage:store,engine:Arc::new(TranslationEngine::new()),snapshot:RwLock::new(None),latest_translation:RwLock::new(None),metrics:Mutex::new(Metrics{last_reader_status:"初始化…".into(),..Default::default()}),generation:AtomicU64::new(0),pending:Mutex::new(None),last_history:Mutex::new(None)});
            app.manage(Arc::clone(&state));
            // Keep the local model resident: a cold Ollama load costs seconds and
            // would otherwise hit the first word the user types.
            {
                let state=Arc::clone(&state);
                tauri::async_runtime::spawn(async move {
                    let cfg=state.settings.read().clone();
                    let profile=cfg.providers.iter().find(|p|p.id=="ollama"&&p.enabled&&(cfg.provider_id=="ollama"||cfg.provider_id=="auto")).cloned();
                    if let Some(p)=profile {
                        if p.model.trim().is_empty(){return;}
                        match state.engine.provider.ollama_load(&p.base_url,&p.model).await {
                            Ok(_)=>trace("ollama warmup ok"),
                            Err(e)=>trace(&format!("ollama warmup failed: {e}")),
                        }
                    }
                });
            }
            if state.settings.read().start_minimized {
                if let Some(main)=app.get_webview_window("main"){let _=main.hide();}
            }
            if let Some(hud)=app.get_webview_window("hud") {
                let _=hud.set_ignore_cursor_events(true);
            }
            keys::install(handle.clone());
            if let Some(icon)=app.default_window_icon().cloned() {
                let paused=!state.settings.read().enabled;
                let open=MenuItem::with_id(app,"open","打开",true,None::<&str>)?;
                let settings_item=MenuItem::with_id(app,"settings","设置",true,None::<&str>)?;
                let toggle=MenuItem::with_id(app,"toggle",if paused {"启动"} else {"暂停"},true,None::<&str>)?;
                let quit=MenuItem::with_id(app,"quit","退出应用",true,None::<&str>)?;
                let toggle_for_menu=toggle.clone();
                let menu=Menu::with_items(app,&[&open,&settings_item,&toggle,&quit])?;
                TrayIconBuilder::new().icon(icon).menu(&menu).tooltip("RINO")
                    .on_menu_event(move |app,event|{
                        match event.id().as_ref() {
                            "open"=>{if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}},
                            "settings"=>{
                                if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}
                                let _=app.emit_to("main","rino:navigate","settings");
                            }
                            "toggle"=>{
                                let s=app.state::<Arc<RuntimeState>>();
                                let settings={let mut cfg=s.settings.write();cfg.enabled=!cfg.enabled;cfg.clone()};
                                let _=s.storage.save_settings(&settings);
                                let _=app.emit_to("main","rino:settings",&settings);
                                let _=toggle_for_menu.set_text(if settings.enabled {"暂停"} else {"启动"});
                                crate::trace(if settings.enabled {"tray: monitor resumed"} else {"tray: monitor paused"});
                            }
                            "quit"=>app.exit(0),_=>{}
                        }
                    })
                    .on_tray_icon_event(|tray,event|{
                        // Double click on the tray icon is the fastest way back.
                        if let tauri::tray::TrayIconEvent::DoubleClick{..}=event {
                            let app=tray.app_handle();
                            if let Some(w)=app.get_webview_window("main"){let _=w.show();let _=w.unminimize();let _=w.set_focus();}
                        }
                    }).build(app)?;
                // Show the window once the front end is ready; keep a 6 s safety
                // net so a failed dev server can never leave the app invisible.
                {
                    let handle=handle.clone();
                    thread::spawn(move||{
                        thread::sleep(Duration::from_secs(6));
                        let start_min=handle.state::<Arc<RuntimeState>>().settings.read().start_minimized;
                        if !start_min {if let Some(w)=handle.get_webview_window("main"){if !w.is_visible().unwrap_or(false){let _=w.show();}}}
                    });
                }
            }
                // Closing the main window either asks once, hides to tray, or quits.
                if let Some(main)=app.get_webview_window("main") {
                    let handle=app.handle().clone();
                    main.on_window_event(move|event|{
                        if let WindowEvent::CloseRequested{api,..}=event {
                            let state=handle.state::<Arc<RuntimeState>>();
                            let (prompt,action)={let cfg=state.settings.read();(cfg.close_prompt,cfg.close_action.clone())};
                            api.prevent_close();
                            if prompt {
                                let _=handle.emit_to("main","rino:close-request",());
                                if let Some(w)=handle.get_webview_window("main"){let _=w.set_focus();}
                            } else if action=="quit" {handle.exit(0);}
                            else if let Some(w)=handle.get_webview_window("main"){let _=w.hide();}
                        }
                    });
                }
            // RINO_HUD_SELFTEST=1 paints one canned pill so the HUD window can be
            // verified without an IME session (diagnostics only).
            if std::env::var("RINO_HUD_SELFTEST").ok().as_deref()==Some("1") {
                let app=app.handle().clone();
                thread::spawn(move||{
                    thread::sleep(Duration::from_secs(2));
                    for _ in 0..3 {
                    let cell=match std::env::var("RINO_HUD_SELFTEST_CELL") {
                    Ok(raw)=>{let parts=raw.split(',').filter_map(|p|p.trim().parse::<i32>().ok()).collect::<Vec<_>>();
                        if parts.len()==4 {Rect{left:parts[0],top:parts[1],right:parts[2],bottom:parts[3]}}
                        else {Rect{left:340,top:220,right:430,bottom:256}}},
                    Err(_)=>Rect{left:340,top:220,right:430,bottom:256},
                };
                    let snapshot=CandidateSnapshot{text:"你好".into(),selected_index:0,
                        candidates:vec![crate::domain::Candidate{text:"你好".into(),bounds:cell,selected:true}],
                        bounds:cell,window_bounds:Rect{left:340,top:220,right:1200,bottom:256},
                        process_id:0,source:"self-test".into(),revision:1};
                    let _=app.emit_to("hud","rino:candidate",&snapshot);
                    let _=app.emit_to("main","rino:candidate",&snapshot);
                    position_hud(&app,&snapshot,9);
                    thread::sleep(Duration::from_millis(250));
                    let translation=Translation{original:"你好".into(),translated:"Hello".into(),
                        target:"en".into(),engine:"self-test".into(),alternatives:vec![],elapsed_ms:0};
                    let _=app.emit_to("hud","rino:translation",&translation);
                    trace("self-test presented");
                    thread::sleep(Duration::from_secs(3));
                    }
                });
            }
            thread::Builder::new().name("rino-ime-uia".into()).spawn(move||monitor_loop(handle,state))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_settings,save_settings,current_candidate,runtime_metrics,provider_stats,show_main,open_settings,confirm_close,list_history,delete_history,clear_history,set_api_key,has_api_key,translate_text,list_ollama_models,warmup_ollama_model,save_vocab,list_vocab,review_vocab,remove_vocab,export_vocab,diagnostic_report,hud_resize])
        .run(tauri::generate_context!()).expect("failed to run RINO");
}
