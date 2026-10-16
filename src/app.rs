//! User interface styled after the Windows 11 calculator, in a dark or light theme.

mod converter_view;
mod date_view;
mod palette;

use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, Context, CornerRadius, CursorIcon, Event, FontFamily, FontId,
    Galley, Id, Key, Modifiers, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat,
    TextureHandle, TextureOptions, Ui, UiBuilder, ViewportCommand, WindowLevel, pos2,
    text::LayoutJob, vec2,
};

use crate::converter::Category;
use crate::engine::{Calculator, Command, Func, Op, format_number};
use palette::Palette;

// Glyphs from the Segoe Fluent Icons / Segoe MDL2 Assets fonts.
const ICON_MENU: char = '\u{E700}';
const ICON_CHEVRON_DOWN: char = '\u{E70D}';
const ICON_CHEVRON_UP: char = '\u{E70E}';
const ICON_SETTINGS: char = '\u{E713}';
const ICON_BACK: char = '\u{E72B}';
const ICON_DELETE: char = '\u{E74D}';
const ICON_THEME: char = '\u{E790}';
const ICON_HISTORY: char = '\u{E81C}';
const ICON_CALENDAR: char = '\u{E787}';
const ICON_COPY: char = '\u{E8C8}';
const ICON_CALCULATOR: char = '\u{E8EF}';
const ICON_MULTIPLY: char = '\u{E947}';
const ICON_PLUS: char = '\u{E948}';
const ICON_MINUS: char = '\u{E949}';
const ICON_DIVIDE: char = '\u{E94A}';
const ICON_EQUALS: char = '\u{E94E}';
const ICON_BACKSPACE: char = '\u{E94F}';
const ICON_KEEP_ON_TOP: char = '\u{EE49}';
const ICON_BACK_TO_FULL: char = '\u{EE47}';

/// The converters' icons in the navigation menu. Windows draws Volume, Weight,
/// Temperature, Speed, Data and Angle from the Calculator's own icon font, which
/// may only be used inside Windows, so those use similar Segoe Fluent Icons glyphs.
fn category_icon(category: Category) -> char {
    match category {
        Category::Volume => '\u{F158}',
        Category::Length => '\u{ECC6}',
        Category::Weight => '\u{E821}',
        Category::Temperature => '\u{E9CA}',
        Category::Energy => '\u{ECAD}',
        Category::Area => '\u{E809}',
        Category::Speed => '\u{E805}',
        Category::Time => '\u{E917}',
        Category::Power => '\u{E945}',
        Category::Data => '\u{EDA2}',
        Category::Pressure => '\u{EC4A}',
        Category::Angle => '\u{F461}',
    }
}

const HEADER_HEIGHT: f32 = 48.0;
const COMPACT_HEADER_HEIGHT: f32 = 40.0;
const MEMORY_ROW_HEIGHT: f32 = 30.0;
const MARGIN: f32 = 4.0;
const GAP: f32 = 2.0;
const RADIUS: f32 = 4.0;
const COMPACT_SIZE: egui::Vec2 = vec2(320.0, 394.0);
/// How long (in seconds) a button looks pressed after its keyboard key is pressed.
const KEY_FLASH: f64 = 0.1;

const REPOSITORY_URL: &str = "https://github.com/gavrilov/calccraft";
const FEEDBACK_URL: &str = "https://github.com/gavrilov/calccraft/issues";
/// Storage key and values of the theme choice, the same as Windows uses.
const THEME_KEY: &str = "SelectedAppTheme";
/// Storage key of the last converter and its units, the same as Windows uses.
const CONVERTER_KEY: &str = "UnitConverterPreferences";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flyout {
    None,
    History,
    Memory,
    Navigation,
}

/// Calculator mode, picked in the navigation menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Standard,
    DateCalculation,
    Converter(Category),
}

impl Mode {
    /// The modes of the menu's "Calculator" group; the converters follow.
    const CALCULATORS: [Mode; 2] = [Mode::Standard, Mode::DateCalculation];

    fn title(self) -> &'static str {
        match self {
            Mode::Standard => "Standard",
            Mode::DateCalculation => "Date calculation",
            Mode::Converter(category) => category.name(),
        }
    }
}

/// What fills the window: the calculator or, as in Windows, the Settings page instead.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Calculator,
    Settings,
}

/// The "App theme" choice on the Settings page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Theme {
    Light,
    Dark,
    /// Follow the Windows app mode, and switch when it changes.
    System,
}

impl Theme {
    /// The stored value, as Windows stores it (`Default` means "use system setting").
    fn key(self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Dark => "Dark",
            Theme::System => "Default",
        }
    }

    fn from_key(key: &str) -> Option<Theme> {
        [Theme::Light, Theme::Dark, Theme::System]
            .into_iter()
            .find(|theme| theme.key() == key)
    }

    fn preference(self) -> egui::ThemePreference {
        match self {
            Theme::Light => egui::ThemePreference::Light,
            Theme::Dark => egui::ThemePreference::Dark,
            Theme::System => egui::ThemePreference::System,
        }
    }
}

/// The palette for the theme egui resolved (the system one for `Theme::System`).
fn palette_for(theme: egui::Theme) -> &'static Palette {
    match theme {
        egui::Theme::Dark => &palette::DARK,
        egui::Theme::Light => &palette::LIGHT,
    }
}

/// Button style.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Digit,
    Operator,
    Accent,
    /// Transparent button (memory row, header icons).
    Subtle,
}

pub struct CalculatorApp {
    mode: Mode,
    calc: Calculator,
    date: date_view::DateState,
    converter: converter_view::ConverterState,
    flyout: Flyout,
    /// The right-click menu of a history or memory item, when open.
    context_menu: Option<ContextMenu>,
    /// "Keep on top" mode: a small always-on-top window without the memory row.
    compact: bool,
    full_size: Option<egui::Vec2>,
    /// Whether the Windows icon font was found.
    has_icons: bool,
    /// Command triggered from the keyboard and when, to highlight its button.
    key_flash: Option<(Command, f64)>,
    page: Page,
    theme: Theme,
    /// Colors of the theme in effect, picked at the start of every frame.
    palette: &'static Palette,
    /// The theme the title bar was last colored for.
    title_bar_theme: Option<egui::Theme>,
    title_bar: TitleBar,
    /// Whether the "App theme" card on the Settings page shows its options.
    theme_expanded: bool,
    /// The window icon, shown in the About card.
    app_icon: TextureHandle,
}

