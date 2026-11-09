use crate::{
    config::Config,
    drawing::rect,
    model::{Sample, ServerKind},
};
use objc2::{MainThreadOnly, rc::Retained, runtime::AnyObject, sel};
use objc2_app_kit::*;
use objc2_foundation::*;
use std::cell::Cell;

#[path = "settings_order.rs"]
mod order;
#[path = "settings_palette.rs"]
mod palette;

pub struct Settings {
    pub window: Retained<NSWindow>,
    fields: Vec<(&'static str, Retained<NSTextField>)>,
    graph: Retained<NSButton>,
    background: Retained<palette::Palette>,
    metrics: Retained<order::MetricOrder>,
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
fn surface(view: &NSView, hex: &str, radius: f64) {
    view.setWantsLayer(true);
    if let Some(layer) = view.layer() {
        layer.setBackgroundColor(Some(&crate::drawing::color(hex, 1.).CGColor()));
        layer.setCornerRadius(radius);
    }
}
fn secondary(view: &NSView, mtm: MainThreadMarker, title: &str, frame: NSRect) {
    label(view, mtm, title, frame, 11.).setTextColor(Some(&NSColor::secondaryLabelColor()));
}
fn card(
    parent: &NSView,
    mtm: MainThreadMarker,
    frame: NSRect,
    title: &str,
    hint: &str,
) -> Retained<NSView> {
    let view = NSView::initWithFrame(NSView::alloc(mtm), frame);
    surface(&view, "#25282F", 12.);
    let heading = label(
        &view,
        mtm,
        title,
        rect(20., frame.size.height - 36., frame.size.width - 40., 22.),
        15.,
    );
    heading.setFont(Some(&NSFont::systemFontOfSize_weight(15., unsafe {
        NSFontWeightSemibold
    })));
    if !hint.is_empty() {
        secondary(
            &view,
            mtm,
            hint,
            rect(20., frame.size.height - 59., frame.size.width - 40., 18.),
        );
    }
    parent.addSubview(&view);
    view
}
fn action(
    view: &NSView,
    mtm: MainThreadMarker,
    target: &AnyObject,
    title: &str,
    frame: NSRect,
    selector: objc2::runtime::Sel,
) -> Retained<NSButton> {
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(selector),
            mtm,
        )
    };
    button.setFrame(frame);
    button.setBezelStyle(NSBezelStyle::Push);
    button.setFont(Some(&NSFont::systemFontOfSize(12.)));
    view.addSubview(&button);
    button
}
fn checkbox(
    view: &NSView,
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
) -> Retained<NSButton> {
    let button = unsafe {
        NSButton::checkboxWithTitle_target_action(&NSString::from_str(title), None, None, mtm)
    };
    button.setFrame(frame);
    button.setFont(Some(&NSFont::systemFontOfSize(12.)));
    view.addSubview(&button);
    button
}
fn input(
    view: &NSView,
    mtm: MainThreadMarker,
    title: &str,
    frame: NSRect,
    numeric: bool,
) -> Retained<NSTextField> {
    let field = NSTextField::initWithFrame(NSTextField::alloc(mtm), frame);
    field.setFont(Some(&NSFont::systemFontOfSize(12.)));
    if numeric {
        field.setAlignment(NSTextAlignment::Right);
    }
    field.setAccessibilityLabel(Some(&NSString::from_str(title)));
    view.addSubview(&field);
    field
}

