//! A theme reproducing the "Sun Valley" (sv-ttk) look of the original GUI.
//!
//! The original (Python/tkinter) front-end shipped `theme/sun-valley.tcl`
//! together with the sprite sheets `theme/light.png` and `theme/dark.png`.
//! That code and those assets have since been removed from this repository;
//! this module mirrors their palette, the widget metrics of the tcl file and
//! its custom widgets (notebook tabs, switch check buttons, radio buttons,
//! spin boxes) so the rewrite looks the same as the original.
//!
//! Colour values below were sampled directly from the two sprite sheets.

use egui::style::WidgetVisuals;
use egui::{Align2, Color32, CornerRadius, FontId, Margin, Response, Sense, Stroke, Ui, Vec2};

/// Height of buttons, entries, combo boxes and spin boxes (tkinter + padding).
pub const ROW_HEIGHT: f32 = 26.0;

/// Shorthand for the palette table below.
const fn c(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// The sv-ttk palette, one table per theme.
///
/// Some entries are kept for completeness even if the widgets built from this
/// palette happen not to use them yet.
#[allow(dead_code)]
#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    /// Window, panel and notebook content background.
    pub bg: Color32,
    /// Separators, notebook border and widget outlines.
    pub border: Color32,
    /// Primary text colour.
    pub fg: Color32,
    /// Secondary text (chevrons, hints).
    pub weak_fg: Color32,
    /// Text of disabled widgets.
    pub disabled_fg: Color32,
    pub button_bg: Color32,
    pub button_hover: Color32,
    pub button_pressed: Color32,
    pub button_border: Color32,
    /// Entries, text areas and combo boxes.
    pub entry_bg: Color32,
    pub entry_border: Color32,
    /// Accent: filled buttons, progress bar, hyperlinks.
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    /// Text drawn on top of [`Self::accent`].
    pub accent_fg: Color32,
    /// Accent of the "on" state of switches, check boxes and radio buttons.
    pub accent_alt: Color32,
    pub accent_alt_hover: Color32,
    /// Foreground of the "on" indicator (switch knob, check mark, radio centre).
    pub on_fg: Color32,
    /// Track / fill of the "off" state of switches, check boxes and radios.
    pub track: Color32,
    pub track_hover: Color32,
    /// Knob of a switch in the "off" state.
    pub knob: Color32,
    /// Outline of an unchecked check box / radio button.
    pub indicator_border: Color32,
    /// Notebook tabs.
    pub tab_bg: Color32,
    pub tab_hover: Color32,
    pub tab_selected: Color32,
    /// Text selection.
    pub selection: Color32,
    /// Progress bar fill.
    pub progress: Color32,
    /// Warning / error text.
    pub danger: Color32,
}

impl Palette {
    pub const LIGHT: Self = Self {
        dark: false,
        bg: c(0xfa, 0xfa, 0xfa),
        border: c(0xe7, 0xe7, 0xe7),
        fg: c(0x20, 0x20, 0x20),
        weak_fg: c(0x6b, 0x6b, 0x6b),
        disabled_fg: c(0xa2, 0xa2, 0xa2),
        button_bg: c(0xfd, 0xfd, 0xfd),
        button_hover: c(0xf9, 0xf9, 0xf9),
        button_pressed: c(0xfa, 0xfa, 0xfa),
        button_border: c(0xd6, 0xd6, 0xd6),
        entry_bg: c(0xf9, 0xf9, 0xf9),
        entry_border: c(0xd6, 0xd6, 0xd6),
        accent: c(0x05, 0x60, 0xb6),
        accent_hover: c(0x1e, 0x6f, 0xbc),
        accent_pressed: c(0x32, 0x7e, 0xc5),
        accent_fg: c(0xff, 0xff, 0xff),
        accent_alt: c(0x19, 0x6e, 0xbf),
        accent_alt_hover: c(0x32, 0x7e, 0xc5),
        on_fg: c(0xff, 0xff, 0xff),
        track: c(0xea, 0xea, 0xea),
        track_hover: c(0xe2, 0xe2, 0xe2),
        knob: c(0x5c, 0x5c, 0x5c),
        indicator_border: c(0x8a, 0x8a, 0x8a),
        tab_bg: c(0xe7, 0xe7, 0xe7),
        tab_hover: c(0xf4, 0xf4, 0xf4),
        tab_selected: c(0xfa, 0xfa, 0xfa),
        selection: c(0x2f, 0x60, 0xd8),
        progress: c(0x05, 0x60, 0xb6),
        danger: c(0xc4, 0x2b, 0x1c),
    };