impl CalculatorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let theme = cc
            .storage
            .and_then(|storage| storage.get_string(THEME_KEY))
            .and_then(|key| Theme::from_key(&key))
            .unwrap_or(Theme::System);
        cc.egui_ctx.set_theme(theme.preference());
        let has_icons = setup_fonts(&cc.egui_ctx);
        let icon = window_icon();
        let app_icon = cc.egui_ctx.load_texture(
            "app-icon",
            egui::ColorImage::from_rgba_unmultiplied(
                [icon.width as usize, icon.height as usize],
                &icon.rgba,
            ),
            // Mipmaps keep the 64 px icon smooth at the 16 pt it is drawn at.
            TextureOptions::LINEAR.with_mipmap_mode(Some(egui::TextureFilter::Linear)),
        );
        let mut converter = converter_view::ConverterState::new();
        if let Some(saved) = cc.storage.and_then(|s| s.get_string(CONVERTER_KEY)) {
            converter.converter.restore_preferences(&saved);
        }
        Self {
            mode: Mode::Standard,
            calc: Calculator::default(),
            date: date_view::DateState::new(),
            converter,
            flyout: Flyout::None,
            context_menu: None,
            compact: false,
            full_size: None,
            has_icons,
            key_flash: None,
            page: Page::Calculator,
            theme,
            palette: palette_for(cc.egui_ctx.theme()),
            title_bar_theme: None,
            title_bar: TitleBar::new(cc),
            theme_expanded: false,
            app_icon,
        }
    }

    fn set_theme(&mut self, ctx: &Context, theme: Theme) {
        self.theme = theme;
        ctx.set_theme(theme.preference());
    }

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.page = Page::Calculator;
        self.flyout = Flyout::None;
        self.date.close_popup();
        self.converter.close_popup();
        if let Mode::Converter(category) = mode {
            self.converter.converter.set_category(category);
        }
    }

    // -------------------------------------------------------------- keyboard

    fn handle_keyboard(&mut self, ctx: &Context) {
        if self.page == Page::Settings {
            // The calculator is hidden, so its keys and shortcuts do nothing here.
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.page = Page::Calculator;
            }
            return;
        }
        let events = ctx.input(|i| i.events.clone());
        let now = ctx.input(|i| i.time);
        for event in events {
            match event {
                Event::Text(text) => {
                    for command in text.chars().filter_map(char_command) {
                        self.press(command, now);
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    self.handle_key(ctx, key, modifiers, now);
                }
                Event::Copy => {
                    if let Some(text) = self.calc.copy_text() {
                        ctx.copy_text(text);
                    }
                }
                Event::Paste(text) => {
                    self.calc.paste(&text);
                }
                _ => {}
            }
        }
        // The button highlighted by a key press has to be turned off again.
        if self.key_flash.is_some() {
            ctx.request_repaint_after(Duration::from_secs_f64(KEY_FLASH));
        }
    }

    fn handle_key(&mut self, ctx: &Context, key: Key, modifiers: Modifiers, now: f64) {
        let command = if modifiers.command {
            match key {
                Key::M => Command::MemoryStore,
                Key::P => Command::MemoryAdd,
                Key::Q => Command::MemorySubtract,
                Key::R => Command::MemoryRecall,
                Key::L => Command::MemoryClear,
                Key::H => return self.toggle_flyout(Flyout::History),
                Key::D if modifiers.shift => return self.calc.clear_history(),
                _ => return,
            }
        } else {
            match key {
                Key::Enter => Command::Equals,
                Key::Escape if self.context_menu.is_some() => return self.context_menu = None,
                Key::Escape if self.flyout != Flyout::None => return self.flyout = Flyout::None,
                Key::Escape => Command::Clear,
                Key::Delete => Command::ClearEntry,
                Key::Backspace => Command::Backspace,
                Key::F9 => Command::Function(Func::Negate),
                Key::F1 if self.compact => return self.set_compact(ctx, false),
                _ => return,
            }
        };
        self.press(command, now);
    }

    fn press(&mut self, command: Command, now: f64) {
        self.calc.execute(command);
        self.key_flash = Some((command, now));
    }

    fn is_flashing(&self, command: Command, now: f64) -> bool {
        matches!(self.key_flash, Some((c, t)) if c == command && now - t < KEY_FLASH)
    }

    fn toggle_flyout(&mut self, flyout: Flyout) {
        self.flyout = if self.flyout == flyout {
            Flyout::None
        } else {
            flyout
        };
    }

    fn set_compact(&mut self, ctx: &Context, compact: bool) {
        self.compact = compact;
        self.flyout = Flyout::None;
        if compact {
            self.full_size = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()));
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::AlwaysOnTop));
            ctx.send_viewport_cmd(ViewportCommand::InnerSize(COMPACT_SIZE));
        } else {
            ctx.send_viewport_cmd(ViewportCommand::WindowLevel(WindowLevel::Normal));
            if let Some(size) = self.full_size.take() {
                ctx.send_viewport_cmd(ViewportCommand::InnerSize(size));
            }
        }
    }

    // --------------------------------------------------------------- buttons

    /// Draws a button and returns `true` if it was clicked.
    fn button(
        &self,
        ui: &Ui,
        rect: Rect,
        id: Id,
        kind: Kind,
        enabled: bool,
        label: LayoutJob,
    ) -> bool {
        self.button_flash(ui, rect, id, kind, enabled, label, false)
    }

    #[allow(clippy::too_many_arguments)]
    fn button_flash(
        &self,
        ui: &Ui,
        rect: Rect,
        id: Id,
        kind: Kind,
        enabled: bool,
        label: LayoutJob,
        flash: bool,
    ) -> bool {
        let p = self.palette;
        let sense = if enabled {
            Sense::CLICK
        } else {
            Sense::hover()
        };
        let response = ui.interact(rect, id, sense);
        let pressed = enabled && (response.is_pointer_button_down_on() || flash);
        let hovered = enabled && response.hovered();

        let pick = |normal, hover, down| {
            if pressed {
                down
            } else if hovered {
                hover
            } else {
                normal
            }
        };
        let fill = match kind {
            Kind::Digit if !enabled => p.digit_disabled,
            Kind::Digit => pick(p.digit, p.digit_hover, p.digit_pressed),
            Kind::Operator if !enabled => p.operator_disabled,
            Kind::Operator => pick(p.operator, p.operator_hover, p.operator_pressed),
            Kind::Accent if !enabled => p.accent_disabled,
            Kind::Accent => pick(p.accent, p.accent_hover, p.accent_pressed),
            Kind::Subtle => pick(Color32::TRANSPARENT, p.subtle_hover, p.subtle_pressed),
        };
        let text = if !enabled {
            p.text_disabled
        } else if kind == Kind::Accent {
            p.text_on_accent
        } else if pressed {
            p.text_pressed
        } else {
            p.text
        };

        let painter = ui.painter();
        painter.rect_filled(rect, RADIUS, fill);
        if matches!(kind, Kind::Digit | Kind::Operator) {
            painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, p.border), StrokeKind::Inside);
        }
        let galley = painter.layout_job(label);
        painter.galley(rect.center() - galley.size() / 2.0, galley, text);
        enabled && response.clicked()
    }

    /// A label made of one icon glyph (or fallback text without the icon font).
    fn icon(&self, glyph: char, fallback: &str, size: f32) -> LayoutJob {
        if self.has_icons {
            text(&glyph.to_string(), size, icons())
        } else {
            text(fallback, size, FontFamily::Proportional)
        }
    }

    // ---------------------------------------------------------------- header

    fn draw_header(&mut self, ui: &Ui, rect: Rect) {
        let p = self.palette;
        let ctx = ui.ctx().clone();
        let y = rect.center().y;
        let icon_button = |x: f32| Rect::from_center_size(pos2(x, y), vec2(40.0, 36.0));

        if self.compact {
            let label = self.icon(ICON_BACK_TO_FULL, "⇱", 14.0);
            if self.button(
                ui,
                icon_button(rect.left() + 24.0),
                Id::new("full"),
                Kind::Subtle,
                true,
                label,
            ) {
                self.set_compact(&ctx, false);
            }
            return;
        }

        let label = self.icon(ICON_MENU, "≡", 16.0);
        if self.button(
            ui,
            icon_button(rect.left() + 24.0),
            Id::new("menu"),
            Kind::Subtle,
            true,
            label,
        ) {
            self.toggle_flyout(Flyout::Navigation);
        }

        let title = ui.painter().text(
            pos2(rect.left() + 48.0, y),
            Align2::LEFT_CENTER,
            self.mode.title(),
            FontId::new(20.0, semibold()),
            p.text,
        );
        // Keep on top and history exist only in the Standard mode.
        if self.mode != Mode::Standard {
            return;
        }

        let label = self.icon(ICON_KEEP_ON_TOP, "⇲", 14.0);
        let keep_on_top = icon_button(title.right() + 26.0);
        if self.button(
            ui,
            keep_on_top,
            Id::new("keep-on-top"),
            Kind::Subtle,
            true,
            label,
        ) {
            self.set_compact(&ctx, true);
        }

        let label = self.icon(ICON_HISTORY, "↺", 16.0);
        if self.button(
            ui,
            icon_button(rect.right() - 24.0),
            Id::new("history"),
            Kind::Subtle,
            true,
            label,
        ) {
            self.toggle_flyout(Flyout::History);
        }
    }

    // --------------------------------------------------------------- display

    fn draw_display(&self, ui: &Ui, rect: Rect) {
        let p = self.palette;
        let painter = ui.painter();
        let right = rect.right() - 12.0;
        let max_width = rect.width() - 24.0;

        let expression = self.calc.expression();
        if !expression.is_empty() {
            let galley =
                layout_elided_left(painter, &expression, FontId::proportional(14.0), max_width);
            painter.galley(
                pos2(right - galley.size().x, rect.top() + 6.0),
                galley,
                p.text_secondary,
            );
        }

        // Long numbers shrink to fit the width.
        let display = self.calc.display();
        let base_size = (rect.height() * 0.46).clamp(28.0, 72.0);
        let mut galley =
            painter.layout_no_wrap(display.clone(), FontId::new(base_size, semibold()), p.text);
        if galley.size().x > max_width {
            let size = (base_size * max_width / galley.size().x).max(10.0);
            galley = painter.layout_no_wrap(display, FontId::new(size, semibold()), p.text);
        }
        let center_y = rect.top() + rect.height() * 0.6;
        let pos = pos2(right - galley.size().x, center_y - galley.size().y / 2.0);
        painter.galley(pos, galley, p.text);
    }

    // ------------------------------------------------------------ memory row

    fn draw_memory_row(&mut self, ui: &Ui, rect: Rect, now: f64) {
        let buttons = [
            ("MC", Command::MemoryClear),
            ("MR", Command::MemoryRecall),
            ("M+", Command::MemoryAdd),
            ("M−", Command::MemorySubtract),
            ("MS", Command::MemoryStore),
        ];
        let width = (rect.width() - 2.0 * MARGIN) / 6.0;
        let cell = |i: usize| {
            Rect::from_min_size(
                pos2(rect.left() + MARGIN + i as f32 * width, rect.top()),
                vec2(width, rect.height()),
            )
            .shrink2(vec2(1.0, 0.0))
        };
        for (i, (label, command)) in buttons.into_iter().enumerate() {
            let enabled = self.calc.is_enabled(command);
            let flash = self.is_flashing(command, now);
            let label = text(label, 12.0, semibold());
            if self.button_flash(
                ui,
                cell(i),
                Id::new("memory").with(i),
                Kind::Subtle,
                enabled,
                label,
                flash,
            ) {
                self.calc.execute(command);
            }
        }

        let enabled = !self.calc.memory().is_empty() && !self.calc.has_error();
        let mut label = text("M", 12.0, semibold());
        if self.has_icons {
            label.append(" ", 0.0, format(6.0, FontFamily::Proportional));
            label.append(&ICON_CHEVRON_DOWN.to_string(), 0.0, format(8.0, icons()));
        } else {
            label.append("˅", 0.0, format(12.0, FontFamily::Proportional));
        }
        if self.button(
            ui,
            cell(5),
            Id::new("memory-list"),
            Kind::Subtle,
            enabled,
            label,
        ) {
            self.toggle_flyout(Flyout::Memory);
        }
    }

    // ---------------------------------------------------------------- keypad

    fn keypad_label(&self, command: Command, height: f32) -> LayoutJob {
        let digit_size = (height * 0.38).clamp(14.0, 40.0);
        let icon_size = (height * 0.31).clamp(12.0, 32.0);
        let text_size = (height * 0.29).clamp(12.0, 30.0);
        let regular = FontFamily::Proportional;
        // x², ¹⁄x, ²√x as in Windows, with an italic "x".
        let formula = |before: &str, after: &str| {
            let mut job = LayoutJob::default();
            let plain = format(text_size, regular.clone());
            job.append(before, 0.0, plain.clone());
            job.append(
                "x",
                0.0,
                TextFormat {
                    italics: true,
                    ..plain.clone()
                },
            );
            job.append(after, 0.0, plain);
            job
        };
        match command {
            Command::Digit(d) => text(&d.to_string(), digit_size, semibold()),
            Command::Decimal => text(".", digit_size, semibold()),
            Command::Function(Func::Negate) => negate_label(text_size),
            Command::Function(Func::Reciprocal) => formula("¹⁄", ""),
            Command::Function(Func::Square) => formula("", "²"),
            Command::Function(Func::SquareRoot) => formula("²√", ""),
            Command::Operator(Op::Divide) => self.icon(ICON_DIVIDE, "÷", icon_size),
            Command::Operator(Op::Multiply) => self.icon(ICON_MULTIPLY, "×", icon_size),
            Command::Operator(Op::Subtract) => self.icon(ICON_MINUS, "−", icon_size),
            Command::Operator(Op::Add) => self.icon(ICON_PLUS, "+", icon_size),
            Command::Equals => self.icon(ICON_EQUALS, "=", icon_size),
            Command::Backspace => self.icon(ICON_BACKSPACE, "⌫", icon_size),
            Command::Percent => text("%", text_size, regular),
            Command::ClearEntry => text("CE", text_size, regular),
            Command::Clear => text("C", text_size, regular),
            _ => LayoutJob::default(),
        }
    }

    fn draw_keypad(&mut self, ui: &Ui, rect: Rect, now: f64) {
        use Command::*;
        #[rustfmt::skip]
        const KEYS: [Command; 24] = [
            Percent, ClearEntry, Clear, Backspace,
            Function(Func::Reciprocal), Function(Func::Square), Function(Func::SquareRoot), Operator(Op::Divide),
            Digit(7), Digit(8), Digit(9), Operator(Op::Multiply),
            Digit(4), Digit(5), Digit(6), Operator(Op::Subtract),
            Digit(1), Digit(2), Digit(3), Operator(Op::Add),
            Function(Func::Negate), Digit(0), Decimal, Equals,
        ];
        let width = (rect.width() - 3.0 * GAP) / 4.0;
        let height = (rect.height() - 5.0 * GAP) / 6.0;
        for (i, command) in KEYS.into_iter().enumerate() {
            let (row, col) = ((i / 4) as f32, (i % 4) as f32);
            let cell = Rect::from_min_size(
                rect.min + vec2(col * (width + GAP), row * (height + GAP)),
                vec2(width, height),
            );
            let kind = match command {
                Digit(_) | Decimal | Function(Func::Negate) => Kind::Digit,
                Equals => Kind::Accent,
                _ => Kind::Operator,
            };
            let enabled = self.calc.is_enabled(command);
            let flash = self.is_flashing(command, now);
            let label = self.keypad_label(command, height);
            if self.button_flash(
                ui,
                cell,
                Id::new("key").with(i),
                kind,
                enabled,
                label,
                flash,
            ) {
                self.calc.execute(command);
            }
        }
    }

    // ---------------------------------------------------- history and memory

    fn draw_flyout(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        let painter = ui.painter();
        let top = CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(rect, top, p.flyout);
        painter.rect_stroke(
            rect.expand2(vec2(1.0, 0.0)),
            top,
            Stroke::new(1.0, p.flyout_border),
            StrokeKind::Inside,
        );
        // Clicks on the panel's empty space must not fall through it.
        ui.interact(rect, Id::new("flyout"), Sense::CLICK);

        let history = self.flyout == Flyout::History;
        // A menu whose item is gone (or that belongs to the other panel) closes.
        self.context_menu = self.context_menu.filter(|menu| match menu.target {
            MenuTarget::History(i) => history && i < self.calc.history().len(),
            MenuTarget::Memory(i) => !history && i < self.calc.memory().len(),
        });
        let empty = if history {
            self.calc.history().is_empty()
        } else {
            self.calc.memory().is_empty()
        };
        if empty {
            let message = if history {
                "There's no history yet"
            } else {
                "There's nothing saved in memory"
            };
            painter.text(
                rect.left_top() + vec2(16.0, 16.0),
                Align2::LEFT_TOP,
                message,
                FontId::new(14.0, semibold()),
                p.text,
            );
            return;
        }

        let list = Rect::from_min_max(
            rect.min + vec2(4.0, 8.0),
            pos2(rect.right() - 4.0, rect.bottom() - 48.0),
        );
        let mut action = None;
        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if history {
                        for i in 0..self.calc.history().len() {
                            if let Some(a) = self.history_item(ui, i) {
                                action = Some(a);
                            }
                        }
                    } else {
                        for i in 0..self.calc.memory().len() {
                            if let Some(a) = self.memory_item(ui, i) {
                                action = Some(a);
                            }
                        }
                    }
                });
        });

        let trash = Rect::from_center_size(
            pos2(rect.right() - 28.0, rect.bottom() - 24.0),
            vec2(40.0, 36.0),
        );
        let label = self.icon(ICON_DELETE, "🗑", 15.0);
        if self.button(ui, trash, Id::new("trash"), Kind::Subtle, true, label) {
            action = Some(FlyoutAction::ClearAll);
        }
        // Drawn last, so it covers the panel and catches the clicks outside it.
        if let Some(menu) = self.context_menu {
            action = self.draw_context_menu(ui, menu);
        }

        if action.is_some() {
            self.context_menu = None;
        }
        match action {
            Some(FlyoutAction::RecallHistory(i)) => {
                self.calc.recall_history(i);
                self.flyout = Flyout::None;
            }
            Some(FlyoutAction::RecallMemory(i)) => {
                self.calc.memory_recall_item(i);
                self.flyout = Flyout::None;
            }
            Some(FlyoutAction::MemoryAdd(i, subtract)) => self.calc.memory_add(i, subtract),
            Some(FlyoutAction::MemoryClear(i)) => self.calc.memory_clear_item(i),
            Some(FlyoutAction::ClearAll) if history => self.calc.clear_history(),
            Some(FlyoutAction::ClearAll) => self.calc.execute(Command::MemoryClear),
            Some(FlyoutAction::OpenMenu(target, pos)) => {
                self.context_menu = Some(ContextMenu { target, pos });
            }
            // Windows copies the result as the list shows it, with separators.
            Some(FlyoutAction::CopyHistory(i)) => {
                ui.ctx()
                    .copy_text(format_number(&self.calc.history()[i].result));
            }
            Some(FlyoutAction::DeleteHistory(i)) => self.calc.delete_history_item(i),
            Some(FlyoutAction::CloseMenu) | None => {}
        }
    }

    /// History entry: small expression, large result. A click recalls it and a
    /// right click opens its menu.
    fn history_item(&self, ui: &mut Ui, index: usize) -> Option<FlyoutAction> {
        let p = self.palette;
        let entry = &self.calc.history()[index];
        let width = ui.available_width();
        let painter = ui.painter().clone();
        let expression = layout_elided_left(
            &painter,
            &entry.expression,
            FontId::proportional(14.0),
            width - 24.0,
        );
        let result = layout_elided_left(
            &painter,
            &format_number(&entry.result),
            FontId::new(20.0, semibold()),
            width - 24.0,
        );
        let height = 10.0 + expression.size().y + result.size().y + 10.0;

        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::CLICK);
        if response.hovered() {
            painter.rect_filled(rect, RADIUS, p.subtle_hover);
        }
        let right = rect.right() - 12.0;
        let y = rect.top() + 10.0;
        let expression_height = expression.size().y;
        painter.galley(
            pos2(right - expression.size().x, y),
            expression,
            p.text_secondary,
        );
        painter.galley(
            pos2(right - result.size().x, y + expression_height),
            result,
            p.text,
        );
        if response.secondary_clicked() {
            let pos = response.interact_pointer_pos().unwrap_or(rect.center());
            return Some(FlyoutAction::OpenMenu(MenuTarget::History(index), pos));
        }
        response
            .clicked()
            .then_some(FlyoutAction::RecallHistory(index))
    }

    /// Memory slot; on hover it shows MC, M+ and M− buttons.
    fn memory_item(&self, ui: &mut Ui, index: usize) -> Option<FlyoutAction> {
        let p = self.palette;
        let value = &self.calc.memory()[index];
        let width = ui.available_width();
        let result = layout_elided_left(
            ui.painter(),
            &format_number(value),
            FontId::new(20.0, semibold()),
            width - 24.0,
        );
        let height = 10.0 + result.size().y + 6.0 + 26.0 + 10.0;

        let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::CLICK);
        let hovered = ui.rect_contains_pointer(rect);
        let painter = ui.painter();
        if hovered {
            painter.rect_filled(rect, RADIUS, p.subtle_hover);
        }
        let right = rect.right() - 12.0;
        let result_height = result.size().y;
        painter.galley(
            pos2(right - result.size().x, rect.top() + 10.0),
            result,
            p.text,
        );

        let mut action = response
            .clicked()
            .then_some(FlyoutAction::RecallMemory(index));
        if response.secondary_clicked() {
            let pos = response.interact_pointer_pos().unwrap_or(rect.center());
            action = Some(FlyoutAction::OpenMenu(MenuTarget::Memory(index), pos));
        }
        if hovered {
            let buttons = [
                ("MC", FlyoutAction::MemoryClear(index)),
                ("M+", FlyoutAction::MemoryAdd(index, false)),
                ("M−", FlyoutAction::MemoryAdd(index, true)),
            ];
            let top = rect.top() + 10.0 + result_height + 6.0;
            for (i, (label, button_action)) in buttons.into_iter().enumerate() {
                let x = right - (3 - i) as f32 * 42.0 + 2.0;
                let cell = Rect::from_min_size(pos2(x, top), vec2(40.0, 26.0));
                let id = Id::new("memory-item").with(index).with(i);
                if self.button(
                    ui,
                    cell,
                    id,
                    Kind::Digit,
                    true,
                    text(label, 12.0, semibold()),
                ) {
                    action = Some(button_action);
                }
            }
        }
        action
    }

    /// The right-click menu of a history or memory item, a WinUI `MenuFlyout`
    /// at the pointer. A click outside it closes it.
    fn draw_context_menu(&self, ui: &Ui, menu: ContextMenu) -> Option<FlyoutAction> {
        const ITEM_HEIGHT: f32 = 32.0;
        const STRIDE: f32 = ITEM_HEIGHT + 2.0;
        const ICON_CENTER: f32 = 20.0;
        const TEXT_LEFT: f32 = 40.0;
        let p = self.palette;
        let full = ui.max_rect();
        let backdrop = ui.interact(full, Id::new("context-menu-backdrop"), Sense::CLICK);

        // The memory items show the Calculator font's MC, M+ and M− glyphs as text.
        let items = match menu.target {
            MenuTarget::History(i) => vec![
                (
                    self.icon(ICON_COPY, "⧉", 16.0),
                    "Copy",
                    FlyoutAction::CopyHistory(i),
                ),
                (
                    self.icon(ICON_DELETE, "🗑", 16.0),
                    "Delete",
                    FlyoutAction::DeleteHistory(i),
                ),
            ],
            MenuTarget::Memory(i) => vec![
                (
                    text("MC", 11.0, semibold()),
                    "Clear memory item",
                    FlyoutAction::MemoryClear(i),
                ),
                (
                    text("M+", 11.0, semibold()),
                    "Add to memory item",
                    FlyoutAction::MemoryAdd(i, false),
                ),
                (
                    text("M−", 11.0, semibold()),
                    "Subtract from memory item",
                    FlyoutAction::MemoryAdd(i, true),
                ),
            ],
        };
        let painter = ui.painter();
        let font = FontId::proportional(14.0);
        let text_width = items
            .iter()
            .map(|(_, label, _)| {
                painter
                    .layout_no_wrap(label.to_string(), font.clone(), p.text)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let size = vec2(
            (8.0 + TEXT_LEFT + text_width + 24.0).max(120.0),
            8.0 + items.len() as f32 * STRIDE - 2.0,
        );
        // The menu opens down and to the right of the pointer, and flips where
        // the window has no room for that.
        let mut min = menu.pos;
        if min.x + size.x > full.right() - 4.0 {
            min.x = (min.x - size.x).max(full.left() + 4.0);
        }
        if min.y + size.y > full.bottom() - 4.0 {
            min.y = (min.y - size.y).max(full.top() + 4.0);
        }
        let rect = Rect::from_min_size(min, size);
        date_view::popup_frame(p, painter, rect, p.popup);
        ui.interact(rect, Id::new("context-menu"), Sense::CLICK);

        let mut chosen = None;
        for (n, (icon, label, action)) in items.into_iter().enumerate() {
            let item = Rect::from_min_size(
                rect.min + vec2(4.0, 4.0 + n as f32 * STRIDE),
                vec2(rect.width() - 8.0, ITEM_HEIGHT),
            );
            let response = ui.interact(item, Id::new("context-menu-item").with(n), Sense::CLICK);
            let fill = if response.is_pointer_button_down_on() {
                p.popup_item_pressed
            } else if response.hovered() {
                p.popup_item
            } else {
                Color32::TRANSPARENT
            };
            painter.rect_filled(item, RADIUS, fill);
            let icon = painter.layout_job(icon);
            let icon_pos = pos2(item.left() + ICON_CENTER, item.center().y) - icon.size() / 2.0;
            painter.galley(icon_pos, icon, p.text);
            painter.text(
                pos2(item.left() + TEXT_LEFT, item.center().y),
                Align2::LEFT_CENTER,
                label,
                font.clone(),
                p.text,
            );
            if response.clicked() {
                chosen = Some(action);
            }
        }
        if chosen.is_none() && (backdrop.clicked() || backdrop.secondary_clicked()) {
            chosen = Some(FlyoutAction::CloseMenu);
        }
        chosen
    }

    // ------------------------------------------------------------------ menu

    fn draw_navigation(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        let painter = ui.painter().clone();
        painter.rect_filled(rect, 0.0, p.backdrop);
        let backdrop = ui.interact(rect, Id::new("nav-backdrop"), Sense::CLICK);

        let panel = Rect::from_min_size(
            rect.min,
            vec2((rect.width() * 0.85).min(280.0), rect.height()),
        );
        painter.rect_filled(panel, 0.0, p.flyout);
        painter.vline(
            panel.right(),
            panel.y_range(),
            Stroke::new(1.0, p.flyout_border),
        );
        ui.interact(panel, Id::new("nav-panel"), Sense::CLICK);

        let menu = Rect::from_center_size(
            pos2(panel.left() + 24.0, panel.top() + HEADER_HEIGHT / 2.0),
            vec2(40.0, 36.0),
        );
        let label = self.icon(ICON_MENU, "≡", 16.0);
        let mut close = self.button(ui, menu, Id::new("nav-menu"), Kind::Subtle, true, label);

        // The modes in two groups. They scroll between the header and Settings
        // when the window is too short for all of them.
        let list = Rect::from_min_max(
            pos2(panel.left(), panel.top() + HEADER_HEIGHT),
            pos2(panel.right(), panel.bottom() - 58.0),
        );
        let groups = [
            ("Calculator", Mode::CALCULATORS.to_vec()),
            ("Converter", Category::ALL.map(Mode::Converter).to_vec()),
        ];
        let mut chosen = None;
        ui.scope_builder(UiBuilder::new().max_rect(list), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("nav-modes")
                .auto_shrink(false)
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for (title, modes) in groups {
                        let size = vec2(ui.available_width(), 34.0);
                        let (header, _) = ui.allocate_exact_size(size, Sense::hover());
                        ui.painter().text(
                            pos2(header.left() + 16.0, header.top() + 8.0),
                            Align2::LEFT_TOP,
                            title,
                            FontId::new(14.0, semibold()),
                            p.text,
                        );
                        for mode in modes {
                            let size = vec2(ui.available_width(), 40.0);
                            let (row, _) = ui.allocate_exact_size(size, Sense::hover());
                            let item = row.shrink2(vec2(4.0, 0.0));
                            let icon = match mode {
                                Mode::Standard => self.icon(ICON_CALCULATOR, "▦", 16.0),
                                Mode::DateCalculation => self.icon(ICON_CALENDAR, "▤", 16.0),
                                Mode::Converter(category) => {
                                    self.icon(category_icon(category), "▫", 16.0)
                                }
                            };
                            let id = Id::new("nav-mode").with(mode.title());
                            if self.nav_item(ui, item, id, icon, mode.title(), self.mode == mode) {
                                chosen = Some(mode);
                            }
                        }
                    }
                });
        });
        if let Some(mode) = chosen {
            self.set_mode(mode);
            close = true;
        }

        // Settings sits at the bottom of the pane, below a separator, as in Windows.
        let item = Rect::from_min_size(
            pos2(panel.left() + 4.0, panel.bottom() - 46.0),
            vec2(panel.width() - 8.0, 40.0),
        );
        painter.hline(
            panel.x_range(),
            item.top() - 12.0,
            Stroke::new(1.0, p.flyout_border),
        );
        let icon = self.icon(ICON_SETTINGS, "⚙", 16.0);
        let settings = self.nav_item(ui, item, Id::new("nav-settings"), icon, "Settings", false);

        if settings {
            self.page = Page::Settings;
        }
        if close || settings || backdrop.clicked() {
            self.flyout = Flyout::None;
        }
    }

    /// Navigation pane entry. The selected one has a fill and the accent pill.
    /// Returns `true` if it was clicked.
    fn nav_item(
        &self,
        ui: &Ui,
        rect: Rect,
        id: Id,
        icon: LayoutJob,
        label: &str,
        selected: bool,
    ) -> bool {
        let p = self.palette;
        let painter = ui.painter();
        let response = ui.interact(rect, id, Sense::CLICK);
        let fill = if response.is_pointer_button_down_on() || (selected && !response.hovered()) {
            p.subtle_hover
        } else if response.hovered() {
            p.operator_hover
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, RADIUS, fill);
        if selected {
            let pill =
                Rect::from_center_size(pos2(rect.left() + 2.0, rect.center().y), vec2(3.0, 16.0));
            painter.rect_filled(pill, 2.0, p.accent);
        }
        let icon = painter.layout_job(icon);
        painter.galley(
            pos2(rect.left() + 16.0, rect.center().y - icon.size().y / 2.0),
            icon,
            p.text,
        );
        painter.text(
            pos2(rect.left() + 48.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(14.0),
            p.text,
        );
        response.clicked()
    }

    // -------------------------------------------------------------- settings

    fn draw_settings(&mut self, ui: &mut Ui, rect: Rect) {
        let p = self.palette;
        // Windows puts the back button in its own title bar. This app keeps the system title
        // bar, so the button takes the place of the menu button.
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER_HEIGHT));
        let back = Rect::from_center_size(
            pos2(header.left() + 24.0, header.center().y),
            vec2(40.0, 36.0),
        );
        let label = self.icon(ICON_BACK, "←", 16.0);
        if self.button(
            ui,
            back,
            Id::new("settings-back"),
            Kind::Subtle,
            true,
            label,
        ) {
            self.page = Page::Calculator;
        }
        ui.painter().text(
            pos2(header.left() + 48.0, header.center().y),
            Align2::LEFT_CENTER,
            "Settings",
            FontId::new(20.0, semibold()),
            p.text,
        );

        let mut action = None;
        ui.scope_builder(
            UiBuilder::new().max_rect(rect.with_min_y(header.bottom())),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("settings")
                    .auto_shrink(false)
                    .show(ui, |ui| action = self.settings_content(ui));
            },
        );
        match action {
            Some(SettingsAction::ToggleTheme) => self.theme_expanded = !self.theme_expanded,
            Some(SettingsAction::SetTheme(theme)) => self.set_theme(ui.ctx(), theme),
            Some(SettingsAction::OpenUrl(url)) => ui.ctx().open_url(egui::OpenUrl::new_tab(url)),
            None => {}
        }
    }

    /// The scrolling part of the Settings page: the Appearance and About sections and the links.
    fn settings_content(&self, ui: &mut Ui) -> Option<SettingsAction> {
        let p = self.palette;
        // Distances measured on the Windows 11 Settings page.
        const SIDE: f32 = 24.0;
        const CARD_HEIGHT: f32 = 68.0;
        // From a card's left edge to the center of its icon and to its text.
        const ICON_X: f32 = 28.0;
        const TEXT_X: f32 = 58.0;
        const OPTION_HEIGHT: f32 = 40.0;
        const THEMES: [(Theme, &str); 3] = [
            (Theme::Light, "Light"),
            (Theme::Dark, "Dark"),
            (Theme::System, "Use system setting"),
        ];

        let width = ui.available_width();
        let painter = ui.painter().clone();
        let body = FontId::proportional(14.0);

        // Lay out the text that wraps first: the page height depends on it.
        let text_width = width - 2.0 * SIDE - TEXT_X - 40.0;
        let title = painter.layout("App theme".into(), body.clone(), p.text, text_width);
        let subtitle = painter.layout(
            "Select which app theme to display".into(),
            FontId::proportional(12.0),
            p.text_secondary,
            text_width,
        );
        let header_height = (title.size().y + subtitle.size().y + 32.0).max(CARD_HEIGHT);
        let options_height = if self.theme_expanded {
            2.0 * MARGIN + THEMES.len() as f32 * OPTION_HEIGHT
        } else {
            0.0
        };
        let link_text = "GitHub";
        let prefix = "To learn how you can contribute to CalcCraft, check out the project on ";
        let paragraph = |link_color: Color32| {
            let mut job = LayoutJob::default();
            job.wrap.max_width = width - 2.0 * SIDE;
            for (part, color) in [(prefix, p.text), (link_text, link_color), (".", p.text)] {
                job.append(part, 0.0, TextFormat::simple(body.clone(), color));
            }
            painter.layout_job(job)
        };
        let mut contribute = paragraph(p.link);
        let row_height = contribute.rows.first().map_or(0.0, |row| row.size.y);

        // Vertical positions, from the top of the page body.
        let appearance_y = 23.0;
        let theme_top = appearance_y + 18.0;
        let theme_bottom = theme_top + header_height + options_height;
        let about_y = theme_bottom + 45.0;
        let about_top = about_y + 18.0;
        let feedback_y = about_top + CARD_HEIGHT + 25.0;
        let contribute_top = feedback_y + 40.0 - row_height / 2.0;
        let height = contribute_top + contribute.size().y + SIDE;

        let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
        let (left, right, top) = (rect.left() + SIDE, rect.right() - SIDE, rect.top());
        let mut action = None;

        // Appearance: the App theme card expands to show the three choices.
        section_label(p, &painter, pos2(left, top + appearance_y), "Appearance");
        let card = Rect::from_x_y_ranges(left..=right, top + theme_top..=top + theme_bottom);
        let header = card.with_max_y(card.top() + header_height);
        let response = ui.interact(header, Id::new("theme-card"), Sense::CLICK);
        let fill = if response.is_pointer_button_down_on() {
            p.card_pressed
        } else if response.hovered() {
            p.card_hover
        } else {
            p.card
        };
        painter.rect_filled(card, RADIUS, p.card);
        let corners = if self.theme_expanded {
            CornerRadius {
                nw: 4,
                ne: 4,
                sw: 0,
                se: 0,
            }
        } else {
            CornerRadius::same(4)
        };
        painter.rect_filled(header, corners, fill);
        painter.rect_stroke(
            card,
            RADIUS,
            Stroke::new(1.0, p.card_border),
            StrokeKind::Inside,
        );
        let icon = painter.layout_job(self.icon(ICON_THEME, "◐", 20.0));
        painter.galley(
            pos2(card.left() + ICON_X, header.center().y) - icon.size() / 2.0,
            icon,
            p.text,
        );
        let text_top = header.center().y - (title.size().y + subtitle.size().y) / 2.0;
        let title_height = title.size().y;
        painter.galley(pos2(card.left() + TEXT_X, text_top), title, p.text);
        painter.galley(
            pos2(card.left() + TEXT_X, text_top + title_height),
            subtitle,
            p.text_secondary,
        );
        let chevron = if self.theme_expanded {
            self.icon(ICON_CHEVRON_UP, "˄", 12.0)
        } else {
            self.icon(ICON_CHEVRON_DOWN, "˅", 12.0)
        };
        let chevron = painter.layout_job(chevron);
        painter.galley(
            pos2(card.right() - 24.0, header.center().y) - chevron.size() / 2.0,
            chevron,
            p.text,
        );
        if response.clicked() {
            action = Some(SettingsAction::ToggleTheme);
        }
        if self.theme_expanded {
            painter.hline(
                card.x_range(),
                header.bottom(),
                Stroke::new(1.0, p.card_border),
            );
            for (i, (theme, label)) in THEMES.into_iter().enumerate() {
                let y = header.bottom() + MARGIN + (i as f32 + 0.5) * OPTION_HEIGHT;
                let id = Id::new("theme").with(i);
                if radio_button(
                    p,
                    ui,
                    pos2(card.left() + TEXT_X, y),
                    label,
                    self.theme == theme,
                    id,
                ) {
                    action = Some(SettingsAction::SetTheme(theme));
                }
            }
        }

        // About: the app's name, icon and version
        section_label(p, &painter, pos2(left, top + about_y), "About");
        let card = Rect::from_x_y_ranges(
            left..=right,
            top + about_top..=top + about_top + CARD_HEIGHT,
        );
        painter.rect_filled(card, RADIUS, p.card);
        painter.rect_stroke(
            card,
            RADIUS,
            Stroke::new(1.0, p.card_border),
            StrokeKind::Inside,
        );
        let icon = Rect::from_center_size(
            pos2(card.left() + ICON_X, card.center().y),
            vec2(16.0, 16.0),
        );
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        painter.image(self.app_icon.id(), icon, uv, Color32::WHITE);
        painter.text(
            pos2(card.left() + TEXT_X, card.center().y),
            Align2::LEFT_CENTER,
            "CalcCraft",
            body.clone(),
            p.text,
        );
        painter.text(
            pos2(card.right() - 20.0, card.center().y),
            Align2::RIGHT_CENTER,
            env!("CARGO_PKG_VERSION"),
            body.clone(),
            p.text_secondary,
        );

        let feedback = pos2(left, top + feedback_y);
        if link_button(p, ui, feedback, "Send feedback", Id::new("feedback")) {
            action = Some(SettingsAction::OpenUrl(FEEDBACK_URL));
        }

        // A sentence with an inline link: find the link's glyphs to make them clickable.
        let start = prefix.chars().count();
        let link = char_range_rect(&contribute, start..start + link_text.chars().count());
        let origin = pos2(left, top + contribute_top);
        let response = ui
            .interact(
                link.translate(origin.to_vec2()),
                Id::new("github"),
                Sense::CLICK,
            )
            .on_hover_cursor(CursorIcon::PointingHand);
        if response.is_pointer_button_down_on() {
            contribute = paragraph(p.link_pressed);
        } else if response.hovered() {
            contribute = paragraph(p.link_hover);
        }
        painter.galley(origin, contribute, p.text);
        if response.clicked() {
            action = Some(SettingsAction::OpenUrl(REPOSITORY_URL));
        }
        action
    }
}