impl Settings {
    pub fn new(mtm: MainThreadMarker, target: &AnyObject) -> Self {
        // This controller owns the window, even after closing it.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0., 0., 880., 716.),
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
        surface(&view, "#1B1D22", 0.);
        let title = label(&view, mtm, "Ajustes", rect(28., 660., 300., 34.), 26.);
        title.setFont(Some(&NSFont::systemFontOfSize_weight(26., unsafe {
            NSFontWeightSemibold
        })));
        secondary(
            &view,
            mtm,
            "Personaliza cómo se ve y se actualiza Kairos Monitor.",
            rect(28., 636., 550., 20.),
        );
        action(
            &view,
            mtm,
            target,
            "Acerca de…",
            rect(596., 664., 108., 28.),
            sel!(about:),
        );
        action(
            &view,
            mtm,
            target,
            "Actualizaciones…",
            rect(714., 664., 142., 28.),
            sel!(updates:),
        );

        let connection = card(
            &view,
            mtm,
            rect(24., 374., 404., 244.),
            "Conexión",
            "Tu servidor y sus modelos, detectados automáticamente.",
        );
        let mut fields = Vec::new();
        for (key, title, y, width) in [
            ("url", "URL del servidor", 134., 364.),
            ("dashboard", "Dashboard", 76., 364.),
        ] {
            label(&connection, mtm, title, rect(20., y + 28., 364., 18.), 11.);
            fields.push((
                key,
                input(&connection, mtm, title, rect(20., y, width, 26.), false),
            ));
        }
        let test_button = action(
            &connection,
            mtm,
            target,
            "Probar conexión",
            rect(20., 38., 132., 30.),
            sel!(testConnection:),
        );
        let models = NSTextField::wrappingLabelWithString(
            ns_string!("Detección automática al conectar."),
            mtm,
        );
        models.setFrame(rect(166., 10., 218., 36.));
        models.setFont(Some(&NSFont::systemFontOfSize(11.)));
        models.setTextColor(Some(&NSColor::secondaryLabelColor()));
        connection.addSubview(&models);
        secondary(&connection, mtm, "Modelos", rect(166., 48., 218., 18.));

        let refresh = card(&view, mtm, rect(24., 202., 404., 156.), "Actualización", "");
        for (key, title, x, y) in [
            ("interval", "Panel cerrado · s", 20., 70.),
            ("openInterval", "Panel abierto · s", 212., 70.),
            ("timeout", "Espera máxima · s", 20., 18.),
            ("history", "Lecturas del gráfico", 212., 18.),
        ] {
            label(&refresh, mtm, title, rect(x, y + 27., 174., 16.), 11.);
            fields.push((
                key,
                input(&refresh, mtm, title, rect(x, y, 172., 24.), true),
            ));
        }
        let startup = card(
            &view,
            mtm,
            rect(24., 82., 404., 104.),
            "Al iniciar el Mac",
            "",
        );
        let login = checkbox(
            &startup,
            mtm,
            "Abrir al iniciar sesión",
            rect(20., 40., 364., 24.),
        );
        secondary(
            &startup,
            mtm,
            if crate::login::available() {
                "En la barra de menús, sin abrir ventanas."
            } else {
                "Disponible al instalar la app en Aplicaciones."
            },
            rect(20., 14., 364., 20.),
        );

        let appearance = card(
            &view,
            mtm,
            rect(444., 304., 412., 314.),
            "Apariencia",
            "Orden de izquierda a derecha · usa las flechas.",
        );
        label(
            &appearance,
            mtm,
            "Fondo de color",
            rect(20., 180., 364., 20.),
            12.,
        );
        let background = palette::Palette::new(mtm, &appearance);
        let metrics = order::MetricOrder::new(mtm, &appearance, &background);
        secondary(&appearance, mtm, "Claro", rect(20., 96., 174., 16.));
        secondary(&appearance, mtm, "Oscuro", rect(206., 96., 174., 16.));
        secondary(
            &appearance,
            mtm,
            "Vista previa · valores de ejemplo",
            rect(20., 32., 364., 18.),
        );
        let graph = checkbox(
            &appearance,
            mtm,
            "Mostrar gráfico de tokens/s en el panel",
            rect(20., 8., 370., 24.),
        );

        let alerts = card(
            &view,
            mtm,
            rect(444., 82., 412., 206.),
            "Avisos",
            "Solo cambia de color la cifra afectada.",
        );
        secondary(&alerts, mtm, "Métrica", rect(20., 118., 156., 18.));
        let warn = label(&alerts, mtm, "Aviso", rect(196., 118., 84., 18.), 11.);
        warn.setTextColor(Some(&crate::drawing::soft_alert_color(
            crate::model::AlertLevel::Warning,
            true,
        )));
        let critical = label(&alerts, mtm, "Crítico", rect(300., 118., 84., 18.), 11.);
        critical.setTextColor(Some(&crate::drawing::soft_alert_color(
            crate::model::AlertLevel::Critical,
            true,
        )));
        for (title, warn, danger, y) in [
            ("Uso de GPU · %", "gpuWarn", "gpuDanger", 82.),
            ("Memoria RAM · %", "memWarn", "memDanger", 48.),
            ("Temperatura GPU · °C", "tempWarn", "tempDanger", 14.),
        ] {
            label(&alerts, mtm, title, rect(20., y + 3., 172., 20.), 12.);
            for (key, suffix, x) in [(warn, "aviso", 196.), (danger, "crítico", 300.)] {
                fields.push((
                    key,
                    input(
                        &alerts,
                        mtm,
                        &format!("{title}: {suffix}"),
                        rect(x, y, 84., 26.),
                        true,
                    ),
                ));
            }
        }
        let error = NSTextField::wrappingLabelWithString(ns_string!(""), mtm);
        error.setFrame(rect(170., 39., 420., 36.));
        error.setTextColor(Some(&NSColor::systemRedColor()));
        error.setFont(Some(&NSFont::systemFontOfSize(11.)));
        view.addSubview(&error);
        secondary(
            &view,
            mtm,
            concat!(
                "Kairos Monitor ",
                env!("CARGO_PKG_VERSION"),
                " · Se aplica al guardar"
            ),
            rect(170., 14., 420., 18.),
        );
        for (title, x, w, selector, key) in [
            ("Restablecer", 24., 124., sel!(resetSettings:), ""),
            ("Cancelar", 634., 100., sel!(cancelSettings:), "\u{1b}"),
            ("Guardar", 746., 110., sel!(saveSettings:), "\r"),
        ] {
            let button = action(&view, mtm, target, title, rect(x, 24., w, 32.), selector);
            button.setKeyEquivalent(&NSString::from_str(key));
        }
        Self {
            window,
            fields,
            graph,
            background,
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
        self.background.select(c.bar_background);
        self.graph.setState(if c.graph {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.metrics.populate(c);
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
        base.bar_background = self.background.selected();
        [
            base.show_gpu,
            base.show_memory,
            base.show_tps,
            base.show_gpt,
        ] = self.metrics.enabled();
        base.bar_order = self.metrics.order();
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
    settings.background.verify_actions();
    settings.metrics.verify_actions();
    assert_eq!(settings.read(config).unwrap().bar_order, config.bar_order);
    let edited = settings
        .read(config)
        .expect("The populated form must remain valid");
    assert_eq!(edited.bar_background, crate::config::BarBackground::Gray);
    settings.populate(config);
    assert_eq!(
        settings.read(config).unwrap().bar_background,
        config.bar_background
    );
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
