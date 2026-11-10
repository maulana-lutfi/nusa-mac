//! Implementasi macOS memakai AppKit lewat objc2 (tanpa Swift, tanpa webview).
//!
//! - Window notch: level di atas menu bar, ikut semua Space, tanpa bayangan.
//! - Klik tembus (ignoresMouseEvents) di luar area notch; monitor mouse global
//!   hanya membangunkan UI saat kursor masuk/keluar area notch → 0% CPU saat diam.
//! - Musik: notifikasi terdistribusi Spotify & Apple Music (event-driven),
//!   detail lagu + kontrol lewat AppleScript (osascript).
//! - Musik dari browser: AppleScript membaca tab YouTube/SoundCloud/dll. di Chrome, Arc,
//!   Edge, Brave & Safari, lalu JavaScript di tab itu (Media Session API) memberi judul,
//!   artis, sampul, posisi, dan kontrol. Hanya berjalan saat browser terbuka; interval
//!   adaptif 5/8/15 dtk.
//! - Clipboard: cek `changeCount` (satu integer) tiap 0,8 dtk di thread latar.

use super::{DragPayload, ScreenInfo};
use crate::browser::{
    js_command, media_filter, parse_poll_output, pick_best, tabs_script, AsQuote, Browser, Candidate, JS_STATE,
};
use crate::media::{load_art, MediaCmd, Player, PlayerState, Track};
use crate::shared::Shared;
use crate::util::{cache_dir, http_download, run, BUNDLE_ID};

use std::cell::{Cell, RefCell};
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::{autoreleasepool, Retained};
use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy,
    NSApplicationDidChangeScreenParametersNotification, NSDragOperation, NSDraggingContext,
    NSDraggingItem, NSDraggingSession, NSDraggingSource, NSEvent, NSEventMask,
    NSEventModifierFlags, NSEventType, NSPasteboard, NSPasteboardTypeString, NSPasteboardWriting,
    NSRunningApplication, NSScreen, NSSound, NSView, NSWindow, NSWindowCollectionBehavior,
    NSWorkspace,
};
use objc2_foundation::{
    NSActivityOptions, NSArray, NSDistributedNotificationCenter, NSNotification,
    NSNotificationCenter, NSObject, NSPoint, NSProcessInfo, NSRect, NSSize, NSString, NSURL,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// NSMainMenuWindowLevel (24) + 3 — sama seperti app notch lain, di atas menu bar.
const NOTCH_WINDOW_LEVEL: isize = 27;

#[cfg_attr(target_os = "macos", link(name = "CoreGraphics", kind = "framework"))]
extern "C" {
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
}

// ---------------------------------------------------------------------------
// Sumber drag (untuk menyeret file/teks keluar dari shelf & clipboard)

struct DragIvars {
    window_number: Cell<isize>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NusaDragSource"]
    #[ivars = DragIvars]
    struct DragSource;

    unsafe impl NSObjectProtocol for DragSource {}

    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(
            &self,
            _session: &NSDraggingSession,
            _context: NSDraggingContext,
        ) -> NSDragOperation {
            NSDragOperation::Copy
        }

        /// AppKit menelan mouseUp selama sesi drag; kirim mouseUp sintetis supaya
        /// egui tidak mengira tombol masih ditekan.
        #[unsafe(method(draggingSession:endedAtPoint:operation:))]
        fn session_ended(&self, _session: &NSDraggingSession, screen_point: NSPoint, _op: NSDragOperation) {
            let Some(mtm) = MainThreadMarker::new() else { return };
            let num = self.ivars().window_number.get();
            let app = NSApplication::sharedApplication(mtm);
            let Some(win) = app.windowWithWindowNumber(num) else { return };
            let loc = win.convertPointFromScreen(screen_point);
            let ev = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
                NSEventType::LeftMouseUp,
                loc,
                NSEventModifierFlags::empty(),
                0.0,
                num,
                None,
                0,
                1,
                0.0,
            );
            if let Some(ev) = ev {
                app.postEvent_atStart(&ev, true);
            }
        }
    }
);

impl DragSource {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DragIvars { window_number: Cell::new(0) });
        unsafe { msg_send![super(this), init] }
    }
}

