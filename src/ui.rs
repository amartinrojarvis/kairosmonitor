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
    pub kind: Option<model::ServerKind>,
    pub error: String,
    pub updated: String,
    pub history: VecDeque<(Instant, f64)>,
    clock: Retained<NSDateFormatter>,
}
impl State {
    fn new(config: Config) -> Self {
        let clock = NSDateFormatter::new();
        clock.setDateFormat(Some(ns_string!("HH:mm:ss")));
        Self {
            clock,
            config,
            sample: None,
            kind: None,
            error: "esperando lectura…".into(),
            updated: String::new(),
            history: VecDeque::new(),
        }
    }
    fn apply(&mut self, result: Result<Sample, String>) {
        match result {
            Ok(s) => {
                self.kind = Some(s.kind);
                self.history.push_back((Instant::now(), s.tps));
                self.sample = Some(s);
                self.error.clear();
                self.updated = self.clock.stringFromDate(&NSDate::now()).to_string();
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
                if s.kind == model::ServerKind::OpenAI {
                    return format!(
                        "{} · OpenAI compatible\n{}\nSin telemetría de hardware",
                        s.s("/hostname"),
                        s.model_summary()
                    );
                }
                let b = s.bar();
                format!(
                    "Kairos · GPU {} · RAM {} · {} tok/s. CPU {}. Modelo {}. {}\n{}\nGPT semanal: {}",
                    b[0],
                    b[1],
                    b[2],
                    model::percent(s.n("/cpu/percent")),
                    s.model_summary(),
                    s.consumers(),
                    s.alert_summary(&self.config),
                    model::percent(crate::usage::used(s.usage()))
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
/// A static backing color keeps the native template and selection drawing intact.
fn apply_bar_background(button: &NSStatusBarButton, background: crate::config::BarBackground) {
    button.setWantsLayer(true);
    if let Some(layer) = button.layer() {
        let color = background.hex().map(|hex| drawing::color(hex, 0.24));
        layer.setBackgroundColor(color.as_ref().map(|color| color.CGColor()).as_deref());
        layer.setCornerRadius(6.);
    }
}

struct AlertOverlayIvars {
    values: RefCell<Vec<drawing::AlertValue>>,
}
define_class!(
    // Only the affected numerals are painted here; the button retains its native template.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = AlertOverlayIvars]
    struct AlertOverlay;
    unsafe impl NSObjectProtocol for AlertOverlay {}
    impl AlertOverlay {
        #[unsafe(method(isFlipped))] fn flipped(&self) -> bool { true }
        // Let the status button receive all mouse actions, including over colored numerals.
        #[unsafe(method(hitTest:))] fn hit_test(&self, _point: NSPoint) -> *mut NSView { std::ptr::null_mut() }
        #[unsafe(method(viewDidChangeEffectiveAppearance))]
        fn appearance_changed(&self) {
            unsafe { let _: () = msg_send![super(self), viewDidChangeEffectiveAppearance]; }
            self.setNeedsDisplay(true);
        }
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            let dark = self.effectiveAppearance().bestMatchFromAppearancesWithNames(
                &NSArray::from_slice(&[unsafe { NSAppearanceNameAqua }, unsafe { NSAppearanceNameDarkAqua }]),
            ).is_some_and(|name| &*name == unsafe { NSAppearanceNameDarkAqua });
            drawing::alert_values(&self.ivars().values.borrow(), dark, NSPoint::new(0., 0.));
        }
    }
);
impl AlertOverlay {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AlertOverlayIvars {
            values: RefCell::new(Vec::new()),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        this.setAutoresizingMask(
            NSAutoresizingMaskOptions::ViewMinXMargin
                | NSAutoresizingMaskOptions::ViewMaxXMargin
                | NSAutoresizingMaskOptions::ViewMinYMargin
                | NSAutoresizingMaskOptions::ViewMaxYMargin,
        );
        this.setAccessibilityElement(false);
        this
    }
    fn update(&self, button: &NSStatusBarButton, artwork: &drawing::BarArtwork) {
        let size = artwork.image.size();
        let bounds = button.bounds();
        self.setFrame(rect(
            (bounds.size.width - size.width) / 2.,
            (bounds.size.height - size.height) / 2.,
            size.width,
            size.height,
        ));
    }
}
struct ProbeReply {
    input: String,
    result: Result<Sample, String>,
}
enum WorkerRequest {
    Metrics(u64, Config),
    Probe(Config),
    CheckUpdate,
    InstallUpdate(crate::updates::Release, std::path::PathBuf),
}
enum UpdateReply {
    Checked(Result<Option<crate::updates::Release>, String>),
    Prepared(Result<crate::updates::Prepared, String>),
}
struct AppIvars {
    state: Rc<RefCell<State>>,
    status: OnceCell<Retained<NSStatusItem>>,
    alerts: OnceCell<Retained<AlertOverlay>>,
    popover: OnceCell<Retained<NSPopover>>,
    panel: OnceCell<Retained<PanelView>>,
    tx: Sender<WorkerRequest>,
    rx: Receiver<(u64, Result<Sample, String>)>,
    revision: Cell<u64>,
    probe_rx: Receiver<ProbeReply>,
    update_rx: Receiver<UpdateReply>,
    update_prefs: RefCell<crate::updates::Preferences>,
    update_busy: Cell<bool>,
    update_message: RefCell<String>,
    update_window: OnceCell<crate::update_window::UpdateWindow>,
    update_button: OnceCell<Retained<NSButton>>,
    settings: OnceCell<crate::settings::Settings>,
    config_path: std::path::PathBuf,
    show_settings: bool,
    busy: Cell<bool>,
    timer: RefCell<Option<Retained<NSTimer>>>,
    last_bar: RefCell<String>,
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
    unsafe impl NSPopoverDelegate for App {
        #[unsafe(method(popoverDidShow:))]
        fn popover_opened(&self, _note: &NSNotification) {
            if let Some(b) = self.ivars().status.get().and_then(|s| s.button(self.mtm())) { b.highlight(true); }
            self.refresh();
        }
        #[unsafe(method(popoverDidClose:))]
        fn popover_closed(&self, _note: &NSNotification) {
            if let Some(b) = self.ivars().status.get().and_then(|s| s.button(self.mtm())) { b.highlight(false); }
            self.schedule();
        }
    }
    impl App {
        #[unsafe(method(tick:))] fn tick(&self,_timer:&NSTimer){self.refresh();self.check_updates(false);}
        #[unsafe(method(metricsReady:))] fn metrics_ready(&self,_note:&NSNotification){self.poll();}
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
        #[unsafe(method(testConnection:))] fn test_connection(&self, _sender: &AnyObject) { self.probe(); }
        #[unsafe(method(probeReady:))] fn probe_ready(&self, _note: &NSNotification) { self.probe_result(); }
        #[unsafe(method(updates:))] fn on_updates(&self, _sender: &AnyObject) { self.open_updates(); }
        #[unsafe(method(checkUpdates:))] fn on_check_updates(&self, _sender: &AnyObject) { self.check_updates(true); }
        #[unsafe(method(installUpdate:))] fn on_install_update(&self, _sender: &AnyObject) { self.install_update(); }
        #[unsafe(method(automaticUpdates:))] fn on_automatic_updates(&self, _sender: &AnyObject) { self.automatic_updates(); }
        #[unsafe(method(updatesReady:))] fn on_updates_ready(&self, _note: &NSNotification) { self.update_result(); }
        #[unsafe(method(about:))] fn on_about(&self, _sender: &AnyObject) { self.about(); }
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
            NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        let button = status.button(mtm).expect("status button");
        button.setImageScaling(NSImageScaling::ScaleNone);
        let alerts = AlertOverlay::new(mtm);
        button.addSubview(&alerts);
        iv.alerts.set(alerts).ok();
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
        popover.setDelegate(Some(ProtocolObject::from_ref(self)));
        popover.setAppearance(
            NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
        );
        let controller = NSViewController::new(mtm);
        drop(state);
        let panel = PanelView::new(mtm, iv.state.clone());
        let h = drawing::height(&iv.state.borrow().config);
        iv.update_button
            .set(footer_buttons(&panel, Some(self), mtm, h))
            .ok();
        controller.setView(&panel);
        popover.setContentViewController(Some(&controller));
        popover.setContentSize(NSSize::new(drawing::WIDTH, h));
        iv.status.set(status).ok();
        iv.popover.set(popover).ok();
        iv.panel.set(panel).ok();
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                self,
                sel!(metricsReady:),
                Some(ns_string!("KairosMetricsReady")),
                None,
            );
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                self,
                sel!(probeReady:),
                Some(ns_string!("KairosProbeReady")),
                None,
            );
        }
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                self,
                sel!(updatesReady:),
                Some(ns_string!("KairosUpdatesReady")),
                None,
            );
        }
        self.redraw();
        self.refresh();
        self.update_controls();
        self.check_updates(false);
        if iv.show_settings {
            self.open_settings();
        } else if iv.show_on_start {
            self.toggle();
        }
    }
    fn schedule(&self) {
        let iv = self.ivars();
        if let Some(timer) = iv.timer.borrow_mut().take() {
            timer.invalidate();
        }
        if iv.busy.get() {
            return;
        }
        let c = &iv.state.borrow().config;
        let seconds = if iv.popover.get().is_some_and(|p| p.isShown()) {
            c.open_interval
        } else {
            c.interval
        };
        // One wake per request, coalescible by macOS; no repeating UI polling timer.
        let timer = unsafe {
            NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                seconds as f64,
                self,
                sel!(tick:),
                None,
                false,
            )
        };
        timer.setTolerance(seconds as f64 * 0.1);
        *iv.timer.borrow_mut() = Some(timer);
    }
    fn refresh(&self) {
        let iv = self.ivars();
        if iv.busy.replace(true) {
            return;
        }
        if let Some(timer) = iv.timer.borrow_mut().take() {
            timer.invalidate();
        }
        if iv
            .tx
            .send(WorkerRequest::Metrics(
                iv.revision.get(),
                iv.state.borrow().config.clone(),
            ))
            .is_err()
        {
            iv.busy.set(false);
            iv.state
                .borrow_mut()
                .apply(Err("Conexión interrumpida".into()));
            self.redraw();
            self.schedule();
        }
    }
    fn poll(&self) {
        let iv = self.ivars();
        if let Ok((revision, result)) = iv.rx.try_recv() {
            iv.busy.set(false);
            if revision != iv.revision.get() {
                self.refresh();
                return;
            }
            iv.state.borrow_mut().apply(result);
            self.redraw();
            self.schedule();
        }
    }
    fn redraw(&self) {
        let iv = self.ivars();
        let state = iv.state.borrow();
        if let Some(settings) = iv.settings.get()
            && let Some(sample) = &state.sample
        {
            settings.update_models(&state.config.url, sample);
        }
        if let Some(button) = iv.status.get().and_then(|s| s.button(self.mtm())) {
            let key = format!(
                "{:?}:{:?}:{:?}:{:?}:{}:{}:{}:{:?}:{}:{}:{:?}",
                state.sample.as_ref().map(Sample::bar),
                state.sample.as_ref().map(|s| s.bar_alerts(&state.config)),
                state.kind,
                state.sample.as_ref().map(|s| &s.models),
                state.config.show_gpu,
                state.config.show_memory,
                state.config.show_tps,
                state.config.bar_background,
                state.config.show_gpt,
                state
                    .sample
                    .as_ref()
                    .map(|s| crate::usage::used(s.usage()).to_string())
                    .unwrap_or_default(),
                state.config.bar_order
            );
            if *iv.last_bar.borrow() != key {
                let artwork = drawing::bar(&state);
                iv.status
                    .get()
                    .unwrap()
                    .setLength(artwork.image.size().width + 2. * drawing::BAR_PADDING);
                button.setImage(Some(&artwork.image));
                apply_bar_background(&button, state.config.bar_background);
                if let Some(overlay) = iv.alerts.get() {
                    overlay.update(&button, &artwork);
                    *overlay.ivars().values.borrow_mut() = artwork.alerts;
                    overlay.setNeedsDisplay(true);
                }
                *iv.last_bar.borrow_mut() = key;
            }
            button.setToolTip(Some(&NSString::from_str(&format!(
                "{}\nIzquierdo: panel · Derecho: dashboard · Central: refrescar",
                state.summary()
            ))));
            button.setAccessibilityLabel(Some(&NSString::from_str(&state.summary())));
        }
        if let Some(panel) = iv.panel.get() {
            if iv.popover.get().is_some_and(|p| p.isShown()) {
                panel.setNeedsDisplay(true);
            }
            panel.setAccessibilityLabel(Some(&NSString::from_str(&state.summary())));
            panel.setToolTip(
                state
                    .sample
                    .as_ref()
                    .map(|s| NSString::from_str(&s.model_summary()))
                    .as_deref(),
            );
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
                if let Some(panel) = iv.panel.get() {
                    panel.setNeedsDisplay(true);
                }
                #[allow(deprecated)]
                NSApplication::sharedApplication(self.mtm()).activateIgnoringOtherApps(true);
                p.showRelativeToRect_ofView_preferredEdge(b.bounds(), &b, NSRectEdge::MinY);
                if let Some(window) = p.contentViewController().and_then(|c| c.view().window()) {
                    window.makeKeyWindow();
                }
            }
        }
    }
    fn probe(&self) {
        let iv = self.ivars();
        let Some(settings) = iv.settings.get() else {
            return;
        };
        let c = match settings.probe_config(&iv.state.borrow().config) {
            Ok(c) => c,
            Err(e) => {
                settings.error(&e);
                return;
            }
        };
        if !settings.begin_probe() {
            return;
        }
        let input = c.url.clone();
        // Probes and polling share the worker and its persistent HTTP session.
        if iv.tx.send(WorkerRequest::Probe(c)).is_err() {
            settings.finish_probe(
                &input,
                &Err("El servicio de conexión no está disponible".into()),
            );
        }
    }
    fn probe_result(&self) {
        while let Ok(reply) = self.ivars().probe_rx.try_recv() {
            if let Some(settings) = self.ivars().settings.get() {
                settings.finish_probe(&reply.input, &reply.result);
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
        let state = iv.state.borrow();
        settings.show(&state.config);
        if let Some(sample) = &state.sample {
            settings.update_models(&state.config.url, sample);
        }
    }
    fn save_settings(&self) {
        let iv = self.ivars();
        let Some(settings) = iv.settings.get() else {
            return;
        };
        let edited = settings.read(&iv.state.borrow().config);
        let config = match edited.and_then(|c| {
            let old = iv.state.borrow().config.clone();
            c.save(&iv.config_path)?;
            let desired = settings.login_enabled();
            if (desired != crate::login::enabled() || (desired && crate::login::available()))
                && let Err(e) = crate::login::set_enabled(desired)
            {
                old.save(&iv.config_path).map_err(|rollback| {
                    format!("{e}. No se pudo restaurar la configuración: {rollback}")
                })?;
                return Err(e);
            }
            Ok(c)
        }) {
            Ok(c) => c,
            Err(e) => {
                settings.error(&e);
                return;
            }
        };
        let height = drawing::height(&config);
        {
            let mut state = iv.state.borrow_mut();
            if state.config.url != config.url || state.config.service != config.service {
                state.sample = None;
                state.kind = None;
                state.history.clear();
                state.error = "esperando lectura…".into();
            }
            state.config = config;
            while state.history.len() > state.config.history {
                state.history.pop_front();
            }
        }
        iv.revision.set(iv.revision.get() + 1);
        if let Some(panel) = iv.panel.get() {
            panel.setFrameSize(NSSize::new(drawing::WIDTH, height));
            for button in panel.subviews().iter() {
                let mut r = button.frame();
                r.origin.y = drawing::footer_button_y(height);
                button.setFrame(r);
            }
        }
        if let Some(popover) = iv.popover.get() {
            popover.setContentSize(NSSize::new(drawing::WIDTH, height));
        }
        self.redraw();
        self.refresh();
        settings.close();
    }
    fn update_controls(&self) {
        let iv = self.ivars();
        let prefs = iv.update_prefs.borrow();
        if let Some(button) = iv.update_button.get() {
            button.setTitle(&NSString::from_str(if prefs.available.is_some() {
                "Actualizar"
            } else {
                "Actualizaciones"
            }));
            let detail = prefs
                .available
                .as_ref()
                .map(|r| format!("Versión {} disponible · abrir actualizaciones", r.version))
                .unwrap_or_else(|| "Buscar e instalar nuevas versiones".into());
            button.setToolTip(Some(&NSString::from_str(&detail)));
            button.setAccessibilityLabel(Some(&NSString::from_str(&detail)));
        }
        if let Some(window) = iv.update_window.get() {
            window.refresh(&prefs, iv.update_busy.get(), &iv.update_message.borrow());
        }
    }
    fn open_updates(&self) {
        let iv = self.ivars();
        if let Some(popover) = iv.popover.get() {
            popover.close();
        }
        iv.update_window
            .get_or_init(|| crate::update_window::UpdateWindow::new(self.mtm(), self))
            .show();
        if iv.update_message.borrow().is_empty() {
            *iv.update_message.borrow_mut() = iv.update_prefs.borrow().available.as_ref()
                .map(|r| format!("Versión {} disponible. Pulsa Instalar y reiniciar para actualizar conservando tus ajustes.", r.version))
                .unwrap_or_else(|| "Consulta las versiones publicadas en GitHub. Solo se descarga e instala cuando tú lo indicas.".into());
        }
        self.update_controls();
    }
    fn check_updates(&self, manual: bool) {
        let iv = self.ivars();
        if iv.update_busy.get() || (!manual && !iv.update_prefs.borrow().due()) {
            return;
        }
        iv.update_busy.set(true);
        let mut prefs = iv.update_prefs.borrow_mut();
        prefs.mark_checked();
        let _ = prefs.save();
        drop(prefs);
        *iv.update_message.borrow_mut() = "Buscando una nueva versión…".into();
        if iv.tx.send(WorkerRequest::CheckUpdate).is_err() {
            iv.update_busy.set(false);
            *iv.update_message.borrow_mut() = "El servicio de conexión no está disponible".into();
        }
        self.update_controls();
    }
    fn automatic_updates(&self) {
        let iv = self.ivars();
        if let Some(window) = iv.update_window.get() {
            let mut prefs = iv.update_prefs.borrow_mut();
            let old = prefs.automatic;
            prefs.automatic = window.automatic();
            if let Err(error) = prefs.save() {
                prefs.automatic = old;
                *iv.update_message.borrow_mut() = error;
            }
        }
        self.update_controls();
    }
    fn install_update(&self) {
        let iv = self.ivars();
        if iv.update_busy.get() {
            return;
        }
        let Some(release) = iv.update_prefs.borrow().available.clone() else {
            return;
        };
        match crate::login::current_bundle().and_then(|bundle| {
            iv.tx
                .send(WorkerRequest::InstallUpdate(release, bundle))
                .map_err(|e| e.to_string())
        }) {
            Ok(()) => {
                iv.update_busy.set(true);
                *iv.update_message.borrow_mut() = "Descargando y verificando la actualización… La app se reiniciará cuando esté preparada.".into();
            }
            Err(error) => *iv.update_message.borrow_mut() = error,
        }
        self.update_controls();
    }
    fn update_result(&self) {
        let iv = self.ivars();
        while let Ok(reply) = iv.update_rx.try_recv() {
            iv.update_busy.set(false);
            match reply {
                UpdateReply::Checked(result) => match result {
                    Ok(release) => {
                        *iv.update_message.borrow_mut() = release.as_ref().map(|r| format!("Versión {} disponible. Pulsa Instalar y reiniciar para actualizar conservando tus ajustes.", r.version)).unwrap_or_else(|| format!("Estás al día. Versión instalada: {}.", crate::updates::CURRENT));
                        iv.update_prefs.borrow_mut().available = release;
                        let _ = iv.update_prefs.borrow().save();
                    }
                    Err(error) => {
                        *iv.update_message.borrow_mut() =
                            format!("No se pudo buscar una actualización: {error}")
                    }
                },
                UpdateReply::Prepared(result) => match result
                    .and_then(crate::updates::start_install)
                {
                    Ok(()) => {
                        NSApplication::sharedApplication(self.mtm()).terminate(None);
                        return;
                    }
                    Err(error) => {
                        *iv.update_message.borrow_mut() = format!("No se pudo actualizar: {error}")
                    }
                },
            }
            self.update_controls();
        }
    }
    fn about(&self) {
        if let Some(popover) = self.ivars().popover.get() {
            popover.close();
        }
        let app = NSApplication::sharedApplication(self.mtm());
        let content = "Creado por Alberto Martín\n\nInstagram · @amartinro\nX · @amartinro";
        let credits = NSMutableAttributedString::initWithString(
            NSMutableAttributedString::alloc(),
            &NSString::from_str(content),
        );
        let full = NSRange::new(0, content.encode_utf16().count());
        let paragraph = NSMutableParagraphStyle::new();
        paragraph.setAlignment(NSTextAlignment::Center);
        // Attribute keys receive the documented AppKit value types; ranges use UTF-16.
        unsafe {
            credits.addAttribute_value_range(
                NSFontAttributeName,
                &NSFont::systemFontOfSize(13.),
                full,
            );
            credits.addAttribute_value_range(
                NSForegroundColorAttributeName,
                &NSColor::labelColor(),
                full,
            );
            credits.addAttribute_value_range(NSParagraphStyleAttributeName, &paragraph, full);
            for (label, url) in [
                (
                    "Instagram · @amartinro",
                    "https://www.instagram.com/amartinro/",
                ),
                ("X · @amartinro", "https://x.com/amartinro"),
            ] {
                let start = content.find(label).unwrap();
                let range = NSRange::new(
                    content[..start].encode_utf16().count(),
                    label.encode_utf16().count(),
                );
                credits.addAttribute_value_range(
                    NSLinkAttributeName,
                    &NSURL::URLWithString(&NSString::from_str(url)).unwrap(),
                    range,
                );
            }
            let version = NSString::from_str(env!("CARGO_PKG_VERSION"));
            let options = NSDictionary::from_slices(
                &[
                    NSAboutPanelOptionApplicationName,
                    NSAboutPanelOptionApplicationVersion,
                    NSAboutPanelOptionVersion,
                    NSAboutPanelOptionCredits,
                ],
                &[
                    ns_string!("Kairos Monitor") as &AnyObject,
                    &*version as &AnyObject,
                    ns_string!("") as &AnyObject,
                    &*credits as &AnyObject,
                ],
            );
            app.orderFrontStandardAboutPanelWithOptions(&options);
        }
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }
    fn dashboard(&self) {
        let url = self.ivars().state.borrow().config.dashboard.clone();
        if let Some(url) = NSURL::URLWithString(&NSString::from_str(&url)) {
            NSWorkspace::sharedWorkspace().openURL(&url);
        }
    }
}

/// Shared native controls for the live panel and visual snapshots.
fn footer_buttons(
    panel: &NSView,
    target: Option<&AnyObject>,
    mtm: MainThreadMarker,
    height: f64,
) -> Retained<NSButton> {
    let mut updates = None;
    for (title, symbol, x, width, action, help) in [
        (
            "Refrescar",
            "arrow.clockwise",
            20.,
            116.,
            sel!(refresh:),
            "Consultar las métricas ahora",
        ),
        (
            "Dashboard",
            "arrow.up.right.square",
            144.,
            120.,
            sel!(dashboard:),
            "Abrir el dashboard en el navegador",
        ),
        (
            "Actualizaciones",
            "arrow.down.circle",
            272.,
            174.,
            sel!(updates:),
            "Buscar e instalar nuevas versiones",
        ),
        (
            "Ajustes",
            "gearshape",
            454.,
            100.,
            sel!(settings:),
            "Configurar Kairos Monitor",
        ),
        (
            "Salir",
            "power",
            562.,
            78.,
            sel!(quit:),
            "Cerrar Kairos Monitor",
        ),
    ] {
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(title),
                target,
                Some(action),
                mtm,
            )
        };
        button.setFrame(rect(
            x,
            drawing::footer_button_y(height),
            width,
            drawing::FOOTER_BUTTON_HEIGHT,
        ));
        button.setBezelStyle(NSBezelStyle::Push);
        button.setBordered(true);
        button.setFont(Some(&NSFont::systemFontOfSize(12.)));
        button.setContentTintColor(Some(&NSColor::labelColor()));
        button.setToolTip(Some(&NSString::from_str(help)));
        button.setAccessibilityLabel(Some(&NSString::from_str(title)));
        if let Some(image) = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &NSString::from_str(symbol),
            None,
        ) {
            let size = image.size();
            let scale = 13. / size.width.max(size.height);
            image.setSize(NSSize::new(size.width * scale, size.height * scale));
            button.setImage(Some(&image));
            button.setImagePosition(NSCellImagePosition::ImageLeft);
            button.setImageHugsTitle(true);
        }
        panel.addSubview(&button);
        if title == "Actualizaciones" {
            updates = Some(button);
        }
    }
    updates.unwrap()
}

