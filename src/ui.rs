use crate::{
    config::Config,
    drawing::{self, rect},
    model::{self, Sample},
};
use objc2::AnyThread;
use objc2::{
    DefinedClass, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
    sel,
};
use objc2_app_kit::*;
use objc2_foundation::*;
use std::{
    cell::{Cell, OnceCell, RefCell},
    collections::VecDeque,
    rc::Rc,
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};

pub struct State {
    pub config: Config,
    pub sample: Option<Sample>,
    pub error: String,
    pub updated: String,
    pub history: VecDeque<(Instant, f64)>,
}
impl State {
    fn new(config: Config) -> Self {
        Self {
            config,
            sample: None,
            error: "esperando lectura…".into(),
            updated: String::new(),
            history: VecDeque::new(),
        }
    }
    fn apply(&mut self, result: Result<Sample, String>) {
        match result {
            Ok(s) => {
                self.history.push_back((Instant::now(), s.tps));
                self.sample = Some(s);
                self.error.clear();
                let fmt = NSDateFormatter::new();
                fmt.setDateFormat(Some(ns_string!("HH:mm:ss")));
                self.updated = fmt.stringFromDate(&NSDate::now()).to_string();
            }
            Err(e) => {
                self.sample = None;
                self.error = e;
                self.history.push_back((Instant::now(), -1.));
            }
        }
        while self.history.len() > self.config.history {
            self.history.pop_front();
        }
    }
    fn summary(&self) -> String {
        match &self.sample {
            Some(s) => {
                let b = s.bar();
                format!(
                    "Kairos · GPU {} · RAM {} · {} tok/s. CPU {}. Modelo {}. {}",
                    b[0],
                    b[1],
                    b[2],
                    model::percent(s.n("/cpu/percent")),
                    s.s("/vllmThroughput/model"),
                    s.consumers()
                )
            }
            None => format!("Kairos · {}", self.error),
        }
    }
}
pub struct PanelIvars {
    state: Rc<RefCell<State>>,
}
define_class!(
    // SAFETY: NSView's drawing and responder methods run on the main thread.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PanelIvars]
    pub struct PanelView;
    unsafe impl NSObjectProtocol for PanelView {}
    impl PanelView {
        #[unsafe(method(isFlipped))] fn flipped(&self)->bool{true}
        #[unsafe(method(drawRect:))] fn draw(&self,_dirty:NSRect){drawing::panel(&self.ivars().state.borrow());}
    }
);
impl PanelView {
    fn new(mtm: MainThreadMarker, state: Rc<RefCell<State>>) -> Retained<Self> {
        let h = drawing::height(&state.borrow().config);
        let this = Self::alloc(mtm).set_ivars(PanelIvars { state });
        unsafe { msg_send![super(this),initWithFrame:rect(0.,0.,drawing::WIDTH,h)] }
    }
}
struct AppIvars {
    state: Rc<RefCell<State>>,
    status: OnceCell<Retained<NSStatusItem>>,
    popover: OnceCell<Retained<NSPopover>>,
    panel: OnceCell<Retained<PanelView>>,
    tx: Sender<(u64, Config)>,
    rx: Receiver<(u64, Result<Sample, String>)>,
    revision: Cell<u64>,
    settings: OnceCell<crate::settings::Settings>,
    config_path: std::path::PathBuf,
    show_settings: bool,
    busy: Cell<bool>,
    next: Cell<Instant>,
    was_open: Cell<bool>,
    show_on_start: bool,
}
define_class!(
    // SAFETY: all AppKit objects are retained for the lifetime of the main-thread delegate.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppIvars]
    struct App;
    unsafe impl NSObjectProtocol for App {}
    unsafe impl NSApplicationDelegate for App {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn launched(&self,_note:&NSNotification){self.setup();}
        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn reopen(&self,_app:&NSApplication,_visible:bool)->bool {
            if self.ivars().popover.get().is_some_and(|p| !p.isShown()) { self.toggle(); }
            true
        }
    }
    impl App {
        #[unsafe(method(tick:))] fn tick(&self,_timer:&NSTimer){self.poll();}
        #[unsafe(method(clicked:))] fn clicked(&self,_sender:&AnyObject){
            let app=NSApplication::sharedApplication(self.mtm());
            match app.currentEvent().map(|e|e.r#type()) {
                Some(NSEventType::RightMouseUp)=>self.dashboard(),
                Some(NSEventType::OtherMouseUp)=>self.refresh(),
                _=>self.toggle(),
            }
        }
        #[unsafe(method(refresh:))] fn on_refresh(&self,_sender:&AnyObject){self.refresh();}
        #[unsafe(method(dashboard:))] fn on_dashboard(&self,_sender:&AnyObject){self.dashboard();}
        #[unsafe(method(settings:))] fn on_settings(&self,_sender:&AnyObject){self.open_settings();}
        #[unsafe(method(saveSettings:))] fn on_save_settings(&self,_sender:&AnyObject){self.save_settings();}
        #[unsafe(method(cancelSettings:))] fn on_cancel_settings(&self,_sender:&AnyObject){if let Some(s)=self.ivars().settings.get(){s.close();}}
        #[unsafe(method(resetSettings:))] fn on_reset_settings(&self,_sender:&AnyObject){if let Some(s)=self.ivars().settings.get(){s.populate(&Config::default());}}
        #[unsafe(method(quit:))] fn on_quit(&self,_sender:&AnyObject){NSApplication::sharedApplication(self.mtm()).terminate(None);}
    }
);
impl App {
    fn setup(&self) {
        let iv = self.ivars();
        let mtm = self.mtm();
        let state = iv.state.borrow();
        let status =
            NSStatusBar::systemStatusBar().statusItemWithLength(drawing::bar_width(&state.config));
        let button = status.button(mtm).expect("status button");
        // SAFETY: the target is this retained delegate; selectors have the matching action signature.
        unsafe {
            button.setTarget(Some(self));
            button.setAction(Some(sel!(clicked:)));
        }
        button.sendActionOn(
            NSEventMask::LeftMouseUp | NSEventMask::RightMouseUp | NSEventMask::OtherMouseUp,
        );
        button.setAccessibilityLabel(Some(ns_string!("Kairos Monitor")));
        let popover = NSPopover::new(mtm);
        popover.setBehavior(NSPopoverBehavior::Transient);
        popover.setAppearance(
            NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
        );
        let controller = NSViewController::new(mtm);
        drop(state);
        let panel = PanelView::new(mtm, iv.state.clone());
        let h = drawing::height(&iv.state.borrow().config);
        for (title, x, w, action) in [
            ("↻ Refrescar", 20., 107., sel!(refresh:)),
            ("↗ Dashboard", 135., 111., sel!(dashboard:)),
            ("Ajustes", 471., 77., sel!(settings:)),
            ("Salir", 566., 74., sel!(quit:)),
        ] {
            let b = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(title),
                    Some(self),
                    Some(action),
                    mtm,
                )
            };
            b.setFrame(rect(x, h - 25., w, 20.));
            b.setBordered(false);
            b.setFont(Some(&NSFont::monospacedSystemFontOfSize_weight(
                10.,
                unsafe { NSFontWeightRegular },
            )));
            b.setContentTintColor(Some(&drawing::color(&iv.state.borrow().config.muted, 1.)));
            panel.addSubview(&b);
        }
        controller.setView(&panel);
        popover.setContentViewController(Some(&controller));
        popover.setContentSize(NSSize::new(drawing::WIDTH, h));
        iv.status.set(status).ok();
        iv.popover.set(popover).ok();
        iv.panel.set(panel).ok();
        unsafe {
            let timer = NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                0.25,
                self,
                sel!(tick:),
                None,
                true,
            );
            timer.setTolerance(0.1);
        }
        self.redraw();
        self.refresh();
        if iv.show_settings {
            self.open_settings();
        } else if iv.show_on_start {
            self.toggle();
        }
    }
    fn refresh(&self) {
        let iv = self.ivars();
        if !iv.busy.replace(true)
            && iv
                .tx
                .send((iv.revision.get(), iv.state.borrow().config.clone()))
                .is_err()
        {
            iv.busy.set(false);
            iv.state.borrow_mut().apply(Err("Worker detenido".into()));
            self.redraw();
        }
    }
    fn poll(&self) {
        let iv = self.ivars();
        let opened = iv.popover.get().is_some_and(|p| p.isShown());
        let changed = iv.was_open.replace(opened) != opened;
        if let Ok((revision, result)) = iv.rx.try_recv() {
            iv.busy.set(false);
            if revision != iv.revision.get() {
                self.refresh();
                return;
            }
            iv.state.borrow_mut().apply(result);
            self.redraw();
            let c = &iv.state.borrow().config;
            iv.next.set(
                Instant::now()
                    + std::time::Duration::from_secs(if opened {
                        c.open_interval
                    } else {
                        c.interval
                    }),
            );
        }
        if changed && opened {
            self.refresh();
        }
        if Instant::now() >= iv.next.get() {
            self.refresh();
        }
    }
    fn redraw(&self) {
        let iv = self.ivars();
        let state = iv.state.borrow();
        if let Some(button) = iv.status.get().and_then(|s| s.button(self.mtm())) {
            // Resolve the menu bar's own effective appearance (the panel is always the X1 dark theme).
            let appearance = button.effectiveAppearance();
            #[allow(deprecated)]
            unsafe {
                NSAppearance::setCurrentAppearance(Some(&appearance));
            }
            button.setImage(Some(&drawing::bar(&state, &NSColor::labelColor())));
            button.setToolTip(Some(&NSString::from_str(&format!(
                "{}\nIzquierdo: panel · Derecho: dashboard · Central: refrescar",
                state.summary()
            ))));
            button.setAccessibilityLabel(Some(&NSString::from_str(&state.summary())));
            #[allow(deprecated)]
            unsafe {
                NSAppearance::setCurrentAppearance(None);
            }
        }
        if let Some(panel) = iv.panel.get() {
            panel.setNeedsDisplay(true);
            panel.setAccessibilityLabel(Some(&NSString::from_str(&state.summary())));
        }
    }
    fn toggle(&self) {
        let iv = self.ivars();
        if let Some(p) = iv.popover.get() {
            if p.isShown() {
                unsafe {
                    p.performClose(None);
                }
            } else if let Some(b) = iv.status.get().and_then(|s| s.button(self.mtm())) {
                self.refresh();
                #[allow(deprecated)]
                NSApplication::sharedApplication(self.mtm()).activateIgnoringOtherApps(true);
                p.showRelativeToRect_ofView_preferredEdge(b.bounds(), &b, NSRectEdge::MinY);
                if let Some(window) = p.contentViewController().and_then(|c| c.view().window()) {
                    window.makeKeyWindow();
                }
            }
        }
    }
    fn open_settings(&self) {
        let iv = self.ivars();
        if let Some(p) = iv.popover.get() {
            p.close();
        }
        let settings = iv
            .settings
            .get_or_init(|| crate::settings::Settings::new(self.mtm(), self));
        settings.show(&iv.state.borrow().config);
    }
    fn save_settings(&self) {
        let iv = self.ivars();
        let Some(settings) = iv.settings.get() else {
            return;
        };
        let edited = settings.read(&iv.state.borrow().config);
        let config = match edited.and_then(|c| {
            c.save(&iv.config_path)?;
            Ok(c)
        }) {
            Ok(c) => c,
            Err(e) => {
                settings.error(&e);
                return;
            }
        };
        let height = drawing::height(&config);
        let width = drawing::bar_width(&config);
        {
            let mut state = iv.state.borrow_mut();
            if state.config.url != config.url || state.config.service != config.service {
                state.sample = None;
                state.history.clear();
                state.error = "esperando lectura…".into();
            }
            state.config = config;
            while state.history.len() > state.config.history {
                state.history.pop_front();
            }
        }
        iv.revision.set(iv.revision.get() + 1);
        if let Some(status) = iv.status.get() {
            status.setLength(width);
        }
        if let Some(panel) = iv.panel.get() {
            panel.setFrameSize(NSSize::new(drawing::WIDTH, height));
            for button in panel.subviews().iter() {
                let mut r = button.frame();
                r.origin.y = height - 25.;
                button.setFrame(r);
            }
        }
        if let Some(popover) = iv.popover.get() {
            popover.setContentSize(NSSize::new(drawing::WIDTH, height));
        }
        iv.next.set(Instant::now());
        self.redraw();
        self.refresh();
        settings.close();
    }
    fn dashboard(&self) {
        let url = self.ivars().state.borrow().config.dashboard.clone();
        if let Some(url) = NSURL::URLWithString(&NSString::from_str(&url)) {
            NSWorkspace::sharedWorkspace().openURL(&url);
        }
    }
}