// ---------------------------------------------------------------------------

#[derive(Default)]
struct HotState {
    /// x, y (kiri-bawah, koordinat layar Cocoa), w, h
    rect: Option<[f64; 4]>,
    inside: bool,
}

pub struct Platform {
    shared: Arc<Shared>,
    mtm: MainThreadMarker,
    window: Option<Retained<NSWindow>>,
    view: Option<Retained<NSView>>,
    hot: Arc<Mutex<HotState>>,
    drag_source: Retained<DragSource>,
    /// mouseDown/mouseDragged terakhir — dibutuhkan untuk memulai sesi drag native.
    last_mouse: Rc<RefCell<Option<Retained<NSEvent>>>>,
    ignore_clip_count: Arc<AtomicIsize>,
    browser_tx: mpsc::Sender<BrowserMsg>,
    _monitors: Vec<Retained<AnyObject>>,
    _observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    _activity: Retained<ProtocolObject<dyn NSObjectProtocol>>,
}

impl Platform {
    pub fn new(cc: &eframe::CreationContext<'_>, shared: Arc<Shared>) -> Self {
        let mtm = MainThreadMarker::new().expect("Platform::new harus di main thread");
        let app = NSApplication::sharedApplication(mtm);
        // Tidak muncul di Dock / Cmd-Tab (Info.plist juga memakai LSUIElement).
        app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

        let mut view = None;
        let mut window = None;
        if let Ok(h) = cc.window_handle() {
            if let RawWindowHandle::AppKit(h) = h.as_raw() {
                let v: Option<Retained<NSView>> =
                    unsafe { Retained::retain(h.ns_view.as_ptr().cast::<NSView>()) };
                if let Some(v) = v {
                    window = v.window();
                    view = Some(v);
                }
            }
        }

        // Cegah App Nap menunda pengingat (tetap mengizinkan Mac tidur).
        let activity = NSProcessInfo::processInfo().beginActivityWithOptions_reason(
            NSActivityOptions::UserInitiatedAllowingIdleSystemSleep,
            &NSString::from_str("Pengingat Nusa"),
        );

        let (browser_tx, browser_rx) = mpsc::channel();
        spawn_browser_media(shared.clone(), browser_rx);
        let mut p = Self {
            shared: shared.clone(),
            mtm,
            window,
            view,
            hot: Arc::new(Mutex::new(HotState::default())),
            drag_source: DragSource::new(mtm),
            last_mouse: Rc::new(RefCell::new(None)),
            ignore_clip_count: Arc::new(AtomicIsize::new(-1)),
            browser_tx,
            _monitors: Vec::new(),
            _observers: Vec::new(),
            _activity: activity,
        };
        if let Some(w) = &p.window {
            p.drag_source.ivars().window_number.set(w.windowNumber());
        }
        p.install_mouse_monitors();
        p.install_observers();
        spawn_clipboard_poller(shared.clone(), p.ignore_clip_count.clone());
        for pl in Player::NATIVE {
            spawn_refresh(shared.clone(), pl, 0);
        }
        p
    }