    pub const DARK: Self = Self {
        dark: true,
        bg: c(0x1c, 0x1c, 0x1c),
        border: c(0x2f, 0x2f, 0x2f),
        fg: c(0xff, 0xff, 0xff),
        weak_fg: c(0xa0, 0xa0, 0xa0),
        disabled_fg: c(0x7a, 0x7a, 0x7a),
        button_bg: c(0x2a, 0x2a, 0x2a),
        button_hover: c(0x2f, 0x2f, 0x2f),
        button_pressed: c(0x23, 0x23, 0x23),
        button_border: c(0x3a, 0x3a, 0x3a),
        entry_bg: c(0x2f, 0x2f, 0x2f),
        entry_border: c(0x3a, 0x3a, 0x3a),
        accent: c(0x57, 0xc8, 0xff),
        accent_hover: c(0x51, 0xb7, 0xeb),
        accent_pressed: c(0x4b, 0xa6, 0xd5),
        accent_fg: c(0x00, 0x00, 0x00),
        accent_alt: c(0x51, 0xb7, 0xeb),
        accent_alt_hover: c(0x4b, 0xa6, 0xd5),
        on_fg: c(0x00, 0x00, 0x00),
        track: c(0x26, 0x26, 0x26),
        track_hover: c(0x2c, 0x2c, 0x2c),
        knob: c(0xd2, 0xd2, 0xd2),
        indicator_border: c(0x98, 0x98, 0x98),
        tab_bg: c(0x2f, 0x2f, 0x2f),
        tab_hover: c(0x29, 0x29, 0x29),
        tab_selected: c(0x1c, 0x1c, 0x1c),
        selection: c(0x2f, 0x60, 0xd8),
        progress: c(0x61, 0xcb, 0xfc),
        danger: c(0xc4, 0x2b, 0x1c),
    };
}

/// The palette of the given theme.
pub fn palette(dark: bool) -> Palette {
    if dark {
        Palette::DARK
    } else {
        Palette::LIGHT
    }
}

/// Colour used for warning / error labels.
pub fn danger() -> Color32 {
    Palette::LIGHT.danger
}

/// The shared corner radius (tcl: `-border 4`).
fn radius() -> CornerRadius {
    CornerRadius::same(4)
}

/// Replaces the style of `ctx` with the Sun Valley inspired one.
pub fn apply(ctx: &egui::Context, dark: bool) {
    let p = palette(dark);

    ctx.all_styles_mut(|style| {
        let mut v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };

        v.dark_mode = dark;
        v.panel_fill = p.bg;
        v.window_fill = p.bg;
        v.extreme_bg_color = p.entry_bg;
        v.text_edit_bg_color = Some(p.entry_bg);
        v.code_bg_color = p.entry_bg;
        v.faint_bg_color = p.track;
        v.window_stroke = Stroke::new(1.0, p.border);
        v.window_corner_radius = CornerRadius::same(6);
        v.menu_corner_radius = radius();
        v.window_highlight_topmost = false;
        v.hyperlink_color = p.accent;
        v.error_fg_color = p.danger;
        v.warn_fg_color = p.danger;
        v.override_text_color = None;
        v.button_frame = true;
        v.collapsing_header_frame = false;
        v.striped = false;
        v.indent_has_left_vline = false;
        v.slider_trailing_fill = true;
        v.selection.bg_fill = p.selection;
        v.selection.stroke = Stroke::new(1.0, Color32::WHITE);

        let set = |w: &mut WidgetVisuals, bg: Color32, border: Color32| {
            w.bg_fill = bg;
            w.weak_bg_fill = bg;
            w.bg_stroke = Stroke::new(1.0, border);
            w.fg_stroke = Stroke::new(1.0, p.fg);
            w.corner_radius = radius();
            w.expansion = 0.0;
        };
        set(&mut v.widgets.noninteractive, p.bg, p.border);
        set(&mut v.widgets.inactive, p.button_bg, p.button_border);
        set(&mut v.widgets.hovered, p.button_hover, p.button_border);
        set(&mut v.widgets.active, p.button_pressed, p.button_border);
        set(&mut v.widgets.open, p.button_hover, p.button_border);

        style.visuals = v;
        style.spacing.item_spacing = Vec2::new(8.0, 8.0);
        style.spacing.button_padding = Vec2::new(10.0, 4.0);
        style.spacing.interact_size.y = ROW_HEIGHT;
        style.spacing.window_margin = Margin::same(8);
        style.spacing.menu_margin = Margin::same(4);
    });
}

