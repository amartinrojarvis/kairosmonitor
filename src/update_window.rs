use crate::{
    drawing::rect,
    updates::{CURRENT, Preferences},
};
use objc2::{MainThreadOnly, rc::Retained, runtime::AnyObject, sel};
use objc2_app_kit::*;
use objc2_foundation::*;

pub struct UpdateWindow {
    window: Retained<NSWindow>,
    message: Retained<NSTextField>,
    check: Retained<NSButton>,
    install: Retained<NSButton>,
    automatic: Retained<NSButton>,
}
impl UpdateWindow {
    pub fn new(mtm: MainThreadMarker, target: &AnyObject) -> Self {
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0., 0., 500., 280.),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        unsafe {
            window.setReleasedWhenClosed(false);
        }
        window.setTitle(ns_string!("Actualizaciones de Kairos Monitor"));
        window.center();
        let view = window.contentView().unwrap();
        let title = NSTextField::labelWithString(ns_string!("Kairos Monitor"), mtm);
        title.setFont(Some(&NSFont::boldSystemFontOfSize(20.)));
        title.setFrame(rect(22., 229., 456., 28.));
        view.addSubview(&title);
        let version = NSTextField::labelWithString(
            &NSString::from_str(&format!("Versión instalada: {CURRENT} · Apple Silicon")),
            mtm,
        );
        version.setTextColor(Some(&NSColor::secondaryLabelColor()));
        version.setFrame(rect(22., 200., 456., 22.));
        view.addSubview(&version);
        let message = NSTextField::wrappingLabelWithString(ns_string!(""), mtm);
        message.setFrame(rect(22., 110., 456., 76.));
        message.setFont(Some(&NSFont::systemFontOfSize(12.)));
        view.addSubview(&message);
        let automatic = unsafe {
            NSButton::checkboxWithTitle_target_action(
                ns_string!("Buscar nuevas versiones una vez al día"),
                Some(target),
                Some(sel!(automaticUpdates:)),
                mtm,
            )
        };
        automatic.setFrame(rect(22., 72., 456., 24.));
        view.addSubview(&automatic);
        let check = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Buscar actualizaciones"),
                Some(target),
                Some(sel!(checkUpdates:)),
                mtm,
            )
        };
        check.setFrame(rect(16., 20., 206., 32.));
        check.setBezelStyle(NSBezelStyle::Push);
        view.addSubview(&check);
        let install = unsafe {
            NSButton::buttonWithTitle_target_action(
                ns_string!("Instalar y reiniciar"),
                Some(target),
                Some(sel!(installUpdate:)),
                mtm,
            )
        };
        install.setFrame(rect(284., 20., 200., 32.));
        install.setBezelStyle(NSBezelStyle::Push);
        view.addSubview(&install);
        Self {
            window,
            message,
            check,
            install,
            automatic,
        }
    }
    pub fn show(&self) {
        self.window.makeKeyAndOrderFront(None);
        #[allow(deprecated)]
        NSApplication::sharedApplication(self.window.mtm()).activateIgnoringOtherApps(true);
    }
    pub fn automatic(&self) -> bool {
        self.automatic.state() == NSControlStateValueOn
    }
    pub fn refresh(&self, prefs: &Preferences, busy: bool, message: &str) {
        self.message.setStringValue(&NSString::from_str(message));
        self.message.setToolTip(Some(&NSString::from_str(message)));
        self.check.setEnabled(!busy);
        self.install
            .setEnabled(!busy && prefs.available.is_some() && crate::login::available());
        self.automatic.setEnabled(!busy);
        self.automatic.setState(if prefs.automatic {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }
}