    fn install_mouse_monitors(&mut self) {
        let mask = NSEventMask::MouseMoved | NSEventMask::LeftMouseDragged;
        let (hot, sh) = (self.hot.clone(), self.shared.clone());
        let global = RcBlock::new(move |_e: NonNull<NSEvent>| on_mouse(&hot, &sh));
        if let Some(m) = NSEvent::addGlobalMonitorForEventsMatchingMask_handler(mask, &global) {
            self._monitors.push(m);
        }
        let (hot, sh) = (self.hot.clone(), self.shared.clone());
        let last = self.last_mouse.clone();
        let local_mask = mask | NSEventMask::LeftMouseDown;
        let local = RcBlock::new(move |e: NonNull<NSEvent>| -> *mut NSEvent {
            let ev = unsafe { e.as_ref() };
            let t = ev.r#type();
            if t == NSEventType::LeftMouseDown || t == NSEventType::LeftMouseDragged {
                *last.borrow_mut() = unsafe { Retained::retain(e.as_ptr()) };
            }
            on_mouse(&hot, &sh);
            e.as_ptr()
        });
        if let Some(m) = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(local_mask, &local) } {
            self._monitors.push(m);
        }
    }

    fn install_observers(&mut self) {
        // Musik: Spotify & Apple Music memposting notifikasi tiap play/pause/ganti lagu.
        let dnc = NSDistributedNotificationCenter::defaultCenter();
        for (name, player) in [
            ("com.spotify.client.PlaybackStateChanged", Player::Spotify),
            ("com.apple.Music.playerInfo", Player::Music),
        ] {
            let sh = self.shared.clone();
            let block = RcBlock::new(move |_n: NonNull<NSNotification>| {
                spawn_refresh(sh.clone(), player, 0);
            });
            let obs = unsafe {
                dnc.addObserverForName_object_queue_usingBlock(
                    Some(&NSString::from_str(name)),
                    None,
                    None,
                    &block,
                )
            };
            self._observers.push(obs);
        }
        // Monitor dicolok/dicabut → hitung ulang posisi notch.
        let sh = self.shared.clone();
        let block = RcBlock::new(move |_n: NonNull<NSNotification>| {
            sh.update(|g| g.screen_changed = true);
        });
        let obs = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                None,
                &block,
            )
        };
        self._observers.push(obs);
    }

    fn target_screen(&self) -> Option<Retained<NSScreen>> {
        // Utamakan layar yang punya notch (layar bawaan MacBook).
        for s in NSScreen::screens(self.mtm).iter() {
            if s.safeAreaInsets().top > 0.0 {
                return Some(s);
            }
        }
        NSScreen::mainScreen(self.mtm)
    }

    pub fn screen(&self) -> ScreenInfo {
        let Some(s) = self.target_screen() else {
            return ScreenInfo { has_notch: false, notch_w: 0.0, notch_h: 24.0 };
        };
        let frame = s.frame();
        let insets = s.safeAreaInsets();
        if insets.top > 0.0 {
            let l = s.auxiliaryTopLeftArea();
            let r = s.auxiliaryTopRightArea();
            let w = frame.size.width - l.size.width - r.size.width;
            ScreenInfo {
                has_notch: true,
                notch_w: w.max(120.0) as f32,
                notch_h: insets.top as f32,
            }
        } else {
            let vf = s.visibleFrame();
            let mb = (frame.origin.y + frame.size.height) - (vf.origin.y + vf.size.height);
            ScreenInfo {
                has_notch: false,
                notch_w: 0.0,
                notch_h: if mb > 10.0 && mb < 60.0 { mb as f32 } else { 24.0 },
            }
        }
    }

    /// Letakkan window di tengah-atas layar, di atas menu bar.
    pub fn configure_window(&self, show_in_fullscreen: bool, size: egui::Vec2) {
        let Some(w) = &self.window else { return };
        w.setLevel(NOTCH_WINDOW_LEVEL);
        w.setHasShadow(false);
        w.setMovable(false);
        let mut cb = NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::IgnoresCycle;
        if show_in_fullscreen {
            cb |= NSWindowCollectionBehavior::FullScreenAuxiliary;
        }
        w.setCollectionBehavior(cb);
        if let Some(s) = self.target_screen() {
            let f = s.frame();
            let (sw, sh) = (size.x as f64, size.y as f64);
            let rect = NSRect::new(
                NSPoint::new(f.origin.x + (f.size.width - sw) / 2.0, f.origin.y + f.size.height - sh),
                NSSize::new(sw, sh),
            );
            w.setFrame_display(rect, true);
        }
    }

    pub fn supports_passthrough(&self) -> bool {
        self.window.is_some()
    }

    /// `r` dalam koordinat lokal window (titik kiri-atas egui).
    pub fn set_hot_rect(&self, r: egui::Rect) {
        let Some(w) = &self.window else { return };
        let f = w.frame();
        let top = f.origin.y + f.size.height;
        let screen = [
            f.origin.x + r.min.x as f64,
            top - r.max.y as f64,
            r.width() as f64,
            r.height() as f64,
        ];
        if let Ok(mut h) = self.hot.lock() {
            h.rect = Some(screen);
        }
    }

    /// Posisi kursor relatif window — tetap valid saat window sedang klik-tembus.
    pub fn cursor_pos(&self, ctx: &egui::Context) -> Option<egui::Pos2> {
        let Some(w) = &self.window else {
            return ctx.input(|i| i.pointer.hover_pos());
        };
        let m = NSEvent::mouseLocation();
        let f = w.frame();
        let x = m.x - f.origin.x;
        let y = (f.origin.y + f.size.height) - m.y;
        Some(egui::pos2(x as f32, y as f32))
    }

    pub fn mouse_button_down(&self) -> bool {
        NSEvent::pressedMouseButtons() != 0
    }

    pub fn media_cmd(&self, p: Player, c: MediaCmd) {
        if p == Player::Browser {
            let _ = self.browser_tx.send(BrowserMsg::Cmd(c));
            return;
        }
        let sh = self.shared.clone();
        std::thread::spawn(move || {
            if !is_running(p) {
                return;
            }
            let verb = match c {
                MediaCmd::PlayPause => "playpause".to_string(),
                MediaCmd::Pause => "pause".to_string(),
                MediaCmd::Next => "next track".to_string(),
                MediaCmd::Prev => "previous track".to_string(),
                MediaCmd::Seek(s) => format!("set player position to {:.2}", s.max(0.0)),
            };
            let script = format!(
                "if application \"{0}\" is running then tell application \"{0}\" to {1}",
                p.app(),
                verb
            );
            run("osascript", &["-e", &script]);
            refresh_player(&sh, p, 250);
        });
    }

    pub fn refresh_media(&self, p: Player) {
        if p == Player::Browser {
            let _ = self.browser_tx.send(BrowserMsg::Refresh);
            return;
        }
        spawn_refresh(self.shared.clone(), p, 0);
    }

    pub fn idle_seconds(&self) -> f64 {
        // kCGEventSourceStateCombinedSessionState = 0, kCGAnyInputEventType = ~0
        unsafe { CGEventSourceSecondsSinceLastEventType(0, u32::MAX) }
    }

    pub fn copy_text(&self, _ctx: &egui::Context, s: &str) {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        pb.setString_forType(&NSString::from_str(s), unsafe { NSPasteboardTypeString });
        self.ignore_clip_count.store(pb.changeCount(), Ordering::Relaxed);
    }

    pub fn copy_files(&self, _ctx: &egui::Context, paths: &[String]) {
        let urls: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = paths
            .iter()
            .map(|p| ProtocolObject::from_retained(NSURL::fileURLWithPath(&NSString::from_str(p))))
            .collect();
        let arr = NSArray::from_retained_slice(&urls);
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        pb.writeObjects(&arr);
        self.ignore_clip_count.store(pb.changeCount(), Ordering::Relaxed);
    }

    /// Mulai sesi drag native (file → Finder/Slack/Mail, teks → app mana pun).
    pub fn start_drag(&self, payload: DragPayload) -> bool {
        let Some(view) = &self.view else { return false };
        let app = NSApplication::sharedApplication(self.mtm);
        let is_mouse = |e: &NSEvent| {
            let t = e.r#type();
            t == NSEventType::LeftMouseDown || t == NSEventType::LeftMouseDragged
        };
        // beginDraggingSession butuh event mouse asli; event lain memicu exception.
        let event = app
            .currentEvent()
            .filter(|e| is_mouse(e))
            .or_else(|| self.last_mouse.borrow().clone());
        let Some(event) = event.filter(|e| is_mouse(e)) else { return false };
        let loc = view.convertPoint_fromView(event.locationInWindow(), None);
        let ws = NSWorkspace::sharedWorkspace();
        let frame_at = |i: usize| {
            NSRect::new(
                NSPoint::new(loc.x - 20.0 + i as f64 * 5.0, loc.y - 20.0 - i as f64 * 5.0),
                NSSize::new(40.0, 40.0),
            )
        };
        let mut items: Vec<Retained<NSDraggingItem>> = Vec::new();
        match payload {
            DragPayload::Files(paths) => {
                for (i, p) in paths.iter().enumerate() {
                    let ns = NSString::from_str(p);
                    let url = NSURL::fileURLWithPath(&ns);
                    let item = NSDraggingItem::initWithPasteboardWriter(
                        NSDraggingItem::alloc(),
                        ProtocolObject::from_ref(&*url),
                    );
                    let icon = ws.iconForFile(&ns);
                    let contents: &AnyObject = &icon;
                    unsafe { item.setDraggingFrame_contents(frame_at(i), Some(contents)) };
                    items.push(item);
                }
            }
            DragPayload::Text(s) => {
                let ns = NSString::from_str(&s);
                let item = NSDraggingItem::initWithPasteboardWriter(
                    NSDraggingItem::alloc(),
                    ProtocolObject::from_ref(&*ns),
                );
                #[allow(deprecated)]
                let icon = ws.iconForFileType(&NSString::from_str("txt"));
                let contents: &AnyObject = &icon;
                unsafe { item.setDraggingFrame_contents(frame_at(0), Some(contents)) };
                items.push(item);
            }
        }
        if items.is_empty() {
            return false;
        }
        let arr = NSArray::from_retained_slice(&items);
        let _session = view.beginDraggingSessionWithItems_event_source(
            &arr,
            &event,
            ProtocolObject::from_ref(&*self.drag_source),
        );
        true
    }

    pub fn open(&self, path: &str) {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }

    pub fn reveal(&self, path: &str) {
        let _ = std::process::Command::new("open").args(["-R", path]).spawn();
    }

    pub fn play_sound(&self) {
        if let Some(s) = NSSound::soundNamed(&NSString::from_str("Glass")) {
            s.play();
        }
    }

    /// Kembalikan fokus ke app sebelumnya saat notch menutup.
    pub fn deactivate(&self) {
        NSApplication::sharedApplication(self.mtm).deactivate();
    }

    /// Start saat login lewat LaunchAgent (berfungsi untuk app tanpa signing).
    pub fn set_launch_at_login(&self, on: bool) -> Result<(), String> {
        let home = dirs::home_dir().ok_or("HOME tidak ditemukan")?;
        let dir = home.join("Library/LaunchAgents");
        let plist = dir.join(format!("{BUNDLE_ID}.plist"));
        if !on {
            let _ = std::fs::remove_file(&plist);
            return Ok(());
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.to_string_lossy().to_string();
        let args = match exe.find(".app/Contents/MacOS/") {
            Some(i) => format!(
                "<string>/usr/bin/open</string><string>-a</string><string>{}</string>",
                xml_escape(&exe[..i + 4])
            ),
            None => format!("<string>{}</string>", xml_escape(&exe)),
        };
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{BUNDLE_ID}</string>
  <key>ProgramArguments</key><array>{args}</array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#
        );
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(&plist, body).map_err(|e| e.to_string())
    }
}

