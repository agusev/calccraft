//! User interface styled after the Windows 11 calculator (dark theme).

mod date_view;

use std::sync::Arc;
use std::time::Duration;

use eframe::egui::{
    self, Align, Align2, Color32, Context, CornerRadius, Event, FontFamily, FontId, Galley, Id,
    Key, Modifiers, Painter, Rect, Sense, Stroke, StrokeKind, TextFormat, Ui, UiBuilder,
    ViewportCommand, WindowLevel, pos2, text::LayoutJob, vec2,
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
const NAV_PANEL: Color32 = Color32::from_rgb(0x35, 0x2A, 0x38);
const NAV_ITEM: Color32 = Color32::from_rgb(0x3D, 0x31, 0x41);
const NAV_ITEM_HOVER: Color32 = Color32::from_rgb(0x44, 0x38, 0x48);

// Glyphs from the Segoe Fluent Icons / Segoe MDL2 Assets fonts.
const ICON_MENU: char = '\u{E700}';
const ICON_CHEVRON_DOWN: char = '\u{E70D}';
const ICON_DELETE: char = '\u{E74D}';
const ICON_HISTORY: char = '\u{E81C}';
const ICON_CALENDAR: char = '\u{E787}';
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

/// Calculator mode, picked in the navigation menu.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Standard,
    DateCalculation,
}

impl Mode {
    fn title(self) -> &'static str {
        match self {
            Mode::Standard => "Standard",
            Mode::DateCalculation => "Date calculation",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flyout {
    None,
    History,
    Memory,
    Navigation,
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
    flyout: Flyout,
    /// "Keep on top" mode: a small always-on-top window without the memory row.
    compact: bool,
    full_size: Option<egui::Vec2>,
    /// Whether the Windows icon font was found.
    has_icons: bool,
    /// Command triggered from the keyboard and when, to highlight its button.
    key_flash: Option<(Command, f64)>,
}

impl CalculatorApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_theme(egui::Theme::Dark);
        let has_icons = setup_fonts(&cc.egui_ctx);
        #[cfg(windows)]
        tint_title_bar(cc);
        Self {
            mode: Mode::Standard,
            calc: Calculator::default(),
            date: date_view::DateState::new(),
            flyout: Flyout::None,
            compact: false,
            full_size: None,
            has_icons,
            key_flash: None,
        }
    }

    // -------------------------------------------------------------- keyboard

    fn handle_keyboard(&mut self, ctx: &Context) {
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

    fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.flyout = Flyout::None;
        self.date.close_popup();
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
            self.mode.title(),
            FontId::new(20.0, semibold()),
            TEXT,
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
            vec2((rect.width() * 0.85).min(256.0), rect.height()),
        );
        painter.rect_filled(panel, 0.0, NAV_PANEL);
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
        let modes = [
            (Mode::Standard, ICON_CALCULATOR, "▦"),
            (Mode::DateCalculation, ICON_CALENDAR, "▤"),
        ];
        for (i, (mode, glyph, fallback)) in modes.into_iter().enumerate() {
            let item = Rect::from_min_size(
                pos2(
                    panel.left() + 4.0,
                    panel.top() + HEADER_HEIGHT + 37.0 + i as f32 * 40.0,
                ),
                vec2(panel.width() - 8.0, 40.0),
            );
            let response = ui.interact(item, Id::new("nav-mode").with(i), Sense::CLICK);
            let selected = self.mode == mode;
            if response.hovered() {
                painter.rect_filled(item, RADIUS, NAV_ITEM_HOVER);
            } else if selected {
                painter.rect_filled(item, RADIUS, NAV_ITEM);
            }
            if selected {
                let pill = Rect::from_center_size(
                    pos2(item.left() + 2.0, item.center().y),
                    vec2(3.0, 16.0),
                );
                painter.rect_filled(pill, 2.0, ACCENT);
            }
            let icon = painter.layout_job(self.icon(glyph, fallback, 16.0));
            let icon_pos = pos2(item.left() + 20.0, item.center().y) - icon.size() / 2.0;
            painter.galley(icon_pos, icon, TEXT);
            painter.text(
                pos2(item.left() + 45.0, item.center().y),
                Align2::LEFT_CENTER,
                mode.title(),
                FontId::proportional(14.0),
                TEXT,
            );
            if response.clicked() {
                self.set_mode(mode);
                close = true;
            }
        }

        painter.text(
            pos2(panel.left() + 16.0, panel.bottom() - 16.0),
            Align2::LEFT_BOTTOM,
            "Calculator · Rust + egui",
            FontId::proportional(12.0),
            TEXT_DISABLED,
        );

        if close || backdrop.clicked() {
            self.flyout = Flyout::None;
        }
    }
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
        let full = ui.max_rect();
        ui.painter().rect_filled(full, 0.0, BACKGROUND);

        if self.mode == Mode::DateCalculation {
            self.handle_date_keyboard(&ctx);
            let header = Rect::from_min_size(full.min, vec2(full.width(), HEADER_HEIGHT));
            self.draw_header(ui, header);
            self.draw_date_mode(ui, full, header.bottom());
            if self.flyout == Flyout::Navigation {
                self.draw_navigation(ui, full);
            }
            return;
        }

        self.handle_keyboard(&ctx);
        let now = ctx.input(|i| i.time);

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