pub fn run(config: Config, config_path: std::path::PathBuf, show: bool, show_settings: bool) {
    let mtm = MainThreadMarker::new().expect("main thread");
    let application = NSApplication::sharedApplication(mtm);
    application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let (tx, requests) = mpsc::channel();
    let (results, rx) = mpsc::channel();
    std::thread::spawn(move || {
        while let Ok((revision, worker_config)) = requests.recv() {
            if results
                .send((revision, model::fetch(&worker_config)))
                .is_err()
            {
                break;
            }
        }
    });
    let delegate = App::alloc(mtm).set_ivars(AppIvars {
        state: Rc::new(RefCell::new(State::new(config))),
        status: OnceCell::new(),
        popover: OnceCell::new(),
        panel: OnceCell::new(),
        tx,
        rx,
        revision: Cell::new(0),
        settings: OnceCell::new(),
        config_path,
        show_settings,
        busy: Cell::new(false),
        next: Cell::new(Instant::now()),
        was_open: Cell::new(false),
        show_on_start: show,
    });
    let delegate: Retained<App> = unsafe { msg_send![super(delegate), init] };
    application.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    application.run();
}

pub fn snapshot(config: Config, sample: Result<Sample, String>, path: &str) {
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let mut state = State::new(config);
    state.apply(sample);
    let image = NSImage::initWithSize(
        NSImage::alloc(),
        NSSize::new(drawing::WIDTH, drawing::height(&state.config)),
    );
    #[allow(deprecated)]
    image.lockFocusFlipped(true);
    drawing::panel(&state);
    #[allow(deprecated)]
    image.unlockFocus();
    let bitmap = NSBitmapImageRep::imageRepWithData(&image.TIFFRepresentation().unwrap()).unwrap();
    let data = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    assert!(data.writeToFile_atomically(&NSString::from_str(path), true));
}
