//! The model picker: a trigger that shows the chosen model, and a list of the
//! provider's models with a filter. The design is in the Paper file, page
//! "Model picker · Thinking".
//!
//! The list opens in its own borderless window, so it can reach past the edge
//! of the Hub or the onboarding window. It opens under its trigger, or above
//! it when the screen has no room below, and it takes the height of its
//! content. A pick, Escape, or a click anywhere else closes it.
//!
//! For a provider, the list also has the thinking level of the chosen model,
//! under the models: a pen line that gets heavier with the level. The ink
//! follows the pointer while the user holds the line, and it runs to the
//! nearest level when the user lets go. The list stays open.
//!
//! The models themselves live in [`AppModel::model_lists`], so every view that
//! shows the same provider shares one fetch, and an open list updates when a
//! fetch ends.

use crate::model::{AppModel, ModelList};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use crate::shell::{self as mac, native as ns_window};
use sayso_i18n::t;
use sayso_ui::fonts::{Face, LocaleFonts as _};
use sayso_ui::assets::Icon;
use sayso_ui::components::*;
use sayso_ui::{ActivePaper, text};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};

const SHEET_W: f32 = 340.;
/// The space at the left and the right of the thinking level.
const THINKING_PAD: f32 = 18.;
/// How long the ink takes to run to a level after the user lets go.
const SETTLE: Duration = Duration::from_millis(180);
/// Room around the sheet inside the window for its shadow, which is kept
/// small: the margin is part of the window and takes clicks.
const SIDE: f32 = 12.;
const TOP: f32 = 6.;
const BOTTOM: f32 = 16.;
/// Space between the trigger and the sheet.
const GAP: f64 = 4.;
/// The tallest the sheet gets: filter, 280 px of rows, the thinking level.
const MAX_SHEET_H: f32 = 430.;
/// A click on the trigger first closes the open list (the list loses focus).
/// That click means "close", so it must not open the list again.
const REOPEN_GUARD: Duration = Duration::from_millis(400);

/// A trigger's place on screen, in Cocoa coordinates (origin at the bottom
/// left of the main display, y up). GPUI's window bounds are relative to the
/// window's own display, so they cannot place a window on another display.
#[derive(Clone, Copy)]
struct Anchor {
    left: f64,
    top: f64,
    bottom: f64,
    /// The window the trigger is in. The list becomes its child window.
    window: mac::NativeWindow,
    /// The same window for GPUI, and the trigger in its coordinates, for a
    /// system that places the list itself (Wayland).
    handle: AnyWindowHandle,
    local: Bounds<Pixels>,
}

thread_local! {
    static ANCHORS: RefCell<HashMap<String, Anchor>> = RefCell::default();
    static OPEN: RefCell<Option<(String, AnyWindowHandle)>> = const { RefCell::new(None) };
    static LAST_CLOSE: RefCell<Option<(String, Instant)>> = const { RefCell::new(None) };
}

/// What the list shows above the provider's models, for example
/// "Provider's model" in the style editor. Picking it gives `None`.
pub struct Extra {
    pub label: SharedString,
    pub selected: bool,
}

/// Gets the model id, or None for the [`Extra`] row.
pub type OnPick = Box<dyn Fn(Option<String>, &mut App)>;

pub struct Request {
    /// The trigger, for example "provider:openrouter". One list per key.
    pub key: String,
    /// The key in [`AppModel::model_lists`].
    pub list_key: String,
    pub selected: Option<String>,
    pub extra: Option<Extra>,
    /// The id of the provider whose thinking level the list sets. None for a
    /// list without the thinking level.
    pub thinking_for: Option<String>,
    pub on_pick: OnPick,
    pub on_refresh: Box<dyn Fn(&mut App)>,
}

/// Records where a trigger is, so its list can open under it. Add it as a
/// child of a `relative()` trigger.
pub fn anchor(key: impl Into<String>) -> impl IntoElement {
    let key = key.into();
    canvas(
        move |bounds, window, _| {
            // The content view fills the window frame (full-size content).
            let Some(ns) = ns_window(window) else { return };
            // No frame where the system places windows itself; the list then opens as a popup.
            let frame = mac::frame(ns).unwrap_or(mac::Rect { x: 0., y: 0., width: 0., height: 0. });
            let window_top = frame.y + frame.height;
            let anchor = Anchor {
                left: frame.x + bounds.left().as_f32() as f64,
                top: window_top - bounds.top().as_f32() as f64,
                bottom: window_top - bounds.bottom().as_f32() as f64,
                window: ns,
                handle: window.window_handle(),
                local: bounds,
            };
            ANCHORS.with(|a| a.borrow_mut().insert(key, anchor));
        },
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
}

/// The inline trigger: "Haiku 4.5 ⌄", or "Choose a model ⌄" in accent when
/// nothing is chosen. Attach `on_click` that calls [`open`].
pub fn trigger(key: &str, label: impl Into<SharedString>, chosen: bool, cx: &App) -> Stateful<Div> {
    let c = cx.paper().colors;
    let fg = if chosen { c.ink } else { c.accent };
    div()
        .id(SharedString::from(format!("trigger-{key}")))
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.))
        .h(px(22.))
        .px(px(7.))
        .ml(px(-7.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(c.sheet_raised))
        .child(text::ui(label.into(), 12., FontWeight::SEMIBOLD, fg).truncate())
        .child(icon(Icon::ChevronDown, 10., fg))
        .child(anchor(key))
}