/// A plain (secondary) button, sized like the tkinter ones.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    ui.add_sized([76.0, ROW_HEIGHT], egui::Button::new(text))
}

/// A filled button matching sv-ttk's `Accent.TButton` (the "Start" button).
pub fn accent_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let p = palette(ui.visuals().dark_mode);
    let fill = if enabled { p.accent } else { p.track };
    let fg = if enabled { p.accent_fg } else { p.disabled_fg };
    let button = egui::Button::new(egui::RichText::new(text).color(fg).strong())
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(radius())
        .min_size(Vec2::new(88.0, ROW_HEIGHT));
    ui.add_enabled(enabled, button)
}

/// A single line text field, sized like the tkinter entries.
pub fn text_field(ui: &mut Ui, text: &mut String, width: f32) -> Response {
    ui.add_sized([width, ROW_HEIGHT], egui::TextEdit::singleline(text))
}

/// Width of one of the two chevron click targets inside a spin box.
const CHEVRON_W: f32 = 16.0;

/// A spin box: a value field with small up/down chevrons at its right edge,
/// matching `ttk.Spinbox`.  The chevrons are painted instead of using glyphs,
/// because the bundled fonts contain no arrow characters.
pub fn spin(
    ui: &mut Ui,
    value: &mut i64,
    range: std::ops::RangeInclusive<i64>,
    width: f32,
) -> Response {
    let lo = *range.start();
    let hi = *range.end();
    let p = palette(ui.visuals().dark_mode);

    let mut response = ui.add_sized(
        [width, ROW_HEIGHT],
        egui::DragValue::new(value).range(lo..=hi),
    );

    // Two chevron click targets laid over the right end of the field.
    let rect = response.rect;
    let h = (rect.height() - 4.0).max(1.0);
    let up_rect = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - 2.0 * CHEVRON_W - 2.0, rect.min.y + 2.0),
        egui::vec2(CHEVRON_W, h),
    );
    let down_rect = egui::Rect::from_min_size(
        egui::pos2(rect.max.x - CHEVRON_W - 2.0, rect.min.y + 2.0),
        egui::vec2(CHEVRON_W, h),
    );

    if chevron(ui, up_rect, true, p).clicked() {
        *value = (*value + 1).clamp(lo, hi);
        response.mark_changed();
    }
    if chevron(ui, down_rect, false, p).clicked() {
        *value = (*value - 1).clamp(lo, hi);
        response.mark_changed();
    }
    response
}

/// Paints a small chevron (`^` or `v`) centred in `rect`; returns its response.
fn chevron(ui: &mut Ui, rect: egui::Rect, up: bool, p: Palette) -> Response {
    let id = ui
        .scope_id()
        .with(("chevron", up, rect.min.x.to_bits(), rect.min.y.to_bits()));
    let response = ui.interact(rect, id, Sense::click());
    let color = if response.hovered() { p.fg } else { p.weak_fg };
    let center = rect.center();
    let (w, h) = (4.0, 2.5);
    let dir = if up { -1.0 } else { 1.0 };
    let stroke = Stroke::new(1.4, color);
    ui.painter().add(egui::Shape::line_segment(
        [
            egui::pos2(center.x - w, center.y - dir * h),
            egui::pos2(center.x, center.y + dir * h),
        ],
        stroke,
    ));
    ui.painter().add(egui::Shape::line_segment(
        [
            egui::pos2(center.x, center.y + dir * h),
            egui::pos2(center.x + w, center.y - dir * h),
        ],
        stroke,
    ));
    response
}