#[derive(Clone, Copy)]
enum SettingsAction {
    ToggleTheme,
    SetTheme(Theme),
    OpenUrl(&'static str),
}

#[derive(Clone, Copy)]
enum FlyoutAction {
    RecallHistory(usize),
    RecallMemory(usize),
    MemoryAdd(usize, bool),
    MemoryClear(usize),
    ClearAll,
    OpenMenu(MenuTarget, Pos2),
    CopyHistory(usize),
    DeleteHistory(usize),
    CloseMenu,
}

/// The item a context menu belongs to.
#[derive(Clone, Copy)]
enum MenuTarget {
    History(usize),
    Memory(usize),
}

/// An open context menu and where the right click was.
#[derive(Clone, Copy)]
struct ContextMenu {
    target: MenuTarget,
    pos: Pos2,
}

impl eframe::App for CalculatorApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        storage.set_string(THEME_KEY, self.theme.key().to_owned());
        storage.set_string(CONVERTER_KEY, self.converter.converter.preferences());
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // egui resolves "Use system setting" and reports when Windows switches modes.
        let theme = ctx.theme();
        self.palette = palette_for(theme);
        if self.title_bar_theme != Some(theme) {
            self.title_bar.apply(theme);
            self.title_bar_theme = Some(theme);
        }
        match self.mode {
            _ if self.page == Page::Settings => self.handle_keyboard(&ctx),
            Mode::DateCalculation => self.handle_date_keyboard(&ctx),
            Mode::Converter(_) => self.handle_converter_keyboard(&ctx),
            Mode::Standard => self.handle_keyboard(&ctx),
        }
        let now = ctx.input(|i| i.time);

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, self.palette.background);
        if self.page == Page::Settings {
            self.draw_settings(ui, full);
            return;
        }

        if self.mode != Mode::Standard {
            let header = Rect::from_min_size(full.min, vec2(full.width(), HEADER_HEIGHT));
            self.draw_header(ui, header);
            if self.mode == Mode::DateCalculation {
                self.draw_date_mode(ui, full, header.bottom());
            } else {
                self.draw_converter_mode(ui, full, header.bottom());
            }
            if self.flyout == Flyout::Navigation {
                self.draw_navigation(ui, full);
            }
            return;
        }

        let header_height = if self.compact {
            COMPACT_HEADER_HEIGHT
        } else {
            HEADER_HEIGHT
        };
        let memory_height = if self.compact { 0.0 } else { MEMORY_ROW_HEIGHT };
        let header = Rect::from_min_size(full.min, vec2(full.width(), header_height));
        let below = full.with_min_y(header.bottom());
        let display_height = ((below.height() - memory_height) * 0.24).clamp(70.0, 220.0);
        let display = Rect::from_min_size(below.min, vec2(full.width(), display_height));
        let memory_row =
            Rect::from_min_size(display.left_bottom(), vec2(full.width(), memory_height));
        let keypad = Rect::from_min_max(
            memory_row.left_bottom() + vec2(MARGIN, GAP),
            full.max - vec2(MARGIN, MARGIN),
        );

        self.draw_header(ui, header);
        self.draw_display(ui, display);
        match self.flyout {
            Flyout::History | Flyout::Memory => {
                if ui
                    .interact(display, Id::new("display"), Sense::CLICK)
                    .clicked()
                {
                    self.flyout = Flyout::None;
                }
                self.draw_flyout(ui, below.with_min_y(display.bottom()));
            }
            Flyout::None | Flyout::Navigation => {
                self.context_menu = None;
                if !self.compact {
                    self.draw_memory_row(ui, memory_row, now);
                }
                self.draw_keypad(ui, keypad, now);
            }
        }
        if self.flyout == Flyout::Navigation {
            self.draw_navigation(ui, full);
        }
    }
}