/// Open the list for `req.key`, or close it when it is open.
pub fn open(model: &Entity<AppModel>, req: Request, cx: &mut App) {
    if close_open(cx).as_deref() == Some(req.key.as_str()) {
        return;
    }
    let just_closed = LAST_CLOSE.with(|l| l.borrow().as_ref().is_some_and(|(k, at)| *k == req.key && at.elapsed() < REOPEN_GUARD));
    if just_closed {
        return;
    }
    let Some(anchor) = ANCHORS.with(|a| a.borrow().get(&req.key).copied()) else { return };
    if let Some(kind) = mac::popup_kind(anchor.handle, anchor.local, point(px(-SIDE), px(GAP as f32 - TOP))) {
        return open_popup(model, req, anchor, kind, cx);
    }

    // Below the trigger, or above it when the screen has more room there.
    let screens = mac::screens();
    let screen = screens
        .iter()
        .find(|s| anchor.left >= s.frame.x && anchor.left < s.frame.x + s.frame.width && anchor.bottom >= s.frame.y && anchor.bottom <= s.frame.y + s.frame.height)
        .or(screens.first())
        .map(|s| s.visible_frame);
    let Some(area) = screen else { return };
    let below = anchor.bottom - area.y;
    let above = area.y + area.height - anchor.top;
    let flipped = below < (MAX_SHEET_H + BOTTOM) as f64 + GAP && above > below;
    let width = (SHEET_W + SIDE * 2.) as f64;
    // The sheet's left edge meets the trigger's.
    let left = (anchor.left - SIDE as f64).max(area.x).min(area.x + area.width - width);
    let place = Place {
        parent: anchor.window,
        left,
        edge: if flipped { anchor.top + GAP - BOTTOM as f64 } else { anchor.bottom - GAP + TOP as f64 },
        flipped,
    };

    let key = req.key.clone();
    let model = model.clone();
    // The window measures its sheet in its first frame and then takes its
    // size. Windows draws only a shown window, so there the list opens shown,
    // at its place with a first guess of the height; other systems open it hidden.
    const FIRST_H: f32 = 160.;
    let cocoa_top = if place.flipped { place.edge + FIRST_H as f64 } else { place.edge };
    let origin = point(px(place.left as f32), px((mac::primary_screen_height() - cocoa_top) as f32));
    // Plain GPUI, not `gpui_kit::open_window`: the kit's root paints the theme
    // background over the whole window, which shows around the sheet as a frame.
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds { origin, size: size(px(SHEET_W + SIDE * 2.), px(FIRST_H)) })),
            titlebar: None,
            focus: true,
            show: cfg!(windows),
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| PickerWindow::new(model, req, place, window, cx)),
    );
    match opened {
        Ok(handle) => OPEN.with(|o| *o.borrow_mut() = Some((key, handle.into()))),
        Err(e) => log::error!("could not open the model list: {e:#}"),
    }
}

/// Open the list as a popup that the system places under the trigger.
fn open_popup(model: &Entity<AppModel>, req: Request, anchor: Anchor, kind: WindowKind, cx: &mut App) {
    let place = Place { parent: anchor.window, left: 0., edge: 0., flipped: false };
    let key = req.key.clone();
    let model = model.clone();
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(0.), px(0.)),
                size: size(px(SHEET_W + SIDE * 2.), px(MAX_SHEET_H + TOP + BOTTOM)),
            })),
            titlebar: None,
            focus: true,
            show: true,
            kind,
            is_movable: false,
            is_resizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        move |window, cx| cx.new(|cx| PickerWindow::new(model, req, place, window, cx)),
    );
    match opened {
        Ok(handle) => OPEN.with(|o| *o.borrow_mut() = Some((key, handle.into()))),
        Err(e) => log::error!("could not open the model list: {e:#}"),
    }
}