pub fn run(config: Config, config_path: std::path::PathBuf, show: bool, show_settings: bool) {
    let mtm = MainThreadMarker::new().expect("main thread");
    let application = NSApplication::sharedApplication(mtm);
    application.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let (tx, requests) = mpsc::channel();
    let (results, rx) = mpsc::channel();
    let (probe_tx, probe_rx) = mpsc::channel();
    let (update_tx, update_rx) = mpsc::channel();
    std::thread::spawn(move || {
        let client = model::Client::new();
        while let Ok(request) = requests.recv() {
            let notification = match request {
                WorkerRequest::Metrics(revision, config) => {
                    let result = objc2::rc::autoreleasepool(|_| client.fetch(&config));
                    if results.send((revision, result)).is_err() {
                        break;
                    }
                    "KairosMetricsReady"
                }
                WorkerRequest::Probe(config) => {
                    let result = objc2::rc::autoreleasepool(|_| client.probe(&config));
                    if probe_tx
                        .send(ProbeReply {
                            input: config.url,
                            result,
                        })
                        .is_err()
                    {
                        break;
                    }
                    "KairosProbeReady"
                }
                WorkerRequest::CheckUpdate => {
                    let result = objc2::rc::autoreleasepool(|_| crate::updates::check(&client));
                    if update_tx.send(UpdateReply::Checked(result)).is_err() {
                        break;
                    }
                    "KairosUpdatesReady"
                }
                WorkerRequest::InstallUpdate(release, target) => {
                    let result = objc2::rc::autoreleasepool(|_| {
                        crate::updates::prepare(&client, &release, target)
                    });
                    if update_tx.send(UpdateReply::Prepared(result)).is_err() {
                        break;
                    }
                    "KairosUpdatesReady"
                }
            };
            let notify = block2::RcBlock::new(move || unsafe {
                NSNotificationCenter::defaultCenter()
                    .postNotificationName_object(&NSString::from_str(notification), None);
            });
            // No AppKit objects cross threads; the main queue delivers the notification.
            unsafe {
                NSOperationQueue::mainQueue().addOperationWithBlock(&notify);
            }
        }
    });
    let delegate = App::alloc(mtm).set_ivars(AppIvars {
        state: Rc::new(RefCell::new(State::new(config))),
        status: OnceCell::new(),
        alerts: OnceCell::new(),
        popover: OnceCell::new(),
        panel: OnceCell::new(),
        tx,
        rx,
        revision: Cell::new(0),
        probe_rx,
        update_rx,
        update_prefs: RefCell::new(crate::updates::Preferences::load()),
        update_busy: Cell::new(false),
        update_message: RefCell::new(
            std::fs::read_to_string(Config::path().with_file_name("update-result.txt"))
                .unwrap_or_default(),
        ),
        update_window: OnceCell::new(),
        update_button: OnceCell::new(),
        settings: OnceCell::new(),
        config_path,
        show_settings,
        busy: Cell::new(false),
        timer: RefCell::new(None),
        last_bar: RefCell::new(String::new()),
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
    let height = drawing::height(&state.config);
    let panel = PanelView::new(mtm, Rc::new(RefCell::new(state)));
    panel.setAppearance(
        NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
    );
    footer_buttons(&panel, None, mtm, height);
    let bitmap = panel
        .bitmapImageRepForCachingDisplayInRect(panel.bounds())
        .unwrap();
    panel.cacheDisplayInRect_toBitmapImageRep(panel.bounds(), &bitmap);
    let data = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    assert!(data.writeToFile_atomically(&NSString::from_str(path), true));
}

pub fn snapshot_bar(config: Config, sample: Result<Sample, String>, path: &str, dark: bool) {
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let mut state = State::new(config);
    state.apply(sample);
    let artwork = drawing::bar(&state);
    let bar = &artwork.image;
    bar.setTemplate(false);
    if dark {
        #[allow(deprecated)]
        bar.lockFocusFlipped(true);
        NSColor::whiteColor().setFill();
        NSRectFillUsingOperation(
            rect(0., 0., bar.size().width, bar.size().height),
            NSCompositingOperation::SourceIn,
        );
        #[allow(deprecated)]
        bar.unlockFocus();
    }
    let image = NSImage::initWithSize(
        NSImage::alloc(),
        NSSize::new(bar.size().width + 2. * drawing::BAR_PADDING, 28.),
    );
    #[allow(deprecated)]
    image.lockFocusFlipped(true);
    let background = if dark {
        drawing::color("#24262B", 1.)
    } else {
        NSColor::whiteColor()
    };
    drawing::fill(rect(0., 0., image.size().width, 28.), &background, 0.);
    if let Some(hex) = state.config.bar_background.hex() {
        drawing::fill(
            rect(0., 0., image.size().width, 28.),
            &drawing::color(hex, 0.24),
            6.,
        );
    }
    bar.drawInRect(rect(
        drawing::BAR_PADDING,
        5.,
        bar.size().width,
        bar.size().height,
    ));
    drawing::alert_values(
        &artwork.alerts,
        dark,
        NSPoint::new(drawing::BAR_PADDING, 5.),
    );
    #[allow(deprecated)]
    image.unlockFocus();
    let bitmap = NSBitmapImageRep::imageRepWithData(&image.TIFFRepresentation().unwrap()).unwrap();
    let data = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    assert!(data.writeToFile_atomically(&NSString::from_str(path), true));
}

/// Render the actual status button and alert overlay for native layout/appearance QA.
pub fn snapshot_native_bar(
    config: Config,
    sample: Result<Sample, String>,
    path: &str,
    dark: bool,
    selected: bool,
) {
    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let mut state = State::new(config);
    state.apply(sample);
    let artwork = drawing::bar(&state);
    let status = NSStatusBar::systemStatusBar()
        .statusItemWithLength(artwork.image.size().width + 2. * drawing::BAR_PADDING);
    let button = status.button(mtm).unwrap();
    button.setAppearance(
        NSAppearance::appearanceNamed(unsafe {
            if dark {
                NSAppearanceNameDarkAqua
            } else {
                NSAppearanceNameAqua
            }
        })
        .as_deref(),
    );
    button.setImageScaling(NSImageScaling::ScaleNone);
    button.setImage(Some(&artwork.image));
    unsafe {
        button.setTarget(Some(&app));
        button.setAction(Some(sel!(stop:)));
    }
    apply_bar_background(&button, state.config.bar_background);
    button.highlight(selected);
    let overlay = AlertOverlay::new(mtm);
    button.addSubview(&overlay);
    overlay.update(&button, &artwork);
    *overlay.ivars().values.borrow_mut() = artwork.alerts;
    button.layoutSubtreeIfNeeded();
    for value in overlay.ivars().values.borrow().iter() {
        let point = NSPoint::new(
            overlay.frame().origin.x + value.frame.origin.x + 2.,
            button.bounds().size.height / 2.,
        );
        let hit = button
            .hitTest(point)
            .expect("the status button must remain clickable");
        assert!(
            hit.downcast_ref::<AlertOverlay>().is_none(),
            "color overlay intercepted mouse events"
        );
    }
    let bitmap = button
        .bitmapImageRepForCachingDisplayInRect(button.bounds())
        .unwrap();
    button.cacheDisplayInRect_toBitmapImageRep(button.bounds(), &bitmap);
    let native = NSImage::initWithSize(NSImage::alloc(), button.bounds().size);
    native.addRepresentation(&bitmap);
    let image = NSImage::initWithSize(NSImage::alloc(), button.bounds().size);
    #[allow(deprecated)]
    image.lockFocusFlipped(true);
    let background = if dark {
        drawing::color("#24262B", 1.)
    } else {
        NSColor::whiteColor()
    };
    drawing::fill(button.bounds(), &background, 0.);
    native.drawInRect(button.bounds());
    #[allow(deprecated)]
    image.unlockFocus();
    let bitmap = NSBitmapImageRep::imageRepWithData(&image.TIFFRepresentation().unwrap()).unwrap();
    let data = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    assert!(data.writeToFile_atomically(&NSString::from_str(path), true));
    NSStatusBar::systemStatusBar().removeStatusItem(&status);
}