// ------------------------------------------------------------------- text

fn semibold() -> FontFamily {
    FontFamily::Name("semibold".into())
}

/// Segoe UI Light, for the converter value that isn't being typed into.
fn light() -> FontFamily {
    FontFamily::Name("light".into())
}

/// Segoe UI Symbol, for the pictures of the converters' everyday comparisons.
fn symbols() -> FontFamily {
    FontFamily::Name("symbols".into())
}

fn icons() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// Text without a color: the color is picked at paint time from the button state.
fn text(text: &str, size: f32, family: FontFamily) -> LayoutJob {
    LayoutJob::simple_singleline(
        text.to_owned(),
        FontId::new(size, family),
        Color32::PLACEHOLDER,
    )
}

/// The `±` key's label, "⁺∕₋": a small plus at the top, a minus at the bottom.
fn negate_label(size: f32) -> LayoutJob {
    let regular = FontFamily::Proportional;
    let small = size * 0.85;
    let mut job = LayoutJob::default();
    job.append(
        "+",
        0.0,
        TextFormat {
            valign: Align::TOP,
            ..format(small, regular.clone())
        },
    );
    job.append("∕", 0.0, format(size, regular.clone()));
    job.append(
        "−",
        0.0,
        TextFormat {
            valign: Align::BOTTOM,
            ..format(small, regular)
        },
    );
    job
}