/// Close the open list. Returns its key.
fn close_open(cx: &mut App) -> Option<String> {
    let (key, handle) = OPEN.with(|o| o.borrow_mut().take())?;
    LAST_CLOSE.with(|l| *l.borrow_mut() = Some((key.clone(), Instant::now())));
    // Deferred: the call may come from inside the list window's own update.
    cx.defer(move |cx| {
        let _ = handle.update(cx, |_, window, _| window.remove_window());
    });
    Some(key)
}

/// Where the window goes, in Cocoa coordinates. `edge` is the window's top
/// (below the trigger) or its bottom (above it, `flipped`).
#[derive(Clone, Copy)]
struct Place {
    parent: mac::NativeWindow,
    left: f64,
    edge: f64,
    flipped: bool,
}

struct PickerWindow {
    model: Entity<AppModel>,
    req: Request,
    place: Place,
    filter: Entity<InputState>,
    ns: Option<mac::NativeWindow>,
    /// The window height last set, and the sheet height measured in prepaint.
    height: f32,
    measured: Rc<Cell<f32>>,
    was_active: bool,
    /// Where the ink of the thinking level ends while the user holds the line.
    held: Option<f32>,
    /// The run of the ink to a level after the user let go: from, to, start.
    settle: Option<(f32, f32, Instant)>,
    /// The place of the pen line, from its last paint.
    line: Rc<Cell<Bounds<Pixels>>>,
    _subs: Vec<Subscription>,
}

/// The thinking level that a list sets.
struct Thinking {
    provider: String,
    /// From the least thinking to the most.
    levels: Vec<String>,
    /// The level that goes to the provider. None when the model uses its own default.
    chosen: Option<usize>,
}

impl PickerWindow {
    fn new(model: Entity<AppModel>, req: Request, place: Place, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder(t!("model_picker.filter")));
        filter.update(cx, |s, cx| s.focus(window, cx));
        let subs = vec![
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.subscribe(&filter, |_, _, _: &InputEvent, cx| cx.notify()),
            // A click anywhere else takes focus away: close.
            cx.observe_window_activation(window, |this, window, cx| {
                if window.is_window_active() {
                    this.was_active = true;
                } else if this.was_active {
                    let _ = close_open(cx);
                }
            }),
        ];
        let ns = ns_window(window);
        Self {
            model,
            req,
            place,
            filter,
            ns,
            height: 0.,
            measured: Rc::default(),
            was_active: false,
            held: None,
            settle: None,
            line: Rc::default(),
            _subs: subs,
        }
    }

    /// The thinking level of the chosen model. None for a list without it,
    /// and for a model without levels.
    fn thinking(&self, cx: &App) -> Option<Thinking> {
        let id = self.req.thinking_for.as_ref()?;
        let m = self.model.read(cx);
        let provider = m.config.ai.providers.iter().find(|p| &p.id == id)?;
        let levels = m.thinking_choices(provider);
        let chosen = provider.thinking_sent().and_then(|sent| levels.iter().position(|level| level == sent));
        (!levels.is_empty()).then(|| Thinking { provider: id.clone(), levels, chosen })
    }

    /// Move the ink to the pointer.
    fn hold(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let line = self.line.get();
        self.held = Some(value_at((x - line.origin.x).as_f32(), line.size.width.as_f32()));
        self.settle = None;
        cx.notify();
    }

    /// Let go of the line: the nearest level becomes the thinking level.
    fn let_go(&mut self, cx: &mut Context<Self>) {
        let Some(value) = self.held.take() else { return };
        let Some(thinking) = self.thinking(cx) else { return };
        let stop = nearest_stop(value, thinking.levels.len());
        self.settle = Some((value, stop_value(stop, thinking.levels.len()), Instant::now()));
        let level = thinking.levels[stop].clone();
        self.model.update(cx, |m, cx| m.set_provider_thinking(&thinking.provider, level, cx));
        cx.notify();
    }

    /// The part of the sheet with the thinking level.
    fn thinking_panel(&mut self, thinking: &Thinking, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let c = cx.paper().colors;
        let count = thinking.levels.len();
        let rest = thinking.chosen.map_or(0., |stop| stop_value(stop, count));
        let value = match (self.held, self.settle) {
            (Some(held), _) => held,
            (None, Some((from, to, start))) if start.elapsed() < SETTLE => {
                // Fast at first, then slow, as ink that runs out.
                let t = start.elapsed().as_secs_f32() / SETTLE.as_secs_f32();
                window.request_animation_frame();
                from + (to - from) * (1. - (1. - t).powi(3))
            }
            _ => {
                self.settle = None;
                rest
            }
        };
        // While the user holds the line, the name is that of the nearest level.
        let marked = if self.held.is_some() { Some(nearest_stop(value, count)) } else { thinking.chosen };
        let name = marked.map_or_else(|| t!("model_picker.default"), |stop| sayso_core::config::thinking_label(&thinking.levels[stop]));
        let line = self.line.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .pt(px(12.))
            .pb(px(10.))
            .px(px(THINKING_PAD))
            .bg(c.sheet)
            .border_t_1()
            .border_color(c.rule)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(crate::hub::pages::kit::caps(t!("model_picker.thinking"), 10., c.graphite).font_weight(FontWeight::SEMIBOLD))
                    .child(crate::hub::pages::kit::fraunces(name, 21., 24., c.ink).italic()),
            )
            .child(
                div()
                    .id("thinking-line")
                    .relative()
                    .w_full()
                    .h(px(pen_line_height(SHEET_W - THINKING_PAD * 2.)))
                    .cursor_pointer()
                    .child(canvas(move |bounds, _, _| line.set(bounds), |_, _, _, _| {}).absolute().inset_0())
                    .child(pen_line(value, count, marked.unwrap_or(usize::MAX), self.held.is_some(), &c).size_full())
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, ev: &MouseDownEvent, _, cx| this.hold(ev.position.x, cx))),
            )
    }

    fn pick(&mut self, pick: Option<String>, cx: &mut Context<Self>) {
        (self.req.on_pick)(pick, cx);
        let _ = close_open(cx);
    }

    /// Size the window to the sheet, keeping the edge at the trigger fixed.
    fn fit(&mut self, cx: &mut Context<Self>) {
        let sheet = self.measured.get();
        if sheet <= 0. || (sheet + TOP + BOTTOM - self.height).abs() < 0.5 {
            return;
        }
        let first = self.height == 0.;
        self.height = sheet + TOP + BOTTOM;
        let Some(ns) = self.ns else { return };
        let (w, h) = ((SHEET_W + SIDE * 2.) as f64, self.height as f64);
        // Cocoa y is the bottom edge.
        let y = if self.place.flipped { self.place.edge } else { self.place.edge - h };
        let x = self.place.left;
        let parent = self.place.parent;
        crate::app::appkit_later(cx, move || {
            mac::set_frame(ns, x, y, w, h);
            if first {
                // The sheet draws its own shadow in the margin.
                mac::make_clear(ns);
                // Move with the window that holds the trigger.
                mac::add_child_window(parent, ns);
                mac::show_and_focus(ns);
            }
        });
    }
}