/// Posisi awal window di tengah-atas layar ber-notch (koordinat winit: kiri-atas layar utama),
/// supaya window tidak sempat muncul di pojok kiri atas saat aplikasi dibuka.
pub fn initial_position(w: f32) -> Option<[f32; 2]> {
    let mtm = MainThreadMarker::new()?;
    let screens = NSScreen::screens(mtm);
    let primary_h = screens.iter().next()?.frame().size.height;
    let target = screens
        .iter()
        .find(|s| s.safeAreaInsets().top > 0.0)
        .or_else(|| NSScreen::mainScreen(mtm))?;
    let f = target.frame();
    let x = f.origin.x + (f.size.width - w as f64) / 2.0;
    let y = primary_h - (f.origin.y + f.size.height);
    Some([x as f32, y as f32])
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn on_mouse(hot: &Mutex<HotState>, shared: &Shared) {
    let p = NSEvent::mouseLocation();
    let Ok(mut h) = hot.lock() else { return };
    let inside = h.rect.is_some_and(|r| {
        p.x >= r[0] && p.x <= r[0] + r[2] && p.y >= r[1] && p.y <= r[1] + r[3]
    });
    if inside != h.inside || shared.wake_on_move() {
        h.inside = inside;
        drop(h);
        shared.ctx.request_repaint();
    }
}

// ---------------------------------------------------------------------------
// Clipboard

fn spawn_clipboard_poller(shared: Arc<Shared>, ignore: Arc<AtomicIsize>) {
    std::thread::Builder::new()
        .name("clipboard".into())
        .spawn(move || {
            let mut last: isize = -1;
            loop {
                std::thread::sleep(Duration::from_millis(800));
                if !shared.clipboard_enabled.load(Ordering::Relaxed) {
                    continue;
                }
                autoreleasepool(|_| {
                    let pb = NSPasteboard::generalPasteboard();
                    let c = pb.changeCount();
                    if last == -1 || c == last {
                        last = c;
                        return;
                    }
                    last = c;
                    if c == ignore.load(Ordering::Relaxed) {
                        return;
                    }
                    // Hormati penanda password manager (nspasteboard.org).
                    if let Some(types) = pb.types() {
                        for t in types.iter() {
                            let t = t.to_string();
                            if t == "org.nspasteboard.ConcealedType"
                                || t == "org.nspasteboard.TransientType"
                                || t == "org.nspasteboard.AutoGeneratedType"
                            {
                                return;
                            }
                        }
                    }
                    if let Some(s) = pb.stringForType(unsafe { NSPasteboardTypeString }) {
                        let s = s.to_string();
                        if !s.trim().is_empty() && s.len() < 100_000 {
                            shared.update(|g| g.clip_new.push(s));
                        }
                    }
                });
            }
        })
        .ok();
}

// ---------------------------------------------------------------------------
// Musik via AppleScript

const SPOTIFY_INFO: &str = r#"if application "Spotify" is running then
	tell application "Spotify"
		try
			set t to current track
			set s to player state as text
			set au to ""
			try
				set au to artwork url of t
			end try
			return (name of t) & linefeed & (artist of t) & linefeed & (album of t) & linefeed & (((duration of t) / 1000) as text) & linefeed & (player position as text) & linefeed & s & linefeed & au & linefeed & (id of t)
		end try
	end tell
end if
return """#;

const MUSIC_INFO: &str = r#"if application "Music" is running then
	tell application "Music"
		try
			set t to current track
			set s to player state as text
			return (name of t) & linefeed & (artist of t) & linefeed & (album of t) & linefeed & ((duration of t) as text) & linefeed & (player position as text) & linefeed & s & linefeed & "" & linefeed & (persistent ID of t)
		end try
	end tell
end if
return """#;

fn music_art_script(out: &str) -> String {
    format!(
        r#"set outPath to "{out}"
tell application "Music"
	try
		set d to raw data of artwork 1 of current track
	on error
		return ""
	end try
end tell
set f to open for access (POSIX file outPath) with write permission
set eof f to 0
write d to f
close access f
return outPath"#
    )
}

fn is_running(p: Player) -> bool {
    !p.bundle_id().is_empty() && bundle_running(p.bundle_id())
}

fn bundle_running(bundle_id: &str) -> bool {
    autoreleasepool(|_| {
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(bundle_id)).count() > 0
    })
}