/// Format of one piece of a label; vertically centered by default.
fn format(size: f32, family: FontFamily) -> TextFormat {
    TextFormat {
        font_id: FontId::new(size, family),
        color: Color32::PLACEHOLDER,
        valign: Align::Center,
        ..Default::default()
    }
}

/// If the text doesn't fit, cuts it from the left: `…+ sqr(5)`.
fn layout_elided_left(painter: &Painter, text: &str, font: FontId, max_width: f32) -> Arc<Galley> {
    let mut galley = painter.layout_no_wrap(text.to_owned(), font.clone(), Color32::PLACEHOLDER);
    let mut start = 0;
    while galley.size().x > max_width {
        let Some((offset, _)) = text[start..].char_indices().nth(1) else {
            break;
        };
        start += offset;
        galley = painter.layout_no_wrap(
            format!("…{}", &text[start..]),
            font.clone(),
            Color32::PLACEHOLDER,
        );
    }
    galley
}

/// The bounding rectangle of the characters in `range`, relative to the galley.
fn char_range_rect(galley: &Galley, range: std::ops::Range<usize>) -> Rect {
    // Every character, spaces included, is one glyph in the rows.
    let glyphs = galley.rows.iter().flat_map(|row| {
        row.glyphs
            .iter()
            .map(|glyph| glyph.logical_rect().translate(row.pos.to_vec2()))
    });
    glyphs
        .enumerate()
        .filter(|(i, _)| range.contains(i))
        .fold(Rect::NOTHING, |rect, (_, glyph)| rect.union(glyph))
}

