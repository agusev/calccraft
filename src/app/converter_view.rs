//! The unit converters: two values with their unit drop-downs, and a keypad. The
//! layout and sizes follow `UnitConverter.xaml` from the Windows Calculator source.

use std::sync::Arc;

use eframe::egui::{
    self, Context, Event, FontFamily, FontId, Galley, Id, Key, Rect, Sense, Ui, UiBuilder, pos2,
    text::LayoutJob, vec2,
};

use super::date_view::{list_item, popup_frame};
use super::{
    CalculatorApp, Flyout, GAP, ICON_BACKSPACE, ICON_CHEVRON_DOWN, KEY_FLASH, Kind, MARGIN, icons,
    layout_elided_left, light, negate_label, semibold, symbols, text,
};
use crate::converter::{Converter, ConverterCommand, Region};

/// From this window width on, the values and the keypad get larger (`Wide640`).
const WIDE_WIDTH: f32 = 640.0;
/// From this window size on, the digits grow again (`Wide`).
const HUGE_WIDTH: f32 = 1280.0;
const HUGE_HEIGHT: f32 = 768.0;
/// The row under the values with the "About equal to" results (`RowDltrUnits`).
const RESULTS_ROW_HEIGHT: f32 = 48.0;
const LIST_ITEM_HEIGHT: f32 = 36.0;
/// WinUI's `MaxDropDownHeight` (504 px) in items.
const LIST_VISIBLE_ITEMS: usize = 14;

/// The converter and what its UI keeps.
pub(super) struct ConverterState {
    pub(super) converter: Converter,
    /// The open unit drop-down, if any.
    popup: Option<UnitPopup>,
    /// A key pressed on the keyboard and when, to highlight its button.
    flash: Option<(ConverterCommand, f64)>,
}

#[derive(Clone, Copy)]
struct UnitPopup {
    /// The top field's units (`true`) or the bottom field's.
    top: bool,
    /// The unit picker the list opened from.
    anchor: Rect,
    /// Scroll the selected unit into view on the first frame.
    scroll_pending: bool,
}

impl ConverterState {
    pub(super) fn new() -> Self {
        Self {
            converter: Converter::new(Region::new(&user_region())),
            popup: None,
            flash: None,
        }
    }

    /// Closes the unit drop-down. Returns whether one was open.
    pub(super) fn close_popup(&mut self) -> bool {
        self.popup.take().is_some()
    }
}

/// The user's region (Windows Settings → Time & language → Language & region), which
/// picks the default units as Windows' `GeographicRegion` does. `US` when unknown.
fn user_region() -> String {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultGeoName;
        let mut name = [0u16; 16];
        // SAFETY: the buffer and its length match; the call writes at most that many
        // characters, including the terminating zero it counts in its result.
        let length = unsafe { GetUserDefaultGeoName(name.as_mut_ptr(), name.len() as i32) };
        if length > 1 {
            return String::from_utf16_lossy(&name[..length as usize - 1]);
        }
    }
    "US".to_string()
}

/// The command a typed character stands for.
fn char_command(c: char) -> Option<ConverterCommand> {
    match c {
        '0'..='9' => Some(ConverterCommand::Digit(c as u8 - b'0')),
        '.' | ',' => Some(ConverterCommand::Decimal),
        '-' => Some(ConverterCommand::Negate),
        _ => None,
    }
}

/// The heights of rows sized like XAML star rows: in proportion to their weights,
/// but never below their minimum heights.
fn star_rows(total: f32, rows: &[(f32, f32)]) -> Vec<f32> {
    let mut heights = vec![0.0; rows.len()];
    let mut fixed = vec![false; rows.len()];
    loop {
        let free: f32 = total
            - (0..rows.len())
                .filter(|&i| fixed[i])
                .map(|i| heights[i])
                .sum::<f32>();
        let weight: f32 = (0..rows.len())
            .filter(|&i| !fixed[i])
            .map(|i| rows[i].0)
            .sum();
        let mut changed = false;
        for (i, &(star, min)) in rows.iter().enumerate() {
            if fixed[i] {
                continue;
            }
            let share = if weight > 0.0 {
                free.max(0.0) * star / weight
            } else {
                0.0
            };
            if share < min {
                heights[i] = min;
                fixed[i] = true;
                changed = true;
            } else {
                heights[i] = share;
            }
        }
        if !changed {
            return heights;
        }
    }
}

impl CalculatorApp {
    // -------------------------------------------------------------- keyboard