fn num(s: &str) -> f64 {
    // AppleScript memakai pemisah desimal sesuai locale (mis. "12,5" di Indonesia).
    s.trim().replace(',', ".").parse().unwrap_or(0.0)
}

fn spawn_refresh(shared: Arc<Shared>, p: Player, delay_ms: u64) {
    std::thread::spawn(move || refresh_player(&shared, p, delay_ms));
}

fn refresh_player(shared: &Shared, p: Player, delay_ms: u64) {
    if delay_ms > 0 {
        std::thread::sleep(Duration::from_millis(delay_ms));
    }
    // Jangan pernah memanggil osascript untuk app yang tidak berjalan/terpasang
    // (bisa memunculkan dialog "Where is …?").
    if !is_running(p) {
        shared.set_player(p, None);
        return;
    }
    let script = match p {
        Player::Spotify => SPOTIFY_INFO,
        Player::Music => MUSIC_INFO,
        Player::Browser => return,
    };
    let out = run("osascript", &["-e", script]).unwrap_or_default();
    let lines: Vec<&str> = out.trim_end_matches('\n').split('\n').collect();
    if lines.len() < 8 || lines[0].trim().is_empty() {
        shared.set_player(p, None);
        return;
    }
    let track = Track {
        title: lines[0].to_string(),
        artist: lines[1].to_string(),
        album: lines[2].to_string(),
        duration: num(lines[3]),
        position: num(lines[4]),
        playing: lines[5].trim() == "playing",
        id: lines[7].trim().to_string(),
        ..Default::default()
    };
    let art_src = lines[6].trim().to_string();
    let prev = shared.get_player(p);
    let art_key = format!("{}:{}", track.id, art_src);
    let (art, art_key) = match prev {
        Some(ps) if ps.art_key == art_key => (ps.art, art_key),
        _ => (fetch_art(p, &art_src), art_key),
    };
    shared.set_player(
        p,
        Some(PlayerState {
            track,
            at: Instant::now(),
            art,
            art_key,
        }),
    );
}

