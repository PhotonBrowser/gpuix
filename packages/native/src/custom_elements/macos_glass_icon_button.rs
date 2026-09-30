//! Small AppKit control hosted over a GPUI layout placeholder.

use super::{CustomElement, CustomElementFactory, CustomRenderContext};

pub struct GlassIconButtonFactory;

impl CustomElementFactory for GlassIconButtonFactory {
    fn element_type(&self) -> &str {
        "macos-glass-icon-button"
    }

    fn create(&self, id: u64) -> Box<dyn CustomElement> {
        Box::new(GlassIconButton { id })
    }
}

struct GlassIconButton {
    id: u64,
}

impl CustomElement for GlassIconButton {
    fn render(
        &mut self,
        ctx: CustomRenderContext,
        window: &mut gpui::Window,
        _cx: &mut gpui::Context<crate::renderer::GpuixView>,
    ) -> gpui::AnyElement {
        use gpui::prelude::*;

        let id = self.id;
        let icon = ctx
            .props()
            .get("icon")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("plus")
            .to_owned();
        let label = ctx
            .props()
            .get("accessibilityLabel")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        let disabled = ctx
            .props()
            .get("disabled")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let size = ctx
            .props()
            .get("size")
            .and_then(serde_json::Value::as_f64)
            .filter(|size| size.is_finite() && *size > 0.0)
            .unwrap_or(32.0) as f32;

        let controls = ctx.native_controls.clone();
        let hidden = ctx.style().is_some_and(|style| {
            style.visibility.as_deref() == Some("hidden")
                || style.opacity == Some(0.0)
                || style.display.as_deref() == Some("none")
        });
        {
            let mut registry = controls.borrow_mut();
            registry.create_icon_button(
                id,
                window,
                &icon,
                label.as_deref(),
                disabled,
                ctx.event_callback(),
            );
            registry.set_visible(id, !hidden);
        }

        let mut placeholder = gpui::div()
            .id(gpui::ElementId::Integer(id))
            .w(gpui::px(size))
            .h(gpui::px(size));
        if let Some(style) = ctx.style() {
            placeholder = crate::renderer::apply_styles(placeholder, style);
        }
        placeholder
            .on_painted(move |bounds, _, _| {
                crate::macos_controls::update_bounds(
                    &controls,
                    id,
                    crate::macos_controls::ControlBounds {
                        x: f64::from(f32::from(bounds.origin.x)),
                        y: f64::from(f32::from(bounds.origin.y)),
                        width: f64::from(f32::from(bounds.size.width)),
                        height: f64::from(f32::from(bounds.size.height)),
                    },
                );
            })
            .into_any_element()
    }

    fn set_prop(&mut self, _key: &str, _value: serde_json::Value) {}

    fn supported_props(&self) -> &'static [&'static str] {
        &["icon", "size", "disabled", "accessibilityLabel"]
    }

    fn supported_events(&self) -> &'static [&'static str] {
        &["click"]
    }

    fn destroy(&mut self) {}
}
