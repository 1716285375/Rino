//! Commit detection without waiting for UI Automation.
//!
//! Three ways the pill is taken down, all faster than a poll:
//!   * a key that commits the composition (space, enter, digits, punctuation);
//!   * a left click inside the candidate bar (that click *is* the selection);
//!   * any foreground change (the composition cannot survive it).
//! Everything else (backspace, more letters, arrows, clicks outside the bar)
//! only wakes the monitor, which re-reads the bar immediately.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

const WH_KEYBOARD_LL: i32 = 13;
const WH_MOUSE_LL: i32 = 14;
const WM_KEYDOWN: usize = 0x0100;
const WM_SYSKEYDOWN: usize = 0x0104;
const WM_LBUTTONDOWN: usize = 0x0201;
const WM_RBUTTONDOWN: usize = 0x0204;
const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_HIDE: u32 = 0x8003;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;

const VK_CONTROL: i32 = 0x11;
const VK_MENU: i32 = 0x12;
const VK_SHIFT: i32 = 0x10;
const VK_TAB: u32 = 0x09;
const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const KEYEVENTF_UNICODE: u32 = 0x0004;

#[repr(C)]
struct KbdLlHookStruct {
    vk_code: u32,
    scan_code: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

#[repr(C)]
struct MsllHookStruct {
    pt_x: i32,
    pt_y: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

#[repr(C)]
struct WRect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct Msg {
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
    private: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct KeybdInput {
    vk: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct MouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct HardwareInput {
    msg: u32,
    param_l: u16,
    param_h: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
union InputUnion {
    keyboard: KeybdInput,
    mouse: MouseInput,
    hardware: HardwareInput,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Input {
    kind: u32,
    data: InputUnion,
}

extern "system" {
    fn SetWindowsHookExW(
        id: i32,
        proc: Option<unsafe extern "system" fn(i32, usize, isize) -> isize>,
        module: isize,
        thread: u32,
    ) -> isize;
    fn CallNextHookEx(hook: isize, code: i32, wparam: usize, lparam: isize) -> isize;
    fn GetMessageW(msg: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
    fn GetAsyncKeyState(vk: i32) -> i16;
    fn GetWindowRect(hwnd: isize, rect: *mut WRect) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    fn GetForegroundWindow() -> isize;
    fn SendInput(inputs: u32, events: *const Input, size: i32) -> u32;
    fn SetWinEventHook(
        min: u32,
        max: u32,
        module: isize,
        proc: Option<unsafe extern "system" fn(isize, u32, isize, i32, i32, u32, u32)>,
        pid: u32,
        tid: u32,
        flags: u32,
    ) -> isize;
}

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static HUD_VISIBLE: AtomicBool = AtomicBool::new(false);
static BARRIER: Mutex<Option<(String, Instant, u64)>> = Mutex::new(None);

/// Install the hooks on their own message-pumping thread.
pub fn install(app: tauri::AppHandle) {
    let _ = APP.set(app);
    let _ = std::thread::Builder::new()
        .name("rino-keys".into())
        .spawn(|| unsafe {
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(callback), 0, 0);
            if hook == 0 {
                crate::trace("key hook failed");
                return;
            }
            let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_callback), 0, 0);
            // The IME candidate window going away is the ground truth for
            // "the bar is gone": it arrives the moment Windows takes the
            // window down, whatever the app does about composition text.
            let ime_hide = SetWinEventHook(
                EVENT_OBJECT_DESTROY,
                EVENT_OBJECT_HIDE,
                0,
                Some(object_callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let foreground = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                0,
                Some(foreground_callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            crate::trace(&format!(
                "hooks ready keyboard=1 mouse={} foreground={} object={}",
                if mouse_hook == 0 { 0 } else { 1 },
                if foreground == 0 { 0 } else { 1 },
                if ime_hide == 0 { 0 } else { 1 }
            ));
            let mut msg: Msg = std::mem::zeroed();
            while GetMessageW(&mut msg, 0, 0, 0) != -1 {}
        });
}

pub fn set_visible(visible: bool) {
    HUD_VISIBLE.store(visible, Ordering::SeqCst);
    if !visible {
        *BARRIER.lock().unwrap() = None;
    }
}

/// True while the frame we just hid must stay hidden.
pub fn barrier_matches(key: &str) -> bool {
    let mut guard = BARRIER.lock().unwrap();
    let Some((barrier_key, at, hold_ms)) = guard.as_ref() else {
        return false;
    };
    if at.elapsed().as_millis() as u64 >= *hold_ms {
        *guard = None;
        return false;
    }
    if barrier_key == key {
        return true;
    }
    // Different content means the user started composing again.
    *guard = None;
    false
}

fn hide_now(reason: &str, hold_ms: u64) {
    let key = crate::current_hud_key();
    *BARRIER.lock().unwrap() = Some((key, Instant::now(), hold_ms));
    HUD_VISIBLE.store(false, Ordering::SeqCst);
    crate::trace(&format!("hide {reason}"));
    if let Some(app) = APP.get() {
        let handle = app.clone();
        let inner = handle.clone();
        let _ = handle.run_on_main_thread(move || crate::hide_hud_now(&inner));
    }
}

fn modifiers_held() -> bool {
    unsafe {
        (GetAsyncKeyState(VK_CONTROL) as u16 & 0x8000) != 0
            || (GetAsyncKeyState(VK_MENU) as u16 & 0x8000) != 0
    }
}

fn foreground_pid() -> Option<u32> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 { return None; }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        (pid != 0).then_some(pid)
    }
}

fn key_event(vk: u16, scan: u16, flags: u32) -> Input {
    Input {
        kind: INPUT_KEYBOARD,
        data: InputUnion { keyboard: KeybdInput { vk, scan, flags, time: 0, extra: 0 } },
    }
}

fn send_input(events: &[Input]) -> bool {
    if events.is_empty() { return false; }
    unsafe { SendInput(events.len() as u32, events.as_ptr(), std::mem::size_of::<Input>() as i32) == events.len() as u32 }
}

/// Cancel the IME composition and type the translation as Unicode into the
/// focused control. Both are queued in one SendInput call so Windows delivers
/// Escape before the characters; there is no artificial pause between them.
/// The HUD is non-activating, so the target keeps focus.
fn translation_events(text:&str)->Vec<Input>{
    let mut events=Vec::with_capacity(text.encode_utf16().count()*2+2);
    events.push(key_event(0x1B,0,0));
    events.push(key_event(0x1B,0,KEYEVENTF_KEYUP));
    for unit in text.encode_utf16(){
        events.push(key_event(0,unit,KEYEVENTF_UNICODE));
        events.push(key_event(0,unit,KEYEVENTF_UNICODE|KEYEVENTF_KEYUP));
    }
    events
}

fn inject_translation(text: &str, pid: u32) -> bool {
    let clean = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if clean.is_empty() || foreground_pid() != Some(pid) { return false; }
    send_input(&translation_events(&clean))
}

/// Keys that make Microsoft Pinyin commit the composition (and close the bar).
fn is_commit_key(vk: u32) -> bool {
    match vk {
        0x20 | 0x0D | 0x09 | 0x1B => true, // space, enter, tab, esc
        0x31..=0x39 => true,               // 1..9 pick a candidate
        0x61..=0x69 => true,               // numpad 1..9
        0xBA | 0xBB | 0xBC | 0xBD | 0xBE | 0xBF | 0xC0 => true, // ; = , - . / `
        0xDB | 0xDD | 0xDE => true,        // [ ] '
        _ => false,
    }
}

unsafe extern "system" fn callback(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0
        && (wparam == WM_KEYDOWN || wparam == WM_SYSKEYDOWN)
        && HUD_VISIBLE.load(Ordering::SeqCst)
        && lparam != 0
    {
        let info = &*(lparam as *const KbdLlHookStruct);
        let shift = unsafe { (GetAsyncKeyState(VK_SHIFT) as u16 & 0x8000) != 0 };
        // Plain Tab completes with the current translation instead of
        // committing the Chinese candidate. Shift/Ctrl/Alt+Tab keep their
        // normal meaning.
        if info.vk_code == VK_TAB && !modifiers_held() && !shift {
            if let Some((pid, text)) = crate::take_hud_completion() {
                if !text.trim().is_empty() && foreground_pid() == Some(pid) {
                    hide_now("tab fill", 700);
                    let text = text.trim().to_owned();
                    std::thread::spawn(move || {
                        let chars = text.chars().count();
                        let ok = inject_translation(&text, pid);
                        crate::trace(&format!("tab fill ok={ok} chars={chars}"));
                    });
                    return 1;
                }
            }
        }
        // Shift+Space toggles full/half width instead of committing.
        let shift_space = info.vk_code == 0x20 && shift;
        if is_commit_key(info.vk_code) && !modifiers_held() && !shift_space {
            hide_now(&format!("key vk=0x{:02X}", info.vk_code), 600);
        } else if info.vk_code == 0x08
            && matches!(crate::composition_letters(), Some(n) if n <= 1)
        {
            // Backspace about to delete the only remaining letter: the bar is
            // certain to close, so take the pill down before the IME reacts.
            hide_now("backspace last letter", 400);
        } else {
            // Backspace / letters / arrows: the bar is about to change, so
            // probe right now and keep probing fast for a few frames.
            crate::wake_monitor();
            crate::urgent_probe(60);
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

unsafe extern "system" fn mouse_callback(code: i32, wparam: usize, lparam: isize) -> isize {
    if code >= 0
        && (wparam == WM_LBUTTONDOWN || wparam == WM_RBUTTONDOWN)
        && HUD_VISIBLE.load(Ordering::SeqCst)
        && lparam != 0
    {
        let info = &*(lparam as *const MsllHookStruct);
        // A click that lands on the candidate bar is the selection itself: the
        // bar closes before the click is even delivered to the app.
        let on_bar = crate::hud_bar_rect()
            .map(|r| {
                info.pt_x >= r.left - 8
                    && info.pt_x <= r.right + 8
                    && info.pt_y >= r.top - 8
                    && info.pt_y <= r.bottom + 8
            })
            .unwrap_or(false);
        if on_bar {
            hide_now("click on bar", 400);
        } else {
            crate::wake_monitor();
            crate::urgent_probe(40);
        }
    }
    CallNextHookEx(0, code, wparam, lparam)
}

/// Last time an IME window overlapping the bar was hidden or destroyed.
/// The monitor uses it as a hint, never as proof: only an empty re-probe
/// may take the pill down.
static OBJECT_HIDE_AT:Mutex<Option<Instant>>=Mutex::new(None);

pub(crate) fn object_hide_recent(window:std::time::Duration)->bool {
    OBJECT_HIDE_AT.lock().ok().and_then(|g|*g).map(|t|t.elapsed()<=window).unwrap_or(false)
}
unsafe extern "system" fn foreground_callback(
    _hook: isize,
    _event: u32,
    _hwnd: isize,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) -> () {
    if HUD_VISIBLE.load(Ordering::SeqCst) {
        // The composition cannot survive losing the foreground window.
        hide_now("foreground", 80);
    }
}

unsafe extern "system" fn object_callback(
    _hook: isize,
    _event: u32,
    hwnd: isize,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) -> () {
    if hwnd == 0 || !HUD_VISIBLE.load(Ordering::SeqCst) {
        return;
    }
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == std::process::id() {
        return; // our own pill
    }
    let Some(bar) = crate::hud_bar_rect() else { return };
    let mut rect: WRect = std::mem::zeroed();
    if GetWindowRect(hwnd, &mut rect) == 0 {
        return;
    }
    if rect.left <= bar.right + 8
        && rect.right >= bar.left - 8
        && rect.top <= bar.bottom + 8
        && rect.bottom >= bar.top - 8
    {
        // A candidate bar repaints by hiding and re-creating child windows all
        // the time, so "some window overlapping the bar got hidden" is NOT
        // proof that the bar is gone. Ask the monitor to re-read the bar right
        // away; it only hides when the probe also comes back empty.
        if let Ok(mut slot)=OBJECT_HIDE_AT.lock(){*slot=Some(Instant::now());}
        crate::wake_monitor();
        crate::urgent_probe(60);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_keys() {
        for vk in [0x20u32, 0x0D, 0x1B, 0x31, 0x39, 0xBC, 0xDE] {
            assert!(is_commit_key(vk), "expected commit for vk={vk:#x}");
        }
        for vk in [0x08u32, 0x25, 0x26, 0x28, 0x41, 0x10] {
            assert!(!is_commit_key(vk), "must not commit for vk={vk:#x}");
        }
    }

    #[test]
    fn escape_is_sent_as_virtual_key() {
        let event = key_event(0x1B, 0, 0);
        unsafe {
            assert_eq!(event.data.keyboard.vk, 0x1B);
            assert_eq!(event.data.keyboard.scan, 0);
            assert_eq!(event.data.keyboard.flags, 0);
        }
    }

    #[test]
    fn unicode_is_sent_via_scan_code() {
        let event = key_event(0, 'h' as u16, KEYEVENTF_UNICODE);
        unsafe {
            assert_eq!(event.data.keyboard.vk, 0);
            assert_eq!(event.data.keyboard.scan, 'h' as u16);
            assert_eq!(event.data.keyboard.flags, KEYEVENTF_UNICODE);
        }
    }

    #[test]
    fn escape_precedes_translation_in_one_queue() {
        let events = translation_events("hello");
        assert_eq!(events.len(), 2 + "hello".encode_utf16().count() * 2);
        unsafe {
            assert_eq!(events[0].data.keyboard.vk, 0x1B);
            assert_eq!(events[1].data.keyboard.flags, KEYEVENTF_KEYUP);
            assert_eq!(events[2].data.keyboard.scan, 'h' as u16);
            assert_eq!(events[2].data.keyboard.flags, KEYEVENTF_UNICODE);
        }
    }
}