//! User interface styled after the Windows 11 calculator (dark theme).

use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, Context, CornerRadius, CursorIcon, Event, FontFamily, FontId,
    Galley, Id, Key, Modifiers, Painter, Pos2, Rect, Sense, Stroke, StrokeKind, TextFormat,
    TextureHandle, TextureOptions, Ui, UiBuilder, ViewportCommand, WindowLevel, pos2,
    text::LayoutJob, vec2,
};

use crate::engine::{Calculator, Command, Func, Op, format_number};

// Colors sampled from a screenshot of the Windows 11 calculator.
const BACKGROUND: Color32 = Color32::from_rgb(0x29, 0x1A, 0x29);
const DIGIT: Color32 = Color32::from_rgb(0x43, 0x36, 0x44);
const DIGIT_HOVER: Color32 = Color32::from_rgb(0x4B, 0x3E, 0x4C);
const DIGIT_PRESSED: Color32 = Color32::from_rgb(0x3B, 0x2E, 0x3C);
const OPERATOR: Color32 = Color32::from_rgb(0x3B, 0x2D, 0x3C);
const OPERATOR_HOVER: Color32 = Color32::from_rgb(0x43, 0x36, 0x44);
const OPERATOR_PRESSED: Color32 = Color32::from_rgb(0x35, 0x27, 0x35);
const ACCENT: Color32 = Color32::from_rgb(0xDB, 0x9E, 0xE5);
const ACCENT_HOVER: Color32 = Color32::from_rgb(0xC9, 0x91, 0xD3);
const ACCENT_PRESSED: Color32 = Color32::from_rgb(0xB7, 0x84, 0xC1);
const SUBTLE_HOVER: Color32 = Color32::from_rgb(0x35, 0x27, 0x35);
const SUBTLE_PRESSED: Color32 = Color32::from_rgb(0x30, 0x22, 0x30);
const BORDER: Color32 = Color32::from_rgb(0x38, 0x2A, 0x39);
const FLYOUT: Color32 = Color32::from_rgb(0x2F, 0x21, 0x2F);
const FLYOUT_BORDER: Color32 = Color32::from_rgb(0x45, 0x37, 0x45);
const TEXT: Color32 = Color32::WHITE;
const TEXT_PRESSED: Color32 = Color32::from_rgb(0xCF, 0xC8, 0xCF);
const TEXT_SECONDARY: Color32 = Color32::from_rgb(0xC5, 0xBE, 0xC5);
const TEXT_DISABLED: Color32 = Color32::from_rgb(0x6C, 0x62, 0x6D);
const TEXT_ON_ACCENT: Color32 = Color32::from_rgb(0x1A, 0x0F, 0x1B);
// Settings page.
const CARD: Color32 = Color32::from_rgb(0x34, 0x26, 0x35);
const CARD_BORDER: Color32 = Color32::from_rgb(0x22, 0x18, 0x25);
const RADIO_RING: Color32 = Color32::from_rgb(0x9D, 0x97, 0x9F);
const RADIO_FILL: Color32 = Color32::from_rgb(0x2C, 0x23, 0x33);
const RADIO_DOT: Color32 = Color32::BLACK;
const LINK: Color32 = Color32::from_rgb(0xF0, 0xC0, 0xF4);
// Not sampled: the link color at 80% and 60% opacity over the background.
const LINK_HOVER: Color32 = Color32::from_rgb(0xC8, 0x9F, 0xCC);
const LINK_PRESSED: Color32 = Color32::from_rgb(0xA0, 0x7E, 0xA3);

// Glyphs from the Segoe Fluent Icons / Segoe MDL2 Assets fonts.
const ICON_MENU: char = '\u{E700}';
const ICON_CHEVRON_DOWN: char = '\u{E70D}';
const ICON_CHEVRON_UP: char = '\u{E70E}';
const ICON_SETTINGS: char = '\u{E713}';
const ICON_BACK: char = '\u{E72B}';
const ICON_DELETE: char = '\u{E74D}';
const ICON_THEME: char = '\u{E790}';
const ICON_HISTORY: char = '\u{E81C}';
const ICON_CALCULATOR: char = '\u{E8EF}';
const ICON_MULTIPLY: char = '\u{E947}';
const ICON_PLUS: char = '\u{E948}';
const ICON_MINUS: char = '\u{E949}';
const ICON_DIVIDE: char = '\u{E94A}';
const ICON_EQUALS: char = '\u{E94E}';
const ICON_BACKSPACE: char = '\u{E94F}';
const ICON_KEEP_ON_TOP: char = '\u{EE49}';
const ICON_BACK_TO_FULL: char = '\u{EE47}';

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flyout {
    None,
    History,
    Memory,
    Navigation,
}