    pub(super) fn handle_converter_keyboard(&mut self, ctx: &Context) {
        let now = ctx.input(|i| i.time);
        for event in ctx.input(|i| i.events.clone()) {
            // An open drop-down takes the keyboard; Esc closes it.
            if self.converter.popup.is_some() {
                if matches!(
                    event,
                    Event::Key {
                        key: Key::Escape,
                        pressed: true,
                        ..
                    }
                ) {
                    self.converter.close_popup();
                }
                continue;
            }
            match event {
                Event::Text(text) => {
                    for command in text.chars().filter_map(char_command) {
                        self.converter_press(command, now);
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } if !modifiers.command => {
                    let command = match key {
                        Key::Escape => {
                            self.flyout = Flyout::None;
                            continue;
                        }
                        Key::Backspace => ConverterCommand::Backspace,
                        Key::Delete => ConverterCommand::Clear,
                        Key::F9 => ConverterCommand::Negate,
                        _ => continue,
                    };
                    self.converter_press(command, now);
                }
                Event::Copy => ctx.copy_text(self.converter.converter.copy_text()),
                Event::Paste(text) => {
                    self.converter.converter.paste(&text);
                }
                _ => {}
            }
        }
        if self.converter.flash.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(KEY_FLASH));
        }
    }

    fn converter_press(&mut self, command: ConverterCommand, now: f64) {
        // `±` is hidden where values can't be negative, so its key does nothing there.
        let hidden = command == ConverterCommand::Negate
            && !self.converter.converter.category().supports_negative();
        if !hidden {
            self.converter.converter.execute(command);
            self.converter.flash = Some((command, now));
        }
    }

    // ---------------------------------------------------------------- layout

    /// Draws the converter below the header. `full` is the whole window.
    pub(super) fn draw_converter_mode(&mut self, ui: &mut Ui, full: Rect, top: f32) {
        let now = ui.input(|i| i.time);
        let content = Rect::from_min_max(pos2(full.left(), top), full.max);
        let wide = full.width() >= WIDE_WIDTH;
        let huge = wide && full.width() >= HUGE_WIDTH && full.height() >= HUGE_HEIGHT;
        // In a landscape window the keypad moves next to the values (`LandscapeLayout`).
        let landscape = full.width() >= full.height();

        let (fields, keypad) = if landscape {
            let half = content.width() / 2.0;
            let fields = Rect::from_min_size(content.min, vec2(half, content.height()));
            let keypad =
                Rect::from_min_max(pos2(content.left() + half, content.top()), content.max);
            (fields, Some(keypad))
        } else {
            (content, None)
        };
        let heights = if landscape {
            star_rows(
                fields.height(),
                &[
                    (4.0, 56.0),
                    (2.0, 32.0),
                    (4.0, 56.0),
                    (2.0, 32.0),
                    (2.0, RESULTS_ROW_HEIGHT),
                ],
            )
        } else {
            let mut rows = star_rows(
                fields.height() - RESULTS_ROW_HEIGHT,
                &[
                    (56.0, 56.0),
                    (32.0, 32.0),
                    (56.0, 56.0),
                    (32.0, 32.0),
                    (272.0, 0.0),
                ],
            );
            rows.insert(4, RESULTS_ROW_HEIGHT);
            rows
        };
        let mut y = fields.top();
        let mut row = |i: usize| {
            let rect =
                Rect::from_min_size(pos2(fields.left(), y), vec2(fields.width(), heights[i]));
            y += heights[i];
            rect
        };
        let (value1, unit1, value2, unit2, results) = (row(0), row(1), row(2), row(3), row(4));
        let keypad = keypad.unwrap_or_else(|| row(5));

        let (max_font, value_margin, combo_height) = if wide {
            (46.0, 12.0, 44.0)
        } else {
            (40.0, 4.0, 32.0)
        };
        self.draw_value(ui, value1, true, max_font, value_margin);
        let anchor1 = self.draw_unit_picker(ui, unit1, true, combo_height);
        self.draw_value(ui, value2, false, max_font, value_margin);
        let anchor2 = self.draw_unit_picker(ui, unit2, false, combo_height);
        self.draw_suggestions(ui, results);
        for (top, anchor) in [(true, anchor1), (false, anchor2)] {
            if let Some(anchor) = anchor {
                self.converter.popup = Some(UnitPopup {
                    top,
                    anchor,
                    scroll_pending: true,
                });
            }
        }

        // `Margin="3,0,3,3"` around the keypad, and 1 px around each key.
        let keypad = Rect::from_min_max(
            keypad.min + vec2(MARGIN, 0.0),
            keypad.max - vec2(MARGIN, MARGIN),
        );
        let (digit_size, operator_size, negate_size) = if huge {
            (46.0, 24.0, 24.0)
        } else if wide {
            (28.0, 20.0, 20.0)
        } else {
            (18.0, 14.0, 16.0)
        };
        self.draw_converter_keypad(ui, keypad, now, digit_size, operator_size, negate_size);

        if let Some(popup) = self.converter.popup {
            self.draw_unit_list(ui, full, popup);
        }
    }

    /// One of the two values: left-aligned at the bottom of its row, in semibold
    /// while it's the one being typed into, shrinking to fit. A click activates it.
    fn draw_value(&mut self, ui: &Ui, rect: Rect, top: bool, max_font: f32, margin: f32) {
        let p = self.palette;
        let converter = &self.converter.converter;
        let value = if top {
            converter.value1()
        } else {
            converter.value2()
        };
        let active = converter.value1_active() == top;
        let family = if active { semibold() } else { light() };

        // The value sits in a 12 px padded box with a 4 px margin (`CalculationResult`).
        let left = rect.left() + 16.0;
        let max_width = rect.width() - 28.0;
        let painter = ui.painter();
        let measured = painter
            .layout_no_wrap(value.clone(), FontId::new(max_font, family.clone()), p.text)
            .size()
            .x;
        let size = if measured > max_width {
            (max_font * max_width / measured).max(14.0)
        } else {
            max_font
        };
        let galley = layout_elided_left(painter, &value, FontId::new(size, family), max_width);
        let pos = pos2(left, rect.bottom() - margin - galley.size().y);
        painter.galley(pos, galley, p.text);

        let id = Id::new("converter-value").with(top);
        if ui.interact(rect, id, Sense::CLICK).clicked() {
            self.converter.converter.activate(top);
        }
    }

    /// The unit drop-down of a value: the unit's name and a chevron, without a box.
    /// Returns the picker's rectangle when it was clicked.
    fn draw_unit_picker(&self, ui: &Ui, row: Rect, top: bool, height: f32) -> Option<Rect> {
        let p = self.palette;
        let converter = &self.converter.converter;
        let index = if top {
            converter.unit1()
        } else {
            converter.unit2()
        };
        let name = converter.units()[index].name;
        let painter = ui.painter();
        let label = painter.layout_no_wrap(name.to_string(), FontId::proportional(14.0), p.text);
        let chevron = painter.layout_job(if self.has_icons {
            text(&ICON_CHEVRON_DOWN.to_string(), 12.0, icons())
        } else {
            text("⌄", 12.0, FontFamily::Proportional)
        });
        // `Margin="12,0,0,0"`, `Padding="4,0,0,0"`, and the chevron's `Margin="8,3,12,0"`.
        let width = 4.0 + label.size().x + 8.0 + chevron.size().x + 12.0;
        let rect = Rect::from_min_size(pos2(row.left() + 12.0, row.top()), vec2(width, height));

        let response = ui.interact(rect, Id::new("converter-unit").with(top), Sense::CLICK);
        let color = if response.is_pointer_button_down_on() {
            p.unit_pressed
        } else if response.hovered() {
            p.unit_hover
        } else {
            p.text
        };
        let y = rect.center().y;
        painter.galley(
            pos2(rect.left() + 4.0, y - label.size().y / 2.0),
            label.clone(),
            color,
        );
        painter.galley(
            pos2(
                rect.left() + 4.0 + label.size().x + 8.0,
                y + 1.5 - chevron.size().y / 2.0,
            ),
            chevron,
            color,
        );
        response.clicked().then_some(rect)
    }

    /// The "About equal to" results (`SupplementaryResults.xaml`): a caption, then as
    /// many results as fit on one line, like `HorizontalNoOverflowStackPanel`.
    fn draw_suggestions(&self, ui: &Ui, row: Rect) {
        /// A laid-out result and its width with the margins.
        struct Item {
            glyph: Option<(Arc<Galley>, f32)>,
            value: Arc<Galley>,
            unit: Arc<Galley>,
            width: f32,
        }

        let p = self.palette;
        let suggestions = self.converter.converter.suggestions();
        if suggestions.is_empty() {
            return;
        }
        let painter = ui.painter();
        let caption = painter.layout_no_wrap(
            "About equal to".to_string(),
            FontId::proportional(12.0),
            p.text_secondary,
        );
        // The results' line overlaps the caption by 6 px and is 27 px high; the
        // texts sit at its bottom.
        let bottom = row.top() + caption.size().y - 6.0 + 27.0;
        painter.galley(
            pos2(row.left() + 16.0, row.top()),
            caption,
            p.text_secondary,
        );

        let items: Vec<Item> = suggestions
            .iter()
            .map(|s| {
                let value =
                    painter.layout_no_wrap(s.value.clone(), FontId::new(14.0, semibold()), p.text);
                let unit = painter.layout_no_wrap(
                    s.abbreviation.to_string(),
                    FontId::proportional(14.0),
                    p.text_secondary,
                );
                let texts = value.size().x + 4.0 + unit.size().x;
                match s.glyph {
                    // `Margin="16,0,4,0"` before the value and 16 px after the unit.
                    None => Item {
                        glyph: None,
                        width: 16.0 + texts + 16.0,
                        value,
                        unit,
                    },
                    // The picture, its own gap, and the value 2 px closer.
                    Some((glyph, gap)) => {
                        let glyph = painter.layout_no_wrap(
                            glyph.to_string(),
                            FontId::new(20.0, symbols()),
                            p.text,
                        );
                        Item {
                            width: glyph.size().x + gap - 2.0 + texts,
                            glyph: Some((glyph, gap)),
                            value,
                            unit,
                        }
                    }
                }
            })
            .collect();

        // Left to right while they fit; with more than two results, room is kept for
        // the comparison at the end, so the ones before it give way.
        let available = row.width() - 6.0;
        let last = items.len() - 1;
        let reserved = if items.len() > 2 && items[last].glyph.is_some() {
            items[last].width
        } else {
            0.0
        };
        let mut x = row.left();
        for (i, item) in items.into_iter().enumerate() {
            let mut room = available - (x - row.left());
            if i != last {
                room -= reserved;
            }
            if room <= 0.0 || item.width > room {
                continue;
            }
            let mut left = x;
            if let Some((glyph, gap)) = item.glyph {
                // `Margin="0,3,…,-3"`: the picture sits 3 px lower.
                let size = glyph.size();
                painter.galley(pos2(left, bottom + 3.0 - size.y), glyph, p.text);
                left += size.x + gap - 2.0;
            } else {
                left += 16.0;
            }
            let value_width = item.value.size().x;
            painter.galley(pos2(left, bottom - item.value.size().y), item.value, p.text);
            left += value_width + 4.0;
            painter.galley(
                pos2(left, bottom - item.unit.size().y),
                item.unit,
                p.text_secondary,
            );
            x += item.width;
        }
    }

    /// The units list of an open drop-down, with the selected unit over the picker
    /// where it fits, as in WinUI.
    fn draw_unit_list(&mut self, ui: &mut Ui, full: Rect, popup: UnitPopup) {
        let p = self.palette;
        let converter = &self.converter.converter;
        let units = converter.units();
        let selected = if popup.top {
            converter.unit1()
        } else {
            converter.unit2()
        };
        let backdrop = ui.interact(full, Id::new("converter-backdrop"), Sense::CLICK);

        let fits = ((full.height() - 16.0) / LIST_ITEM_HEIGHT).floor() as usize;
        let visible = units.len().min(LIST_VISIBLE_ITEMS).min(fits).max(1);
        let height = visible as f32 * LIST_ITEM_HEIGHT + 8.0;
        let widest = units
            .iter()
            .map(|unit| {
                ui.painter()
                    .layout_no_wrap(unit.name.to_string(), FontId::proportional(14.0), p.text)
                    .size()
                    .x
            })
            .fold(0.0, f32::max);
        let width = (widest + 48.0)
            .max(popup.anchor.width())
            .min(full.width() - 8.0);
        // Like WinUI, put the selected unit on the picker: a list that scrolls is
        // centered on the picker and scrolled there, a short one is moved there.
        // Both stay inside the window.
        let lowest_top = (full.bottom() - 4.0 - height).max(full.top() + 4.0);
        let fit = |top: f32| top.clamp(full.top() + 4.0, lowest_top);
        // Where the selected row's top edge would sit on the picker.
        let target = popup.anchor.center().y - LIST_ITEM_HEIGHT / 2.0 - 4.0;
        let max_offset = (units.len() - visible) as f32 * LIST_ITEM_HEIGHT;
        let centered = fit(target - (visible / 2) as f32 * LIST_ITEM_HEIGHT);
        let row = selected as f32 * LIST_ITEM_HEIGHT;
        let offset = (row - (target - centered)).clamp(0.0, max_offset);
        let top = fit(target - (row - offset));
        let left = popup
            .anchor
            .left()
            .min(full.right() - 4.0 - width)
            .max(full.left() + 4.0);
        let rect = Rect::from_min_size(pos2(left, top), vec2(width, height));
        popup_frame(p, ui.painter(), rect, p.popup);
        ui.interact(rect, Id::new("converter-unit-list"), Sense::CLICK);

        let mut chosen = None;
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(4.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut area = egui::ScrollArea::vertical()
                .id_salt(("converter-unit-list", popup.top))
                .auto_shrink(false);
            if popup.scroll_pending {
                area = area.vertical_scroll_offset(offset);
            }
            area.show_rows(ui, LIST_ITEM_HEIGHT, units.len(), |ui, range| {
                for index in range {
                    let size = vec2(ui.available_width(), LIST_ITEM_HEIGHT);
                    let (row, _) = ui.allocate_exact_size(size, Sense::hover());
                    let item = row.shrink2(vec2(0.0, 2.0));
                    let id = Id::new("converter-unit-item").with(index);
                    if list_item(p, ui, id, item, units[index].name, index == selected) {
                        chosen = Some(index);
                    }
                }
            });
        });

        if let Some(open) = &mut self.converter.popup {
            open.scroll_pending = false;
        }
        if let Some(index) = chosen {
            if popup.top {
                self.converter.converter.set_unit1(index);
            } else {
                self.converter.converter.set_unit2(index);
            }
            self.converter.popup = None;
        } else if backdrop.clicked() {
            self.converter.popup = None;
        }
    }

    /// The keypad: `CE` and `⌫` above the number pad, and `±` left of `0` where
    /// values can be negative.
    fn draw_converter_keypad(
        &mut self,
        ui: &Ui,
        rect: Rect,
        now: f64,
        digit_size: f32,
        operator_size: f32,
        negate_size: f32,
    ) {
        use ConverterCommand::*;
        #[rustfmt::skip]
        const KEYS: [Option<ConverterCommand>; 15] = [
            None, Some(Clear), Some(Backspace),
            Some(Digit(7)), Some(Digit(8)), Some(Digit(9)),
            Some(Digit(4)), Some(Digit(5)), Some(Digit(6)),
            Some(Digit(1)), Some(Digit(2)), Some(Digit(3)),
            Some(Negate), Some(Digit(0)), Some(Decimal),
        ];
        let negate = self.converter.converter.category().supports_negative();
        let width = (rect.width() - 2.0 * GAP) / 3.0;
        let height = (rect.height() - 4.0 * GAP) / 5.0;
        for (i, command) in KEYS.into_iter().enumerate() {
            let Some(command) = command else {
                continue;
            };
            if command == Negate && !negate {
                continue;
            }
            let (row, col) = ((i / 3) as f32, (i % 3) as f32);
            let cell = Rect::from_min_size(
                rect.min + vec2(col * (width + GAP), row * (height + GAP)),
                vec2(width, height),
            );
            let (kind, label): (Kind, LayoutJob) = match command {
                Digit(d) => (Kind::Digit, text(&d.to_string(), digit_size, semibold())),
                Decimal => (Kind::Digit, text(".", digit_size, semibold())),
                Negate => (Kind::Digit, negate_label(negate_size)),
                Clear => (
                    Kind::Operator,
                    text("CE", operator_size, FontFamily::Proportional),
                ),
                Backspace => (
                    Kind::Operator,
                    self.icon(ICON_BACKSPACE, "⌫", operator_size),
                ),
            };
            let flash =
                matches!(self.converter.flash, Some((c, t)) if c == command && now - t < KEY_FLASH);
            let id = Id::new("converter-key").with(i);
            if self.button_flash(ui, cell, id, kind, true, label, flash)
                // Like Windows, CE does nothing while a drop-down is open.
                && !(command == Clear && self.converter.popup.is_some())
            {
                self.converter.converter.execute(command);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_rows_respect_minimums() {
        // Enough room: in proportion.
        let rows = star_rows(448.0, &[(56.0, 56.0), (32.0, 32.0), (360.0, 0.0)]);
        assert_eq!(rows, [56.0, 32.0, 360.0]);
        // Too little room: the minimums win and the last row shrinks.
        let rows = star_rows(100.0, &[(56.0, 56.0), (32.0, 32.0), (360.0, 0.0)]);
        assert_eq!(rows, [56.0, 32.0, 12.0]);
    }

    #[test]
    fn typed_characters() {
        assert_eq!(char_command('7'), Some(ConverterCommand::Digit(7)));
        assert_eq!(char_command(','), Some(ConverterCommand::Decimal));
        assert_eq!(char_command('-'), Some(ConverterCommand::Negate));
        assert_eq!(char_command('+'), None);
    }
}