impl Render for PickerWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The size measured in the last frame; the first frame only measures.
        self.fit(cx);
        let c = cx.paper().colors;
        let list = self.model.read(cx).model_lists.get(&self.req.list_key).cloned();
        let query = self.filter.read(cx).value().trim().to_lowercase();
        let selected = self.req.selected.clone();

        let mut rows = div().id("model-rows").flex().flex_col().max_h(px(280.)).overflow_y_scroll().py(px(4.)).px(px(4.));
        // A row: the name in the display serif, the id at the right, and a
        // note below. An ink dot marks the chosen model.
        let row = |id: ElementId, name: String, model_id: Option<String>, note: Option<String>, is_selected: bool| {
            let mark = div().flex().flex_none().items_center().w(px(8.)).h(px(20.));
            let mut row = div().id(id).flex().items_start().gap(px(8.)).py(px(6.)).px(px(10.)).rounded(px(8.)).cursor_pointer();
            row = if is_selected {
                row.bg(c.sheet).shadow(vec![
                    BoxShadow::new(px(0.), px(1.), c.shadow(0.14)).blur_radius(px(2.)).inset(),
                    BoxShadow::new(px(0.), px(-1.), c.highlight(0.8)).inset(),
                ])
            } else {
                row.hover(|s| s.bg(c.sheet))
            };
            row.child(mark.when(is_selected, |d| d.child(div().size(px(7.)).rounded_full().bg(c.ink)))).child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(1.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(12.))
                            .child(
                                crate::hub::pages::kit::fraunces(name, 15., 20., c.ink)
                                    .font_weight(if is_selected { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                                    .min_w_0()
                                    .truncate(),
                            )
                            .when_some(model_id, |d, id| {
                                d.child(text::mono(id, 10., if is_selected { c.graphite } else { c.pencil }).font_weight(FontWeight::NORMAL).flex_none())
                            }),
                    )
                    .when_some(note, |d, n| d.child(text::ui(n, 11., FontWeight::NORMAL, c.graphite).line_height(px(15.)).truncate())),
            )
        };
        if let Some(extra) = &self.req.extra {
            rows = rows.child(
                row("model-extra".into(), extra.label.to_string(), None, None, extra.selected)
                    .on_click(cx.listener(|this, _, _, cx| this.pick(None, cx))),
            );
        }
        let message = |s: String, color: Hsla| div().px(px(10.)).py(px(8.)).child(text::ui(s, 13., FontWeight::NORMAL, color).line_height(px(18.)));
        let mut count = None;
        match &list {
            None | Some(ModelList::Loading) => rows = rows.child(message(t!("model_picker.loading"), c.graphite)),
            Some(ModelList::Failed(e)) => {
                rows = rows.child(message(t!("model_picker.load_failed", error = e), c.danger));
            }
            Some(ModelList::Ready(models)) => {
                count = Some(models.len());
                let shown: Vec<_> = models
                    .iter()
                    .filter(|m| query.is_empty() || m.name.to_lowercase().contains(&query) || m.id.to_lowercase().contains(&query))
                    .collect();
                if models.is_empty() {
                    rows = rows.child(message(t!("model_picker.no_models"), c.graphite));
                } else if shown.is_empty() {
                    rows = rows.child(message(t!("model_picker.no_match"), c.graphite));
                }
                for (i, m) in shown.into_iter().enumerate() {
                    // HTTP lists give only ids: the name is then the id, and it shows one time.
                    let model_id = (m.name != m.id).then(|| m.id.clone());
                    let id = m.id.clone();
                    rows = rows.child(
                        row(ElementId::from(("model", i)), m.name.clone(), model_id, m.description.clone(), selected.as_deref() == Some(m.id.as_str()))
                            .on_click(cx.listener(move |this, _, _, cx| this.pick(Some(id.clone()), cx))),
                    );
                }
            }
        }

        let head = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .py(px(6.))
            .pl(px(14.))
            .pr(px(6.))
            .border_b_1()
            .border_color(c.rule)
            .child(icon(Icon::Search, 12., c.pencil))
            .child(crate::hub::pages::kit::bare_input(&self.filter, 13.).flex_1())
            .children(count.map(|n| {
                let models = t!("model_picker.count", count = n);
                crate::hub::pages::kit::caps(models, 10., c.pencil).font_weight(FontWeight::SEMIBOLD).flex_none()
            }))
            .child(
                Button::new("refresh-models", t!("model_picker.refresh"))
                    .small()
                    .ghost()
                    .disabled(matches!(list, Some(ModelList::Loading)))
                    .on_click(cx.listener(|this, _, _, cx| (this.req.on_refresh)(cx))),
            );

        let thinking = self.thinking(cx).map(|thinking| self.thinking_panel(&thinking, window, cx));

        let measured = self.measured.clone();
        let sheet = div()
            .id("model-picker")
            .relative()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(SHEET_W))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(c.sheet_raised)
            // Tighter than `paper::floating`, so the margin that holds it stays small.
            .shadow(vec![
                BoxShadow::new(px(0.), px(1.), c.highlight(1.0)).inset(),
                BoxShadow::new(px(0.), px(-1.), c.shadow(0.10)).inset(),
                BoxShadow::new(px(0.), px(1.), c.shadow(0.14)).blur_radius(px(2.)),
                BoxShadow::new(px(0.), px(4.), c.shadow(0.14)).blur_radius(px(12.)),
            ])
            .child(head)
            .child(rows)
            .children(thinking)
            // Measure the sheet, so the window can take its height.
            .child(
                canvas(
                    move |bounds, window, _| {
                        let h = bounds.size.height.as_f32();
                        if (measured.replace(h) - h).abs() >= 0.5 {
                            // Render again, so `fit` sees the new height.
                            window.request_animation_frame();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .inset_0(),
            );
        div()
            .size_full()
            .flex()
            .flex_col()
            .when(self.place.flipped, |d| d.justify_end())
            .pt(px(TOP))
            .px(px(SIDE))
            .pb(px(BOTTOM))
            .font_family(sayso_ui::fonts::UI)
            .locale_fonts(Face::Sans)
            .text_color(c.ink)
            .capture_key_down(cx.listener(|_, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    cx.stop_propagation();
                    let _ = close_open(cx);
                }
            }))
            // The whole window: the pointer can leave the line while the user holds it.
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, cx| {
                if this.held.is_some() {
                    if ev.pressed_button == Some(MouseButton::Left) {
                        this.hold(ev.position.x, cx);
                    } else {
                        this.let_go(cx);
                    }
                }
            }))
            .on_mouse_up(MouseButton::Left, cx.listener(|this, _, _, cx| this.let_go(cx)))
            .child(sheet)
    }
}
