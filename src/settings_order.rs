//! Four movable native controls. Changes remain in the form until Save.
use super::palette::Palette;
use crate::{
    config::{BarMetric, Config},
    drawing::rect,
};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::*;
use objc2_foundation::*;
use std::cell::Cell;

pub struct MetricOrderIvars {
    order: Cell<[BarMetric; 4]>,
    checks: Vec<Retained<NSButton>>,
    left: Vec<Retained<NSButton>>,
    right: Vec<Retained<NSButton>>,
    preview: Retained<Palette>,
}
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MetricOrderIvars]
    pub struct MetricOrder;
    unsafe impl NSObjectProtocol for MetricOrder {}
    impl MetricOrder {
        #[unsafe(method(moveLeft:))]
        fn move_left(&self, sender: &NSButton) { self.move_metric(sender.tag() as usize, -1); }
        #[unsafe(method(moveRight:))]
        fn move_right(&self, sender: &NSButton) { self.move_metric(sender.tag() as usize, 1); }
        #[unsafe(method(toggleMetric:))]
        fn toggle_metric(&self, _sender: &NSButton) { self.sync(); }
    }
);
impl MetricOrder {
    pub fn new(
        mtm: MainThreadMarker,
        view: &NSView,
        preview: &Retained<Palette>,
    ) -> Retained<Self> {
        let mut checks = Vec::new();
        let mut left = Vec::new();
        let mut right = Vec::new();
        for metric in BarMetric::ALL {
            let check = super::checkbox(view, mtm, metric.title(), rect(0., 228., 94., 24.));
            check.setTag(metric.index() as isize);
            checks.push(check);
            for (buttons, title, selector, direction) in [
                (&mut left, "‹", sel!(moveLeft:), "a la izquierda"),
                (&mut right, "›", sel!(moveRight:), "a la derecha"),
            ] {
                let button = unsafe {
                    NSButton::buttonWithTitle_target_action(
                        &NSString::from_str(title),
                        None,
                        Some(selector),
                        mtm,
                    )
                };
                button.setBezelStyle(NSBezelStyle::Push);
                button.setFont(Some(&NSFont::systemFontOfSize(14.)));
                button.setTag(metric.index() as isize);
                let help = NSString::from_str(&format!("Mover {} {direction}", metric.title()));
                button.setAccessibilityLabel(Some(&help));
                button.setToolTip(Some(&help));
                view.addSubview(&button);
                buttons.push(button);
            }
        }
        let this = Self::alloc(mtm).set_ivars(MetricOrderIvars {
            order: Cell::new(BarMetric::ALL),
            checks,
            left,
            right,
            preview: preview.clone(),
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        for button in this.ivars().left.iter().chain(&this.ivars().right) {
            unsafe {
                button.setTarget(Some(&this));
            }
        }
        for check in &this.ivars().checks {
            unsafe {
                check.setTarget(Some(&this));
                check.setAction(Some(sel!(toggleMetric:)));
            }
        }
        this.populate(&Config::default());
        this
    }
    fn move_metric(&self, id: usize, delta: isize) {
        let mut order = self.ivars().order.get();
        let Some(position) = order.iter().position(|m| m.index() == id) else {
            return;
        };
        let next = position as isize + delta;
        if !(0..4).contains(&next) {
            return;
        }
        order.swap(position, next as usize);
        self.ivars().order.set(order);
        self.sync();
    }
    fn sync(&self) {
        let iv = self.ivars();
        for (position, metric) in iv.order.get().iter().enumerate() {
            let id = metric.index();
            let x = 20. + position as f64 * 93.;
            iv.checks[id].setFrame(rect(x, 228., 94., 24.));
            iv.left[id].setFrame(rect(x + 12., 204., 28., 24.));
            iv.right[id].setFrame(rect(x + 42., 204., 28., 24.));
            iv.left[id].setEnabled(position > 0);
            iv.right[id].setEnabled(position < 3);
        }
        iv.preview.set_metrics(iv.order.get(), self.enabled());
    }
    pub fn enabled(&self) -> [bool; 4] {
        std::array::from_fn(|i| self.ivars().checks[i].state() == NSControlStateValueOn)
    }
    pub fn order(&self) -> [BarMetric; 4] {
        self.ivars().order.get()
    }
    pub fn populate(&self, c: &Config) {
        self.ivars().order.set(c.bar_order);
        for (check, enabled) in
            self.ivars()
                .checks
                .iter()
                .zip([c.show_gpu, c.show_memory, c.show_tps, c.show_gpt])
        {
            check.setState(if enabled {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        self.sync();
    }
    pub fn verify_actions(&self) {
        let iv = self.ivars();
        let original = self.order();
        let enabled = self.enabled();
        let first = original[0].index();
        assert!(!iv.left[first].isEnabled());
        assert!(!iv.right[original[3].index()].isEnabled());
        unsafe {
            iv.right[first].performClick(None);
        }
        let mut expected = original;
        expected.swap(0, 1);
        assert_eq!(self.order(), expected);
        assert_eq!(self.enabled(), enabled);
        unsafe {
            iv.left[first].performClick(None);
        }
        assert_eq!(self.order(), original);
        unsafe {
            iv.checks[first].performClick(None);
        }
        assert_ne!(self.enabled()[first], enabled[first]);
        assert_eq!(self.order(), original);
        unsafe {
            iv.checks[first].performClick(None);
        }
        assert_eq!(self.enabled(), enabled);
    }
}
