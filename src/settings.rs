use crate::{
    config::Config,
    drawing::rect,
    model::{Sample, ServerKind},
};
use objc2::{MainThreadOnly, rc::Retained, runtime::AnyObject, sel};
use objc2_app_kit::*;
use objc2_foundation::*;
use std::cell::Cell;

pub struct Settings {
    pub window: Retained<NSWindow>,
    fields: Vec<(&'static str, Retained<NSTextField>)>,
    graph: Retained<NSButton>,
    metrics: Vec<Retained<NSButton>>,
    login: Retained<NSButton>,
    error: Retained<NSTextField>,
    models: Retained<NSTextField>,
    test_button: Retained<NSButton>,
    testing: Cell<bool>,
}
fn label(
    view: &NSView,
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    size: f64,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
    label.setFrame(frame);
    label.setFont(Some(&NSFont::systemFontOfSize(size)));
    view.addSubview(&label);
    label
}
impl Settings {
    pub fn new(mtm: MainThreadMarker, target: &AnyObject) -> Self {
        // This controller retains its window and releases it itself, including after closing.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0., 0., 590., 710.),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setTitle(ns_string!("Ajustes de Kairos Monitor"));
        window.setAppearance(
            NSAppearance::appearanceNamed(unsafe { NSAppearanceNameDarkAqua }).as_deref(),
        );
        window.center();
        let view = window.contentView().unwrap();
        view.setWantsLayer(true);
        if let Some(layer) = view.layer() {
            layer.setBackgroundColor(Some(&crate::drawing::color("#202229", 1.).CGColor()));
        }
        label(
            &view,
            mtm,
            "Kairos Monitor",
            rect(22., 665., 530., 28.),
            20.,
        );
        let hint = label(
            &view,
            mtm,
            concat!(
                "Versión ",
                env!("CARGO_PKG_VERSION"),
                " · Los cambios se aplican al guardar."
            ),
            rect(22., 641., 530., 20.),
            12.,
        );
        hint.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let mut fields = Vec::new();
        for (key, title, y) in [
            ("url", "URL del servidor", 606.),
            ("dashboard", "Dashboard", 571.),
        ] {
            label(&view, mtm, title, rect(22., y + 2., 146., 22.), 12.);
            let field = NSTextField::initWithFrame(
                NSTextField::alloc(mtm),
                rect(175., y, if key == "url" { 280. } else { 391. }, 24.),
            );
            field.setFont(Some(&NSFont::systemFontOfSize(12.)));
            field.setAccessibilityLabel(Some(&NSString::from_str(title)));
            view.addSubview(&field);
            fields.push((key, field));
        }
        let test_button = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Probar conexión"),
                Some(target),
                Some(sel!(testConnection:)),
                mtm,
            )
        };
        test_button.setFrame(rect(462., 603., 104., 30.));
        test_button.setFont(Some(&NSFont::systemFontOfSize(11.)));
        view.addSubview(&test_button);
        label(
            &view,
            mtm,
            "Modelos detectados",
            rect(22., 538., 146., 22.),
            12.,
        );
        let models = NSTextField::wrappingLabelWithString(
            ns_string!("Detección automática al conectar."),
            mtm,
        );
        models.setFrame(rect(175., 518., 391., 44.));
        models.setFont(Some(&NSFont::systemFontOfSize(11.)));
        models.setTextColor(Some(&NSColor::secondaryLabelColor()));
        view.addSubview(&models);
        for (title, y) in [("ACTUALIZACIÓN", 498.), ("UMBRALES DE AVISO", 380.)] {
            let title = label(&view, mtm, title, rect(22., y, 540., 18.), 10.);
            title.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        for (key, title, x, y) in [
            ("interval", "Panel cerrado (s)", 22., 462.),
            ("openInterval", "Panel abierto (s)", 312., 462.),
            ("timeout", "Tiempo de espera (s)", 22., 424.),
            ("history", "Lecturas del gráfico", 312., 424.),
            ("memWarn", "RAM aviso (%)", 22., 346.),
            ("memDanger", "RAM crítico (%)", 312., 346.),
            ("gpuWarn", "GPU aviso (%)", 22., 308.),
            ("gpuDanger", "GPU crítico (%)", 312., 308.),
            ("tempWarn", "GPU aviso (°C)", 22., 270.),
            ("tempDanger", "GPU crítico (°C)", 312., 270.),
        ] {
            label(&view, mtm, title, rect(x, y + 2., 165., 22.), 12.);
            let field =
                NSTextField::initWithFrame(NSTextField::alloc(mtm), rect(x + 175., y, 79., 24.));
            field.setFont(Some(&NSFont::monospacedDigitSystemFontOfSize_weight(
                12.,
                unsafe { NSFontWeightRegular },
            )));
            field.setAlignment(NSTextAlignment::Right);
            field.setAccessibilityLabel(Some(&NSString::from_str(title)));
            view.addSubview(&field);
            fields.push((key, field));
        }
        let graph = unsafe {
            NSButton::checkboxWithTitle_target_action(
                ns_string!("Mostrar gráfico de tokens/s"),
                None,
                None,
                mtm,
            )
        };
        graph.setFrame(rect(22., 225., 282., 24.));
        view.addSubview(&graph);
        let heading = label(
            &view,
            mtm,
            "BARRA DE MENÚS",
            rect(22., 190., 540., 18.),
            10.,
        );
        heading.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let mut metrics = Vec::new();
        for (title, x) in [("GPU (%)", 22.), ("RAM (GiB)", 202.), ("Tokens/s", 382.)] {
            let checkbox = unsafe {
                NSButton::checkboxWithTitle_target_action(
                    &NSString::from_str(title),
                    None,
                    None,
                    mtm,
                )
            };
            checkbox.setFrame(rect(x, 159., 170., 24.));
            view.addSubview(&checkbox);
            metrics.push(checkbox);
        }
        let login = unsafe {
            NSButton::checkboxWithTitle_target_action(
                ns_string!("Abrir al iniciar sesión"),
                None,
                None,
                mtm,
            )
        };
        login.setFrame(rect(22., 117., 540., 24.));
        view.addSubview(&login);
        let hint = label(
            &view,
            mtm,
            if crate::login::available() {
                "Se abrirá en la barra de menús, sin ventanas ni icono en el Dock."
            } else {
                "Para activar el inicio de sesión, copia la app a Aplicaciones."
            },
            rect(42., 91., 524., 20.),
            11.,
        );
        hint.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let error = NSTextField::wrappingLabelWithString(ns_string!(""), mtm);
        error.setFrame(rect(22., 53., 544., 32.));
        error.setTextColor(Some(&NSColor::systemRedColor()));
        error.setFont(Some(&NSFont::systemFontOfSize(11.)));
        view.addSubview(&error);
        for (title, x, w, action, key) in [
            ("Restablecer", 22., 116., sel!(resetSettings:), ""),
            ("Cancelar", 358., 96., sel!(cancelSettings:), "\u{1b}"),
            ("Guardar", 466., 100., sel!(saveSettings:), "\r"),
        ] {
            let button = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(title),
                    Some(target),
                    Some(action),
                    mtm,
                )
            };
            button.setFrame(rect(x, 20., w, 30.));
            button.setBezelStyle(NSBezelStyle::Push);
            button.setKeyEquivalent(&NSString::from_str(key));
            view.addSubview(&button);
        }
        for pair in fields.windows(2) {
            unsafe {
                pair[0].1.setNextKeyView(Some(&pair[1].1));
            }
        }
        Self {
            window,
            fields,
            graph,
            metrics,
            login,
            error,
            models,
            test_button,
            testing: Cell::new(false),
        }
    }
    pub fn populate(&self, c: &Config) {
        let value = toml::Value::try_from(c).unwrap();
        for (key, field) in &self.fields {
            let v = &value[*key];
            let text = match v {
                toml::Value::String(s) => s.clone(),
                _ => v.to_string(),
            };
            field.setStringValue(&NSString::from_str(&text));
        }
        self.graph.setState(if c.graph {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        for (checkbox, enabled) in self
            .metrics
            .iter()
            .zip([c.show_gpu, c.show_memory, c.show_tps])
        {
            checkbox.setState(if enabled {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        self.login.setState(if crate::login::enabled() {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.login
            .setEnabled(crate::login::available() || crate::login::enabled());
        self.error.setStringValue(ns_string!(""));
        self.models
            .setStringValue(ns_string!("Detección automática al conectar."));
        self.models.setToolTip(None);
    }
    pub fn show(&self, c: &Config) {
        self.populate(c);
        self.window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(self.window.mtm()).activateIgnoringOtherApps(true);
        self.window.makeFirstResponder(Some(&self.fields[0].1));
    }
    pub fn read(&self, base: &Config) -> Result<Config, String> {
        let values = self
            .fields
            .iter()
            .map(|(key, f)| (key.to_string(), f.stringValue().to_string()))
            .collect::<Vec<_>>();
        let mut base = base.clone();
        base.service = "auto".into();
        base.show_gpu = self.metrics[0].state() == NSControlStateValueOn;
        base.show_memory = self.metrics[1].state() == NSControlStateValueOn;
        base.show_tps = self.metrics[2].state() == NSControlStateValueOn;
        base.edited(
            &values,
            self.graph.state() == NSControlStateValueOn,
            base.border,
        )
    }
    pub fn current_url(&self) -> String {
        self.fields
            .iter()
            .find(|(key, _)| *key == "url")
            .unwrap()
            .1
            .stringValue()
            .to_string()
            .trim()
            .to_owned()
    }
    pub fn probe_config(&self, base: &Config) -> Result<Config, String> {
        let inputs = self
            .fields
            .iter()
            .filter(|(key, _)| matches!(*key, "url" | "timeout"))
            .map(|(key, field)| (key.to_string(), field.stringValue().to_string()))
            .collect::<Vec<_>>();
        base.edited(&inputs, base.graph, base.border)
    }
    pub fn begin_probe(&self) -> bool {
        if self.testing.replace(true) {
            return false;
        }
        self.test_button.setEnabled(false);
        self.error
            .setTextColor(Some(&NSColor::secondaryLabelColor()));
        self.error.setToolTip(None);
        self.error
            .setStringValue(ns_string!("Probando conexión y detectando modelos…"));
        true
    }
    pub fn update_models(&self, input: &str, sample: &Sample) {
        if self.current_url() != input {
            return;
        }
        let models = sample.model_summary();
        self.models.setStringValue(&NSString::from_str(&models));
        self.models.setToolTip(Some(&NSString::from_str(&models)));
    }
    pub fn finish_probe(&self, input: &str, result: &Result<Sample, String>) {
        self.testing.set(false);
        self.test_button.setEnabled(true);
        if self.current_url() != input {
            self.error("La URL ha cambiado. Pulsa Probar conexión para verificarla.");
            return;
        }
        match result {
            Ok(sample) => {
                self.update_models(input, sample);
                self.error.setTextColor(Some(&NSColor::systemGreenColor()));
                let message = match sample.kind {
                    ServerKind::Kairos => format!(
                        "Conexión correcta · Kairos · Modelos activos: {}",
                        sample.models.len()
                    ),
                    ServerKind::OpenAI => format!(
                        "Conexión correcta · OpenAI compatible · {} modelos. Sin telemetría de hardware.",
                        sample.models.len()
                    ),
                };
                self.error.setStringValue(&NSString::from_str(&message));
                self.error
                    .setToolTip(Some(&NSString::from_str(&sample.endpoint)));
            }
            Err(e) => {
                self.models
                    .setStringValue(ns_string!("No se pudo detectar el catálogo de modelos."));
                self.models.setToolTip(None);
                self.error(e);
            }
        }
    }
    pub fn login_enabled(&self) -> bool {
        self.login.state() == NSControlStateValueOn
    }
    pub fn error(&self, message: &str) {
        self.error.setTextColor(Some(&NSColor::systemRedColor()));
        self.error.setToolTip(Some(&NSString::from_str(message)));
        self.error.setStringValue(&NSString::from_str(message));
    }
    pub fn close(&self) {
        self.window.orderOut(None);
    }
}

pub fn snapshot(config: &Config, path: &str) {
    let mtm = MainThreadMarker::new().unwrap();
    let _app = NSApplication::sharedApplication(mtm);
    let target = NSObject::new();
    let settings = Settings::new(mtm, &target);
    settings.populate(config);
    let view = settings.window.contentView().unwrap();
    #[allow(deprecated)]
    unsafe {
        NSAppearance::setCurrentAppearance(Some(&view.effectiveAppearance()));
    }
    view.layoutSubtreeIfNeeded();
    let bitmap = view
        .bitmapImageRepForCachingDisplayInRect(view.bounds())
        .unwrap();
    view.cacheDisplayInRect_toBitmapImageRep(view.bounds(), &bitmap);
    let data = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .unwrap();
    assert!(data.writeToFile_atomically(&NSString::from_str(path), true));
    #[allow(deprecated)]
    unsafe {
        NSAppearance::setCurrentAppearance(None);
    }
}
