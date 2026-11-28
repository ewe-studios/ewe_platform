use std::process;

use gpui::{
    Bounds, Context, FocusHandle, FontWeight, KeyBinding, QuitMode, Role, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, rgb, size,
};

actions!(counter, [Activate, Tab, TabPrev]);

#[derive(Debug)]
struct Counter {
    count: u32,
    window_focus: FocusHandle,
    button_focus: FocusHandle,
}

impl Counter {
    fn add_one(&mut self, cx: &mut Context<Self>) {
        self.count = self.count.saturating_add(1);
        cx.notify();
    }
}

impl Render for Counter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let count: SharedString = format!("Count: {}", self.count).into();

        div()
            .id("counter")
            .track_focus(&self.window_focus)
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .bg(rgb(0xf8fafc))
            .text_color(rgb(0x0f172a))
            .on_action(cx.listener(|_, _: &Tab, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &TabPrev, window, cx| window.focus_prev(cx)))
            .child(
                div()
                    .id("heading")
                    .role(Role::Heading)
                    .aria_level(1)
                    .aria_label("Counter")
                    .text_size(px(28.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Counter"),
            )
            .child(
                div()
                    .id("count")
                    .role(Role::Status)
                    .aria_label(count.clone())
                    .text_size(px(18.0))
                    .text_color(rgb(0x334155))
                    .child(count),
            )
            .child(
                div()
                    .id("add-one")
                    .track_focus(&self.button_focus)
                    .key_context("CounterButton")
                    .role(Role::Button)
                    .aria_label("Add one")
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(110.0))
                    .h(px(38.0))
                    .rounded_md()
                    .border_2()
                    .border_color(rgb(0x1d4ed8))
                    .bg(rgb(0x1d4ed8))
                    .text_color(gpui::white())
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(0x1e40af)))
                    .focus_visible(|style| style.border_color(rgb(0x0f172a)))
                    .on_click(cx.listener(|counter, _, _, cx| counter.add_one(cx)))
                    .on_action(cx.listener(|counter, _: &Activate, _, cx| counter.add_one(cx)))
                    .child("Add one"),
            )
    }
}

fn main() {
    gpui_platform::application()
        .with_quit_mode(QuitMode::LastWindowClosed)
        .run(|cx| {
            cx.bind_keys([
                KeyBinding::new("tab", Tab, None),
                KeyBinding::new("shift-tab", TabPrev, None),
                KeyBinding::new("enter", Activate, Some("CounterButton")),
                KeyBinding::new("space", Activate, Some("CounterButton")),
            ]);

            let bounds = Bounds::centered(None, size(px(400.0), px(240.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Zed GPUI Counter".into()),
                    ..Default::default()
                }),
                ..Default::default()
            };

            match cx.open_window(options, |window, cx| {
                cx.new(|cx| {
                    let window_focus = cx.focus_handle();
                    window.focus(&window_focus, cx);
                    Counter {
                        count: 0,
                        window_focus,
                        button_focus: cx.focus_handle().tab_index(1).tab_stop(true),
                    }
                })
            }) {
                Ok(_) => cx.activate(true),
                Err(cause) => {
                    eprintln!("Open Zed GPUI Counter: {cause:#}");
                    process::exit(1);
                }
            }
        });
}
