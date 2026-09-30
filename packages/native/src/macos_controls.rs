//! AppKit controls attached to the live GPUI native view.

use std::{cell::RefCell, collections::HashMap, ptr::NonNull, rc::Rc, sync::Arc};

use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyObject, NSObjectProtocol, Sel},
    DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_app_kit::{NSBezelStyle, NSButton, NSCellImagePosition, NSImage, NSView};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

use crate::{element_tree::EventPayload, renderer::EventCallback};

#[derive(Clone, Copy)]
pub(crate) struct ControlBounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

struct ActionIvars {
    element_id: u64,
    callback: EventCallback,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "GpuixGlassButtonActionTarget"]
    #[ivars = ActionIvars]
    struct GlassButtonActionTarget;

    impl GlassButtonActionTarget {
        #[unsafe(method(activate:))]
        fn activate(&self, _sender: &AnyObject) {
            let mut payload = EventPayload {
                element_id: self.ivars().element_id as f64,
                event_type: "click".into(),
                ..Default::default()
            };
            // This is the same renderer callback that dispatches GPUIX events
            // through React's per-root handler registry.
            (self.ivars().callback)(std::mem::take(&mut payload));
        }
    }
);

unsafe impl NSObjectProtocol for GlassButtonActionTarget {}

impl GlassButtonActionTarget {
    fn new(element_id: u64, callback: EventCallback, mtm: MainThreadMarker) -> Retained<Self> {
        let allocated = Self::alloc(mtm).set_ivars(ActionIvars {
            element_id,
            callback,
        });
        // SAFETY: this is the normal NSObject init after setting Rust ivars.
        unsafe { objc2::msg_send![super(allocated), init] }
    }
}

struct NativeButton {
    view: Retained<NSButton>,
    _target: Retained<GlassButtonActionTarget>,
}

pub(crate) struct NativeControlRegistry {
    parent: Option<NonNull<NSView>>,
    controls: HashMap<u64, NativeButton>,
    mtm: MainThreadMarker,
}

impl NativeControlRegistry {
    pub(crate) fn new() -> Self {
        Self {
            parent: None,
            controls: HashMap::new(),
            mtm: MainThreadMarker::new().expect("GPUI macOS rendering runs on the AppKit thread"),
        }
    }

    fn attach_to_window(&mut self, window: &gpui::Window) -> Option<NonNull<NSView>> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return None;
        };
        let pointer = handle.ns_view.cast::<NSView>();
        self.parent = Some(pointer);
        Some(pointer)
    }

    pub(crate) fn create_icon_button(
        &mut self,
        id: u64,
        window: &gpui::Window,
        icon: &str,
        label: Option<&str>,
        disabled: bool,
        callback: Option<EventCallback>,
    ) {
        let Some(parent) = self.attach_to_window(window) else {
            return;
        };
        if self.controls.contains_key(&id) {
            self.update_properties(id, icon, label, disabled);
            return;
        }
        let symbol = NSString::from_str(icon);
        let description = label.map(NSString::from_str);
        // SAFETY: main-thread AppKit API. Unknown symbols return None.
        let image = {
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &symbol,
                description.as_deref(),
            )
        };
        let Some(image) = image else { return };

        let target = GlassButtonActionTarget::new(
            id,
            callback.unwrap_or_else(|| Arc::new(|_| {})),
            self.mtm,
        );
        let action = Some(Sel::register(c"activate:"));
        // SAFETY: button factory is AppKit's main-thread constructor.
        let button = unsafe {
            NSButton::buttonWithImage_target_action(
                &image,
                Some(target.as_ref() as &AnyObject),
                action,
                self.mtm,
            )
        };
        let parent = unsafe { parent.as_ref() };
        button.setTitle(&NSString::from_str(""));
        button.setImagePosition(NSCellImagePosition::ImageOnly);
        button.setEnabled(!disabled);
        unsafe {
            let _: () = msg_send![&*button, setAccessibilityLabel: description.as_deref()];
        }
        // NSBezelStyleGlass is value 16 in the AppKit SDK and available from
        // macOS 26. Older systems keep a native rounded AppKit bezel.
        let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
        if version.majorVersion >= 26 {
            button.setBezelStyle(NSBezelStyle(16));
        } else {
            button.setBezelStyle(NSBezelStyle(1));
        }
        // addSubview places it above GPUI's renderer view content.
        parent.addSubview(&button);
        button.setHidden(true);
        self.controls.insert(
            id,
            NativeButton {
                view: button,
                _target: target,
            },
        );
    }

    pub(crate) fn update_properties(
        &mut self,
        id: u64,
        icon: &str,
        label: Option<&str>,
        disabled: bool,
    ) {
        let Some(control) = self.controls.get(&id) else {
            return;
        };
        let symbol = NSString::from_str(icon);
        let description = label.map(NSString::from_str);
        let image = {
            NSImage::imageWithSystemSymbolName_accessibilityDescription(
                &symbol,
                description.as_deref(),
            )
        };
        control.view.setImage(image.as_deref());
        control.view.setEnabled(!disabled);
        unsafe {
            let _: () = msg_send![&*control.view, setAccessibilityLabel: description.as_deref()];
        }
    }

    pub(crate) fn update_bounds(&mut self, id: u64, bounds: ControlBounds) {
        let (Some(parent), Some(control)) = (self.parent, self.controls.get(&id)) else {
            return;
        };
        let parent = unsafe { parent.as_ref() };
        let parent_bounds = parent.bounds();
        // GPUI reports logical points with top-left origin. GPUI's native
        // GPUIView does not override isFlipped(), so it uses AppKit's standard
        // bottom-left coordinates. Frames are points; AppKit handles Retina.
        let y = if parent.isFlipped() {
            parent_bounds.origin.y + bounds.y
        } else {
            parent_bounds.origin.y + parent_bounds.size.height - bounds.y - bounds.height
        };
        let rect = NSRect::new(
            NSPoint::new(parent_bounds.origin.x + bounds.x, y),
            NSSize::new(bounds.width, bounds.height),
        );
        control.view.setFrame(rect);
    }

    pub(crate) fn set_visible(&mut self, id: u64, visible: bool) {
        if let Some(control) = self.controls.get(&id) {
            control.view.setHidden(!visible);
        }
    }

    pub(crate) fn remove(&mut self, id: u64) {
        if let Some(control) = self.controls.remove(&id) {
            control.view.removeFromSuperview();
        }
    }

    pub(crate) fn prune_missing(&mut self, live_ids: &std::collections::HashSet<u64>) {
        let stale: Vec<_> = self
            .controls
            .keys()
            .filter(|id| !live_ids.contains(id))
            .copied()
            .collect();
        for id in stale {
            self.remove(id);
        }
    }
}

impl Drop for NativeControlRegistry {
    fn drop(&mut self) {
        for (_, control) in self.controls.drain() {
            control.view.removeFromSuperview();
        }
    }
}

pub(crate) fn update_bounds(
    registry: &Rc<RefCell<NativeControlRegistry>>,
    id: u64,
    bounds: ControlBounds,
) {
    registry.borrow_mut().update_bounds(id, bounds);
    registry.borrow_mut().set_visible(id, true);
}