// ------------------------------------------------------- settings controls

/// Section heading on the Settings page, vertically centered on `left_center`.
fn section_label(p: &Palette, painter: &Painter, left_center: Pos2, label: &str) {
    painter.text(
        left_center,
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, semibold()),
        p.text,
    );
}

/// WinUI-style radio button: a 20 pt circle whose left edge is at `left_center`, then the label.
/// Returns `true` if it was clicked.
fn radio_button(
    p: &Palette,
    ui: &Ui,
    left_center: Pos2,
    label: &str,
    checked: bool,
    id: Id,
) -> bool {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), p.text);
    let text_pos = pos2(left_center.x + 28.0, left_center.y - galley.size().y / 2.0);
    let rect = Rect::from_min_max(
        pos2(left_center.x, left_center.y - 16.0),
        pos2(text_pos.x + galley.size().x, left_center.y + 16.0),
    );
    let response = ui.interact(rect, id, Sense::CLICK);
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();

    let center = pos2(left_center.x + 10.0, left_center.y);
    if checked || pressed {
        let fill = if pressed {
            p.accent_pressed
        } else if hovered {
            p.accent_hover
        } else {
            p.accent
        };
        // The dot grows on hover and shrinks while pressed, as in WinUI.
        let dot = if pressed {
            5.0
        } else if hovered {
            7.0
        } else {
            6.0
        };
        painter.circle_filled(center, 10.0, fill);
        painter.circle_filled(center, dot, p.radio_dot);
    } else {
        let fill = if hovered {
            p.subtle_hover
        } else {
            p.radio_fill
        };
        painter.circle(center, 9.5, fill, Stroke::new(1.0, p.radio_ring));
    }
    painter.galley(text_pos, galley, p.text);
    response.clicked()
}