/// What fills the window: the calculator or, as in Windows, the Settings page instead.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Calculator,
    Settings,
}

/// The "App theme" choice. It is only remembered for now: the colors are always the dark ones.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Theme {
    Light,
    Dark,
    System,
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
    calc: Calculator,
    flyout: Flyout,
    /// "Keep on top" mode: a small always-on-top window without the memory row.
    compact: bool,
    full_size: Option<egui::Vec2>,
    /// Whether the Windows icon font was found.
    has_icons: bool,
    /// Command triggered from the keyboard and when, to highlight its button.
    key_flash: Option<(Command, f64)>,
    page: Page,
    theme: Theme,
    /// Whether the "App theme" card on the Settings page shows its options.
    theme_expanded: bool,
    /// The window icon, shown in the About card.
    app_icon: TextureHandle,
}

impl CalculatorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let has_icons = setup_fonts(&cc.egui_ctx);
        #[cfg(windows)]
        tint_title_bar(cc);
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
        Self {
            calc: Calculator::default(),
            flyout: Flyout::None,
            compact: false,
            full_size: None,
            has_icons,
            key_flash: None,
            page: Page::Calculator,
            theme: Theme::System,
            theme_expanded: false,
            app_icon,
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
            Kind::Digit => pick(DIGIT, DIGIT_HOVER, DIGIT_PRESSED),
            Kind::Operator => pick(OPERATOR, OPERATOR_HOVER, OPERATOR_PRESSED),
            Kind::Accent if !enabled => DIGIT,
            Kind::Accent => pick(ACCENT, ACCENT_HOVER, ACCENT_PRESSED),
            Kind::Subtle => pick(Color32::TRANSPARENT, SUBTLE_HOVER, SUBTLE_PRESSED),
        };
        let text = if !enabled {
            TEXT_DISABLED
        } else if kind == Kind::Accent {
            TEXT_ON_ACCENT
        } else if pressed {
            TEXT_PRESSED
        } else {
            TEXT
        };

        let painter = ui.painter();
        painter.rect_filled(rect, RADIUS, fill);
        if matches!(kind, Kind::Digit | Kind::Operator) {
            painter.rect_stroke(rect, RADIUS, Stroke::new(1.0, BORDER), StrokeKind::Inside);
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
            "Standard",
            FontId::new(20.0, semibold()),
            TEXT,
        );

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
                TEXT_SECONDARY,
            );
        }

        // Long numbers shrink to fit the width.
        let display = self.calc.display();
        let base_size = (rect.height() * 0.46).clamp(28.0, 72.0);
        let mut galley =
            painter.layout_no_wrap(display.clone(), FontId::new(base_size, semibold()), TEXT);
        if galley.size().x > max_width {
            let size = (base_size * max_width / galley.size().x).max(10.0);
            galley = painter.layout_no_wrap(display, FontId::new(size, semibold()), TEXT);
        }
        let center_y = rect.top() + rect.height() * 0.6;
        let pos = pos2(right - galley.size().x, center_y - galley.size().y / 2.0);
        painter.galley(pos, galley, TEXT);
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
            Command::Function(Func::Negate) => {
                // "⁺∕₋": a small plus at the top, a minus at the bottom.
                let small = text_size * 0.85;
                let mut job = LayoutJob::default();
                job.append(
                    "+",
                    0.0,
                    TextFormat {
                        valign: Align::TOP,
                        ..format(small, regular.clone())
                    },
                );
                job.append("∕", 0.0, format(text_size, regular.clone()));
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
        let painter = ui.painter();
        let top = CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(rect, top, FLYOUT);
        painter.rect_stroke(
            rect.expand2(vec2(1.0, 0.0)),
            top,
            Stroke::new(1.0, FLYOUT_BORDER),
            StrokeKind::Inside,
        );
        // Clicks on the panel's empty space must not fall through it.
        ui.interact(rect, Id::new("flyout"), Sense::CLICK);

        let history = self.flyout == Flyout::History;
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
                TEXT,
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
                            if self.history_item(ui, i) {
                                action = Some(FlyoutAction::RecallHistory(i));
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
            None => {}
        }
    }

    /// History entry: small expression, large result. Returns `true` on click.
    fn history_item(&self, ui: &mut Ui, index: usize) -> bool {
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
            painter.rect_filled(rect, RADIUS, SUBTLE_HOVER);
        }
        let right = rect.right() - 12.0;
        let y = rect.top() + 10.0;
        let expression_height = expression.size().y;
        painter.galley(
            pos2(right - expression.size().x, y),
            expression,
            TEXT_SECONDARY,
        );
        painter.galley(
            pos2(right - result.size().x, y + expression_height),
            result,
            TEXT,
        );
        response.clicked()
    }

    /// Memory slot; on hover it shows MC, M+ and M− buttons.
    fn memory_item(&self, ui: &mut Ui, index: usize) -> Option<FlyoutAction> {
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
            painter.rect_filled(rect, RADIUS, SUBTLE_HOVER);
        }
        let right = rect.right() - 12.0;
        let result_height = result.size().y;
        painter.galley(
            pos2(right - result.size().x, rect.top() + 10.0),
            result,
            TEXT,
        );

        let mut action = response
            .clicked()
            .then_some(FlyoutAction::RecallMemory(index));
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

    // ------------------------------------------------------------------ menu

    fn draw_navigation(&mut self, ui: &Ui, rect: Rect) {
        let painter = ui.painter();
        painter.rect_filled(rect, 0.0, Color32::from_black_alpha(90));
        let backdrop = ui.interact(rect, Id::new("nav-backdrop"), Sense::CLICK);

        let panel = Rect::from_min_size(
            rect.min,
            vec2((rect.width() * 0.85).min(280.0), rect.height()),
        );
        painter.rect_filled(panel, 0.0, FLYOUT);
        painter.vline(
            panel.right(),
            panel.y_range(),
            Stroke::new(1.0, FLYOUT_BORDER),
        );
        ui.interact(panel, Id::new("nav-panel"), Sense::CLICK);

        let menu = Rect::from_center_size(
            pos2(panel.left() + 24.0, panel.top() + HEADER_HEIGHT / 2.0),
            vec2(40.0, 36.0),
        );
        let label = self.icon(ICON_MENU, "≡", 16.0);
        let mut close = self.button(ui, menu, Id::new("nav-menu"), Kind::Subtle, true, label);

        painter.text(
            pos2(panel.left() + 16.0, panel.top() + HEADER_HEIGHT + 8.0),
            Align2::LEFT_TOP,
            "Calculator",
            FontId::new(14.0, semibold()),
            TEXT,
        );
        // The only mode is "Standard", and it is selected.
        let item = Rect::from_min_size(
            pos2(panel.left() + 4.0, panel.top() + HEADER_HEIGHT + 34.0),
            vec2(panel.width() - 8.0, 40.0),
        );
        let icon = self.icon(ICON_CALCULATOR, "▦", 16.0);
        close |= self.nav_item(ui, item, Id::new("nav-standard"), icon, "Standard", true);

        // Settings sits at the bottom of the pane, below a separator, as in Windows.
        let item = Rect::from_min_size(
            pos2(panel.left() + 4.0, panel.bottom() - 46.0),
            vec2(panel.width() - 8.0, 40.0),
        );
        painter.hline(
            panel.x_range(),
            item.top() - 12.0,
            Stroke::new(1.0, FLYOUT_BORDER),
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
        let painter = ui.painter();
        let response = ui.interact(rect, id, Sense::CLICK);
        let fill = if response.is_pointer_button_down_on() || (selected && !response.hovered()) {
            SUBTLE_HOVER
        } else if response.hovered() {
            OPERATOR_HOVER
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(rect, RADIUS, fill);
        if selected {
            let pill =
                Rect::from_center_size(pos2(rect.left() + 2.0, rect.center().y), vec2(3.0, 16.0));
            painter.rect_filled(pill, 2.0, ACCENT);
        }
        let icon = painter.layout_job(icon);
        painter.galley(
            pos2(rect.left() + 16.0, rect.center().y - icon.size().y / 2.0),
            icon,
            TEXT,
        );
        painter.text(
            pos2(rect.left() + 48.0, rect.center().y),
            Align2::LEFT_CENTER,
            label,
            FontId::proportional(14.0),
            TEXT,
        );
        response.clicked()
    }

    // -------------------------------------------------------------- settings

    fn draw_settings(&mut self, ui: &mut Ui, rect: Rect) {
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
            TEXT,
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
            Some(SettingsAction::SetTheme(theme)) => self.theme = theme,
            Some(SettingsAction::OpenUrl(url)) => ui.ctx().open_url(egui::OpenUrl::new_tab(url)),
            None => {}
        }
    }

    /// The scrolling part of the Settings page: the Appearance and About sections and the links.
    fn settings_content(&self, ui: &mut Ui) -> Option<SettingsAction> {
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
        let title = painter.layout("App theme".into(), body.clone(), TEXT, text_width);
        let subtitle = painter.layout(
            "Select which app theme to display".into(),
            FontId::proportional(12.0),
            TEXT_SECONDARY,
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
            for (part, color) in [(prefix, TEXT), (link_text, link_color), (".", TEXT)] {
                job.append(part, 0.0, TextFormat::simple(body.clone(), color));
            }
            painter.layout_job(job)
        };
        let mut contribute = paragraph(LINK);
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
        section_label(&painter, pos2(left, top + appearance_y), "Appearance");
        let card = Rect::from_x_y_ranges(left..=right, top + theme_top..=top + theme_bottom);
        let header = card.with_max_y(card.top() + header_height);
        let response = ui.interact(header, Id::new("theme-card"), Sense::CLICK);
        let fill = if response.is_pointer_button_down_on() {
            SUBTLE_PRESSED
        } else if response.hovered() {
            OPERATOR
        } else {
            CARD
        };
        painter.rect_filled(card, RADIUS, CARD);
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
            Stroke::new(1.0, CARD_BORDER),
            StrokeKind::Inside,
        );
        let icon = painter.layout_job(self.icon(ICON_THEME, "◐", 20.0));
        painter.galley(
            pos2(card.left() + ICON_X, header.center().y) - icon.size() / 2.0,
            icon,
            TEXT,
        );
        let text_top = header.center().y - (title.size().y + subtitle.size().y) / 2.0;
        let title_height = title.size().y;
        painter.galley(pos2(card.left() + TEXT_X, text_top), title, TEXT);
        painter.galley(
            pos2(card.left() + TEXT_X, text_top + title_height),
            subtitle,
            TEXT_SECONDARY,
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
            TEXT,
        );
        if response.clicked() {
            action = Some(SettingsAction::ToggleTheme);
        }
        if self.theme_expanded {
            painter.hline(
                card.x_range(),
                header.bottom(),
                Stroke::new(1.0, CARD_BORDER),
            );
            for (i, (theme, label)) in THEMES.into_iter().enumerate() {
                let y = header.bottom() + MARGIN + (i as f32 + 0.5) * OPTION_HEIGHT;
                let id = Id::new("theme").with(i);
                if radio_button(
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
        section_label(&painter, pos2(left, top + about_y), "About");
        let card = Rect::from_x_y_ranges(
            left..=right,
            top + about_top..=top + about_top + CARD_HEIGHT,
        );
        painter.rect_filled(card, RADIUS, CARD);
        painter.rect_stroke(
            card,
            RADIUS,
            Stroke::new(1.0, CARD_BORDER),
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
            TEXT,
        );
        painter.text(
            pos2(card.right() - 20.0, card.center().y),
            Align2::RIGHT_CENTER,
            env!("CARGO_PKG_VERSION"),
            body.clone(),
            TEXT_SECONDARY,
        );

        let feedback = pos2(left, top + feedback_y);
        if link_button(ui, feedback, "Send feedback", Id::new("feedback")) {
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
            contribute = paragraph(LINK_PRESSED);
        } else if response.hovered() {
            contribute = paragraph(LINK_HOVER);
        }
        painter.galley(origin, contribute, TEXT);
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
}

impl eframe::App for CalculatorApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_keyboard(&ctx);
        let now = ctx.input(|i| i.time);

        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, BACKGROUND);
        if self.page == Page::Settings {
            self.draw_settings(ui, full);
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
fn section_label(painter: &Painter, left_center: Pos2, label: &str) {
    painter.text(
        left_center,
        Align2::LEFT_CENTER,
        label,
        FontId::new(14.0, semibold()),
        TEXT,
    );
}

/// WinUI-style radio button: a 20 pt circle whose left edge is at `left_center`, then the label.
/// Returns `true` if it was clicked.
fn radio_button(ui: &Ui, left_center: Pos2, label: &str, checked: bool, id: Id) -> bool {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(label.to_owned(), FontId::proportional(14.0), TEXT);
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
            ACCENT_PRESSED
        } else if hovered {
            ACCENT_HOVER
        } else {
            ACCENT
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
        painter.circle_filled(center, dot, RADIO_DOT);
    } else {
        let fill = if hovered { SUBTLE_HOVER } else { RADIO_FILL };
        painter.circle(center, 9.5, fill, Stroke::new(1.0, RADIO_RING));
    }
    painter.galley(text_pos, galley, TEXT);
    response.clicked()
}

/// A text link with a subtle background on hover, like a WinUI hyperlink button.
/// The text starts at `left_center`. Returns `true` if it was clicked.
fn link_button(ui: &Ui, left_center: Pos2, label: &str, id: Id) -> bool {
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
        (SUBTLE_PRESSED, LINK_PRESSED)
    } else if response.hovered() {
        (SUBTLE_HOVER, LINK_HOVER)
    } else {
        (Color32::TRANSPARENT, LINK)
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
    fonts.families.insert(icons(), family(icon_font));

    ctx.set_fonts(fonts);
    has_icons
}

/// Tints the system title bar with the background color, like the Windows 11 calculator.
#[cfg(windows)]
fn tint_title_bar(cc: &eframe::CreationContext<'_>) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_CAPTION_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute,
    };

    let Ok(handle) = cc.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return;
    };
    let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
    let dark_mode: i32 = 1;
    // COLORREF stores the color as 0x00BBGGRR.
    let [r, g, b, _] = BACKGROUND.to_array();
    let caption = u32::from_le_bytes([r, g, b, 0]);
    // SAFETY: hwnd is eframe's live window; the pointers point to local u32/i32 values of the right size.
    unsafe {
        let set = |attribute: i32, value: *const u32| {
            DwmSetWindowAttribute(hwnd, attribute as u32, value.cast(), 4);
        };
        set(
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark_mode as *const i32).cast(),
        );
        set(DWMWA_CAPTION_COLOR, &caption);
    }
}