fn fetch_art(p: Player, src: &str) -> Option<Arc<egui::ColorImage>> {
    let path = match p {
        Player::Spotify => {
            if src.is_empty() {
                return None;
            }
            let dest = cache_dir().join("art-spotify.img");
            if !http_download(src, &dest) {
                return None;
            }
            dest
        }
        Player::Browser => {
            if src.is_empty() {
                return None;
            }
            let dest = cache_dir().join("art-browser.img");
            if !http_download(src, &dest) {
                return None;
            }
            dest
        }
        Player::Music => {
            let dest = cache_dir().join("art-music.img");
            let script = music_art_script(&dest.to_string_lossy());
            let out = run("osascript", &["-e", &script])?;
            if out.trim().is_empty() {
                return None;
            }
            dest
        }
    };
    let bytes = std::fs::read(path).ok()?;
    load_art(&bytes).map(Arc::new)
}

// ---------------------------------------------------------------------------
// Musik dari browser (AppleScript + JavaScript di tab)

enum BrowserMsg {
    Cmd(MediaCmd),
    Refresh,
}

fn spawn_browser_media(shared: Arc<Shared>, rx: mpsc::Receiver<BrowserMsg>) {
    std::thread::Builder::new()
        .name("browser-media".into())
        .spawn(move || {
            let mut art_cache: (String, Option<Arc<egui::ColorImage>>) = (String::new(), None);
            let mut shown = false;
            loop {
                let mut wait = Duration::from_secs(15);
                let enabled = shared.browser_enabled.load(Ordering::Relaxed);
                let running: Vec<Browser> = if enabled {
                    Browser::ALL.into_iter().filter(|b| bundle_running(b.bundle())).collect()
                } else {
                    Vec::new()
                };
                let cands: Vec<Candidate> = running
                    .iter()
                    .flat_map(|b| {
                        let script = tabs_script(*b, &media_filter(), JS_STATE);
                        parse_poll_output(*b, &run("osascript", &["-e", &script]).unwrap_or_default())
                    })
                    .collect();
                let target: Option<(Browser, String)>;
                match pick_best(&cands) {
                    Some(c) => {
                        let track = c.to_track();
                        let art_src = c.state.as_ref().map(|s| s.art.clone()).unwrap_or_default();
                        if art_cache.0 != art_src {
                            art_cache = (art_src.clone(), fetch_art(Player::Browser, &art_src));
                        }
                        wait = Duration::from_secs(if track.playing { 5 } else { 8 });
                        target = Some((c.browser, c.url.clone()));
                        shared.set_player(
                            Player::Browser,
                            Some(PlayerState {
                                track,
                                at: Instant::now(),
                                art: art_cache.1.clone(),
                                art_key: format!("browser:{}", art_cache.0),
                            }),
                        );
                        shown = true;
                    }
                    None => {
                        target = None;
                        if shown {
                            shared.set_player(Player::Browser, None);
                            shown = false;
                        }
                        if running.is_empty() {
                            wait = Duration::from_secs(5);
                        }
                    }
                }
                match rx.recv_timeout(wait) {
                    Ok(BrowserMsg::Cmd(cmd)) => {
                        if let Some((b, url)) = &target {
                            if bundle_running(b.bundle()) {
                                let filter = format!("u is \"{}\"", url.as_quoted());
                                let script = tabs_script(*b, &filter, &js_command(cmd));
                                run("osascript", &["-e", &script]);
                                std::thread::sleep(Duration::from_millis(350));
                            }
                        }
                    }
                    Ok(BrowserMsg::Refresh) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            }
        })
        .ok();
}