/// A text link with a subtle background on hover, like a WinUI hyperlink button.
/// The text starts at `left_center`. Returns `true` if it was clicked.
fn link_button(p: &Palette, ui: &Ui, left_center: Pos2, label: &str, id: Id) -> bool {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(
        label.to_owned(),
        FontId::proportional(14.0),
        Color32::PLACEHOLDER,
    );
    let text_pos = pos2(left_center.x, left_center.y - galley.size().y / 2.0);
    let rect = Rect::from_min_size(text_pos, galley.size()).expand2(vec2(4.0, 2.0));
    let response = ui
        .interact(rect, id, Sense::CLICK)
        .on_hover_cursor(CursorIcon::PointingHand);
    let (fill, color) = if response.is_pointer_button_down_on() {
        (p.subtle_pressed, p.link_pressed)
    } else if response.hovered() {
        (p.subtle_hover, p.link_hover)
    } else {
        (Color32::TRANSPARENT, p.link)
    };
    painter.rect_filled(rect, RADIUS, fill);
    painter.galley(text_pos, galley, color);
    response.clicked()
}

/// Keys that arrive as text: digits and operators, as in Windows.
fn char_command(c: char) -> Option<Command> {
    Some(match c {
        '0'..='9' => Command::Digit(c as u8 - b'0'),
        '.' | ',' => Command::Decimal,
        '+' => Command::Operator(Op::Add),
        '-' => Command::Operator(Op::Subtract),
        '*' => Command::Operator(Op::Multiply),
        '/' => Command::Operator(Op::Divide),
        '=' => Command::Equals,
        '%' => Command::Percent,
        '@' => Command::Function(Func::SquareRoot),
        'r' | 'R' => Command::Function(Func::Reciprocal),
        'q' | 'Q' => Command::Function(Func::Square),
        _ => return None,
    })
}

