use crate::{config::Config, drawing::rect};
use objc2::{MainThreadOnly, rc::Retained, runtime::AnyObject, sel};
use objc2_app_kit::*;
use objc2_foundation::*;

pub struct Settings {
    pub window: Retained<NSWindow>,
    fields: Vec<(&'static str, Retained<NSTextField>)>,
    graph: Retained<NSButton>,
    border: Retained<NSButton>,
    error: Retained<NSTextField>,
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
                rect(0., 0., 590., 574.),
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
            rect(22., 529., 530., 28.),
            20.,
        );
        let hint = label(
            &view,
            mtm,
            "Los cambios se guardan y se aplican al momento.",
            rect(22., 505., 530., 20.),
            12.,
        );
        hint.setTextColor(Some(&NSColor::secondaryLabelColor()));
        let mut fields = Vec::new();
        for (key, title, y) in [
            ("url", "Endpoint de métricas", 470.),
            ("dashboard", "Dashboard", 435.),
            ("service", "Servicio", 400.),
        ] {
            label(&view, mtm, title, rect(22., y + 2., 146., 22.), 12.);
            let field =
                NSTextField::initWithFrame(NSTextField::alloc(mtm), rect(175., y, 391., 24.));
            field.setFont(Some(&NSFont::systemFontOfSize(12.)));
            field.setAccessibilityLabel(Some(&NSString::from_str(title)));
            view.addSubview(&field);
            fields.push((key, field));
        }
        for (title, y) in [("ACTUALIZACIÓN", 362.), ("UMBRALES DE AVISO", 244.)] {
            let title = label(&view, mtm, title, rect(22., y, 540., 18.), 10.);
            title.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        for (key, title, x, y) in [
            ("interval", "Panel cerrado (s)", 22., 326.),
            ("openInterval", "Panel abierto (s)", 312., 326.),
            ("timeout", "Tiempo de espera (s)", 22., 288.),
            ("history", "Lecturas del gráfico", 312., 288.),
            ("memWarn", "RAM aviso (%)", 22., 210.),
            ("memDanger", "RAM crítico (%)", 312., 210.),
            ("tempWarn", "GPU aviso (°C)", 22., 172.),
            ("tempDanger", "GPU crítico (°C)", 312., 172.),
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
        graph.setFrame(rect(22., 121., 282., 24.));
        view.addSubview(&graph);
        let border = unsafe {
            NSButton::checkboxWithTitle_target_action(
                ns_string!("Borde de la píldora"),
                None,
                None,
                mtm,
            )
        };
        border.setFrame(rect(312., 121., 254., 24.));
        view.addSubview(&border);
        let error = NSTextField::wrappingLabelWithString(ns_string!(""), mtm);
        error.setFrame(rect(22., 62., 544., 46.));
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
            border,
            error,
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
        self.border.setState(if c.border {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
        self.error.setStringValue(ns_string!(""));
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
        base.edited(
            &values,
            self.graph.state() == NSControlStateValueOn,
            self.border.state() == NSControlStateValueOn,
        )
    }
    pub fn error(&self, message: &str) {
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
