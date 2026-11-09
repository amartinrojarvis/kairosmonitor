//! Native palette buttons and an event-driven preview; no timers or image assets.
use crate::{
    config::{BarBackground, BarMetric},
    drawing::{self, rect},
};
use objc2::{AnyThread, DefinedClass, MainThreadOnly, define_class, msg_send, rc::Retained, sel};
use objc2_app_kit::*;
use objc2_foundation::*;
use std::cell::Cell;

pub struct PreviewIvars {
    selected: Cell<BarBackground>,
    order: Cell<[BarMetric; 4]>,
    enabled: Cell<[bool; 4]>,
}
define_class!(
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PreviewIvars]
    pub struct PalettePreview;
    unsafe impl NSObjectProtocol for PalettePreview {}
    impl PalettePreview {
        #[unsafe(method(drawRect:))]
        fn draw(&self, _dirty: NSRect) {
            for (x, dark) in [(0., false), (186., true)] {
                let bounds = rect(x, 8., 174., 32.);
                drawing::fill(bounds, &drawing::color(if dark { "#202229" } else { "#F2F3F5" }, 1.), 7.);
                if let Some(hex) = self.ivars().selected.get().hex() {
                    drawing::fill(bounds, &drawing::color(hex, 0.24), 7.);
                }
                let values = ["42%", "24G", "128/s", "✦38%"];
                let example = self.ivars().order.get().iter().filter(|m| self.ivars().enabled.get()[m.index()])
                    .map(|m| values[m.index()]).collect::<Vec<_>>().join("  ");
                drawing::text(&example, rect(x + 8., 13., 158., 20.), 10.,
                    &drawing::color(if dark { "#F2F3F7" } else { "#272A30" }, 1.));
            }
        }
    }
);
impl PalettePreview {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PreviewIvars {
            selected: Cell::new(BarBackground::None),
            order: Cell::new(BarMetric::ALL),
            enabled: Cell::new([true; 4]),
        });
        let this: Retained<Self> =
            unsafe { msg_send![super(this), initWithFrame: rect(0., 0., 360., 48.)] };
        this.setAccessibilityElement(true);
        this.setAccessibilityLabel(Some(ns_string!(
            "Vista previa del fondo en apariencia clara y oscura. Valores de ejemplo."
        )));
        this
    }
}
pub struct PaletteIvars {
    buttons: Vec<Retained<NSButton>>,
    swatches: Vec<(Retained<NSImage>, Retained<NSImage>)>,
    preview: Retained<PalettePreview>,
}
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = PaletteIvars]
    pub struct Palette;
    unsafe impl NSObjectProtocol for Palette {}
    impl Palette {
        #[unsafe(method(selectColor:))]
        fn select_color(&self, sender: &NSButton) {
            if let Some((value, _)) = BarBackground::OPTIONS.get(sender.tag() as usize) {
                self.select(*value);
            }
        }
    }
);
fn swatch(value: BarBackground, selected: bool) -> Retained<NSImage> {
    let image = NSImage::initWithSize(NSImage::alloc(), NSSize::new(40., 40.));
    #[allow(deprecated)]
    image.lockFocusFlipped(false);
    if selected {
        drawing::outline(rect(2., 2., 36., 36.), &NSColor::controlAccentColor(), 18.);
    }
    let circle = rect(7., 7., 26., 26.);
    drawing::fill(
        circle,
        &drawing::color(value.hex().unwrap_or("#363A42"), 1.),
        13.,
    );
    if value == BarBackground::None {
        let line = NSBezierPath::bezierPath();
        line.moveToPoint(NSPoint::new(11., 11.));
        line.lineToPoint(NSPoint::new(29., 29.));
        line.setLineWidth(1.5);
        NSColor::secondaryLabelColor().setStroke();
        line.stroke();
    }
    #[allow(deprecated)]
    image.unlockFocus();
    image
}
impl Palette {
    pub fn new(mtm: MainThreadMarker, view: &NSView) -> Retained<Self> {
        let preview = PalettePreview::new(mtm);
        preview.setFrameOrigin(NSPoint::new(20., 52.));
        view.addSubview(&preview);
        let mut buttons = Vec::new();
        let mut swatches = Vec::new();
        for (index, ((value, name), short)) in BarBackground::OPTIONS
            .into_iter()
            .zip([
                "Sin color",
                "Azul",
                "Salvia",
                "Lavanda",
                "Arena",
                "Rosa",
                "Gris",
            ])
            .enumerate()
        {
            let button = unsafe {
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str(short),
                    None,
                    Some(sel!(selectColor:)),
                    mtm,
                )
            };
            button.setFrame(rect(17. + index as f64 * 52., 112., 52., 64.));
            button.setButtonType(NSButtonType::PushOnPushOff);
            button.setBordered(false);
            button.setFont(Some(&NSFont::systemFontOfSize(10.)));
            button.setImagePosition(NSCellImagePosition::ImageAbove);
            swatches.push((swatch(value, false), swatch(value, true)));
            button.setTag(index as isize);
            button.setAccessibilityLabel(Some(&NSString::from_str(&format!(
                "Fondo de color: {name}"
            ))));
            button.setToolTip(Some(&NSString::from_str(name)));
            view.addSubview(&button);
            buttons.push(button);
        }
        let this = Self::alloc(mtm).set_ivars(PaletteIvars {
            buttons,
            preview,
            swatches,
        });
        let this: Retained<Self> = unsafe { msg_send![super(this), init] };
        for button in &this.ivars().buttons {
            unsafe {
                button.setTarget(Some(&this));
            }
        }
        this.select(BarBackground::None);
        this
    }
    pub fn set_metrics(&self, order: [BarMetric; 4], enabled: [bool; 4]) {
        let preview = &self.ivars().preview;
        preview.ivars().order.set(order);
        preview.ivars().enabled.set(enabled);
        preview.setNeedsDisplay(true);
    }
    /// Exercise the same target/action path as mouse and keyboard input in snapshot QA.
    pub fn verify_actions(&self) {
        for (button, (expected, _)) in self.ivars().buttons.iter().zip(BarBackground::OPTIONS) {
            // SAFETY: the retained palette is the target and selectColor: accepts this button.
            unsafe {
                button.performClick(None);
            }
            assert_eq!(self.selected(), expected);
            // SAFETY: the retained palette is the target and selectColor: accepts this button.
            unsafe {
                button.performClick(None);
            }
            assert_eq!(
                self.selected(),
                expected,
                "Clicking the selected color must keep it selected"
            );
            assert_eq!(
                self.ivars()
                    .buttons
                    .iter()
                    .filter(|b| b.state() == NSControlStateValueOn)
                    .count(),
                1
            );
        }
    }
    pub fn selected(&self) -> BarBackground {
        self.ivars().preview.ivars().selected.get()
    }
    pub fn select(&self, value: BarBackground) {
        for ((button, images), (color, _)) in self
            .ivars()
            .buttons
            .iter()
            .zip(&self.ivars().swatches)
            .zip(BarBackground::OPTIONS)
        {
            let image = if color == value { &images.1 } else { &images.0 };
            button.setImage(Some(image));
            button.setAlternateImage(Some(image));
            button.setState(if color == value {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
        self.ivars().preview.ivars().selected.set(value);
        self.ivars().preview.setNeedsDisplay(true);
    }
}