/// Loads Windows fonts: Segoe UI, Segoe UI Semibold and the icon font.
/// Returns `true` if the icon font was found.
fn setup_fonts(ctx: &Context) -> bool {
    let dir =
        std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".into()))
            .join("Fonts");
    let load = |file: &str| std::fs::read(dir.join(file)).ok();

    let mut fonts = egui::FontDefinitions::default();
    let mut add = |name: &str, data: Vec<u8>| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(egui::FontData::from_owned(data)));
        name.to_owned()
    };
    let regular = load("segoeui.ttf").map(|d| add("segoe", d));
    let semibold_font = load("seguisb.ttf").map(|d| add("segoe-semibold", d));
    let light_font = load("segoeuil.ttf").map(|d| add("segoe-light", d));
    let symbol_font = load("seguisym.ttf").map(|d| add("segoe-symbol", d));
    let icon_font = load("SegoeIcons.ttf")
        .or_else(|| load("segmdl2.ttf"))
        .map(|d| add("icons", d));
    let has_icons = icon_font.is_some();

    let proportional = fonts.families.entry(FontFamily::Proportional).or_default();
    if let Some(name) = regular {
        proportional.insert(0, name);
    }
    let fallback = proportional.clone();
    let family =
        |first: Option<String>| first.into_iter().chain(fallback.iter().cloned()).collect();
    fonts.families.insert(semibold(), family(semibold_font));
    fonts.families.insert(light(), family(light_font));
    fonts.families.insert(symbols(), family(symbol_font));
    fonts.families.insert(icons(), family(icon_font));

    ctx.set_fonts(fonts);
    has_icons
}

/// The system title bar. Like the Windows 11 calculator, the app tints it with
/// the background color and switches its buttons between dark and light.
struct TitleBar {
    /// The native window handle (an `HWND`), if there is one.
    #[cfg(windows)]
    hwnd: Option<isize>,
}

impl TitleBar {
    #[cfg(windows)]
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let hwnd = cc
            .window_handle()
            .ok()
            .and_then(|handle| match handle.as_raw() {
                RawWindowHandle::Win32(handle) => Some(handle.hwnd.get()),
                _ => None,
            });
        Self { hwnd }
    }

    #[cfg(not(windows))]
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {}
    }

    #[cfg(windows)]
    fn apply(&self, theme: egui::Theme) {
        use windows_sys::Win32::Graphics::Dwm::{
            DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
        };

        let Some(hwnd) = self.hwnd else {
            return;
        };
        let hwnd = hwnd as windows_sys::Win32::Foundation::HWND;
        let dark_mode = u32::from(theme == egui::Theme::Dark);
        // COLORREF stores the color as 0x00BBGGRR.
        let [r, g, b, _] = palette_for(theme).background.to_array();
        let caption = u32::from_le_bytes([r, g, b, 0]);
        // SAFETY: hwnd is eframe's window, which lives as long as the app; the pointers point
        // to local u32 values, the size both attributes expect.
        unsafe {
            let set = |attribute: i32, value: &u32| {
                DwmSetWindowAttribute(hwnd, attribute as u32, (value as *const u32).cast(), 4);
            };
            set(DWMWA_USE_IMMERSIVE_DARK_MODE, &dark_mode);
            set(DWMWA_CAPTION_COLOR, &caption);
        }
    }

    #[cfg(not(windows))]
    fn apply(&self, _theme: egui::Theme) {}
}

/// The window icon, drawn by `icon.rs` (the executable embeds the same picture).
pub fn window_icon() -> egui::IconData {
    const SIZE: u32 = 64;
    egui::IconData {
        rgba: crate::icon::rgba(SIZE),
        width: SIZE,
        height: SIZE,
    }
}