/// A combo box with an explicit width, matching the tkinter comboboxes.
pub fn combo<'a>(id: &'a str, width: f32) -> egui::ComboBox<'a> {
    egui::ComboBox::from_id_salt(id).width(width)
}

/// A `Switch.TCheckbutton`: a pill shaped track with a round knob.
pub fn switch(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let p = palette(ui.visuals().dark_mode);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::new(46.0, ROW_HEIGHT), Sense::click());
        let track = egui::Rect::from_center_size(rect.center(), Vec2::new(40.0, 20.0));

        let fill = if *on {
            if response.hovered() {
                p.accent_alt_hover
            } else {
                p.accent_alt
            }
        } else if response.hovered() {
            p.track_hover
        } else {
            p.track
        };
        ui.painter()
            .rect_filled(track, CornerRadius::same(10), fill);
        if !*on {
            ui.painter().rect_stroke(
                track,
                CornerRadius::same(10),
                Stroke::new(1.0, p.indicator_border),
                egui::StrokeKind::Inside,
            );
        }

        let knob_radius = 7.0;
        let knob_x = if *on {
            track.max.x - knob_radius - 3.0
        } else {
            track.min.x + knob_radius + 3.0
        };
        let knob_color = if *on { p.on_fg } else { p.knob };
        ui.painter().circle_filled(
            egui::pos2(knob_x, track.center().y),
            knob_radius,
            knob_color,
        );

        let label_response = ui.add(egui::Label::new(label).sense(Sense::click()));
        response = response.union(label_response);
        if response.clicked() {
            *on = !*on;
            response.mark_changed();
        }
        response
    })
    .inner
}

/// A `TRadiobutton`: a circle that turns into a thick accent ring when selected.
pub fn radio<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, value: T, label: &str) -> Response {
    let p = palette(ui.visuals().dark_mode);
    let selected = *current == value;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;

        let (rect, mut response) =
            ui.allocate_exact_size(Vec2::new(26.0, ROW_HEIGHT), Sense::click());
        let center = rect.center();
        let r = 8.0;
        if selected {
            ui.painter().circle_filled(center, r, p.on_fg);
            ui.painter()
                .circle_stroke(center, r, Stroke::new(3.0, p.accent_alt));
        } else {
            let fill = if response.hovered() {
                p.track_hover
            } else {
                p.track
            };
            ui.painter().circle_filled(center, r, fill);
            ui.painter()
                .circle_stroke(center, r, Stroke::new(1.0, p.indicator_border));
        }

        let label_response = ui.add(egui::Label::new(label).sense(Sense::click()));
        response = response.union(label_response);
        if response.clicked() {
            *current = value;
            response.mark_changed();
        }
        response
    })
    .inner
}

/// The `ttk.Notebook` tab strip: rounded-top tabs that blend into the page
/// below, with a separator line underneath.
pub fn tab_bar(ui: &mut Ui, current: &mut usize, labels: &[String]) -> bool {
    let p = palette(ui.visuals().dark_mode);
    let font = FontId::proportional(14.0);
    let height = 32.0;
    let mut changed = false;

    let strip = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, label) in labels.iter().enumerate() {
            let text_width = ui
                .painter()
                .layout_no_wrap(label.clone(), font.clone(), p.fg)
                .size()
                .x;
            let width = (text_width + 32.0).ceil();
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(width, height), Sense::click());

            let selected = *current == i;
            let fill = if selected {
                p.tab_selected
            } else if response.hovered() {
                p.tab_hover
            } else {
                p.tab_bg
            };
            let corners = CornerRadius {
                nw: 6,
                ne: 6,
                sw: 0,
                se: 0,
            };
            ui.painter().rect_filled(rect, corners, fill);
            ui.painter().text(
                rect.center(),
                Align2::CENTER_CENTER,
                label,
                font.clone(),
                p.fg,
            );

            if response.clicked() && !selected {
                *current = i;
                changed = true;
            }
        }
    });

    let rect = strip.response.rect;
    ui.painter()
        .hline(rect.x_range(), rect.max.y, Stroke::new(1.0, p.border));

    changed
}