/// Window icon: a small calculator drawn pixel by pixel.
pub fn window_icon() -> egui::IconData {
    const SIZE: usize = 64;
    let inside = |x: f32, y: f32, rect: Rect, radius: f32| {
        let dx = (rect.left() + radius - x)
            .max(x - (rect.right() - radius))
            .max(0.0);
        let dy = (rect.top() + radius - y)
            .max(y - (rect.bottom() - radius))
            .max(0.0);
        dx * dx + dy * dy <= radius * radius && rect.contains(pos2(x, y))
    };
    let body = Rect::from_min_max(pos2(10.0, 4.0), pos2(54.0, 60.0));
    let screen = Rect::from_min_max(pos2(16.0, 10.0), pos2(48.0, 22.0));
    let key = |row: usize, col: usize| {
        let min = pos2(16.0 + col as f32 * 11.5, 27.0 + row as f32 * 8.5);
        Rect::from_min_size(min, vec2(9.0, 6.5))
    };
    let color_at = |x: f32, y: f32| -> Color32 {
        if !inside(x, y, body, 7.0) {
            return Color32::TRANSPARENT;
        }
        if inside(x, y, screen, 2.0) {
            return ACCENT;
        }
        for row in 0..4 {
            for col in 0..3 {
                if inside(x, y, key(row, col), 1.5) {
                    return if (row, col) == (3, 2) {
                        ACCENT
                    } else {
                        Color32::from_gray(0xE6)
                    };
                }
            }
        }
        DIGIT
    };

    // Anti-aliasing: 4×4 samples per pixel.
    let mut rgba = Vec::with_capacity(SIZE * SIZE * 4);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let mut sum = [0u32; 4];
            for sy in 0..4 {
                for sx in 0..4 {
                    let c = color_at(
                        x as f32 + (sx as f32 + 0.5) / 4.0,
                        y as f32 + (sy as f32 + 0.5) / 4.0,
                    );
                    let alpha = c.a() as u32;
                    sum[0] += c.r() as u32 * alpha;
                    sum[1] += c.g() as u32 * alpha;
                    sum[2] += c.b() as u32 * alpha;
                    sum[3] += alpha;
                }
            }
            let alpha = sum[3];
            let channel = |v: u32| v.checked_div(alpha).map_or(0, |c| c as u8);
            rgba.extend([
                channel(sum[0]),
                channel(sum[1]),
                channel(sum[2]),
                (alpha / 16) as u8,
            ]);
        }
    }
    egui::IconData {
        rgba,
        width: SIZE as u32,
        height: SIZE as u32,
    }
}
