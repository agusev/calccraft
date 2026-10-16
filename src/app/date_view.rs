//! "Date calculation" mode: the difference between two dates, or adding and
//! subtracting years, months and days. The layout and sizes follow
//! `DateCalculator.xaml` from the Windows Calculator source.

use chrono::{Datelike, Days, Local, Months, NaiveDate};
use eframe::egui::{
    self, Align2, Color32, CornerRadius, Event, FontId, Id, Key, MouseWheelUnit, Painter, Pos2,
    Rect, Sense, Stroke, StrokeKind, Ui, UiBuilder, epaint::Shadow, pos2, text::LayoutJob, vec2,
};

use super::{
    ACCENT, ACCENT_HOVER, ACCENT_PRESSED, CalculatorApp, Flyout, ICON_CALENDAR, ICON_CHEVRON_DOWN,
    RADIUS, SUBTLE_HOVER, SUBTLE_PRESSED, TEXT, TEXT_DISABLED, TEXT_ON_ACCENT, TEXT_SECONDARY,
    semibold,
};
use crate::dates::{self, MAX_DATE, MAX_OFFSET, MIN_DATE};

// Colors sampled from screenshots of the Windows 11 date calculator.
const LIST_POPUP: Color32 = Color32::from_rgb(0x2F, 0x2A, 0x34);
const LIST_ITEM: Color32 = Color32::from_rgb(0x3C, 0x37, 0x40);
const LIST_ITEM_PRESSED: Color32 = Color32::from_rgb(0x36, 0x31, 0x3A);
const CALENDAR: Color32 = Color32::from_rgb(0x3B, 0x31, 0x3F);
const CALENDAR_HEADER: Color32 = Color32::from_rgb(0x32, 0x2A, 0x36);
const CALENDAR_LINE: Color32 = Color32::from_rgb(0x2C, 0x25, 0x30);
const CALENDAR_HOVER: Color32 = Color32::from_rgb(0x47, 0x3D, 0x4B);
const CALENDAR_PRESSED: Color32 = Color32::from_rgb(0x41, 0x37, 0x45);
const CALENDAR_ARROW: Color32 = Color32::from_rgb(0xA1, 0x9D, 0xA2);
const POPUP_BORDER: Color32 = Color32::from_rgb(0x48, 0x3F, 0x4C);
const OTHER_MONTH: Color32 = Color32::from_rgb(0xA8, 0xA1, 0xA9);
const CONTROL: Color32 = Color32::from_rgb(0x36, 0x28, 0x36);
const CONTROL_HOVER: Color32 = Color32::from_rgb(0x3C, 0x2E, 0x3C);
const CONTROL_PRESSED: Color32 = Color32::from_rgb(0x31, 0x23, 0x31);
const CONTROL_BORDER: Color32 = Color32::from_rgb(0x44, 0x36, 0x44);
const RADIO_BORDER: Color32 = Color32::from_rgb(0x9E, 0x96, 0x9F);
/// Darkens the page while the calendar is open.
const DIM: Color32 = Color32::from_black_alpha(110);

// Glyphs from the Segoe Fluent Icons font.
const ICON_CARET_UP: char = '\u{EDDB}';
const ICON_CARET_DOWN: char = '\u{EDDC}';

const PAGE_MARGIN: f32 = 16.0;
const CONTROL_HEIGHT: f32 = 32.0;
/// From this window width on, Windows shows results in a larger font.
const WIDE_LAYOUT_WIDTH: f32 = 480.0;
const COMBO_WIDTH: f32 = 84.0;
const COMBO_GAP: f32 = 12.0;
const LIST_ITEM_HEIGHT: f32 = 32.0;
const LIST_VISIBLE_ITEMS: usize = 9;
const CALENDAR_WIDTH: f32 = 336.0;
const CALENDAR_HEADER_HEIGHT: f32 = 48.0;
const WEEKDAY_ROW_HEIGHT: f32 = 38.0;
const CALENDAR_ROW_HEIGHT: f32 = 41.0;
/// How far the mouse wheel has to move to flip one calendar page.
const WHEEL_STEP: f32 = 40.0;

const WEEKDAYS: [&str; 7] = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// The two pages of the mode, picked in the drop-down at the top.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Difference,
    AddSubtract,
}

impl Page {
    const ALL: [Page; 2] = [Page::Difference, Page::AddSubtract];

    fn title(self) -> &'static str {
        match self {
            Page::Difference => "Difference between dates",
            Page::AddSubtract => "Add or subtract days",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum DateField {
    From,
    To,
    /// The "From" date of "Add or subtract days".
    Start,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Offset {
    Years,
    Months,
    Days,
}

impl Offset {
    const ALL: [Offset; 3] = [Offset::Years, Offset::Months, Offset::Days];

    fn label(self) -> &'static str {
        match self {
            Offset::Years => "Years",
            Offset::Months => "Months",
            Offset::Days => "Days",
        }
    }
}

/// Zoom level of the calendar: days of a month, months of a year, years of a decade.
#[derive(Clone, Copy, PartialEq, Eq)]
enum CalendarView {
    Month,
    Year,
    Decade,
}

#[derive(Clone, Copy, PartialEq)]
enum Popup {
    /// The list of pages ("Difference between dates", "Add or subtract days").
    Pages,
    /// `shown` is the first day of the shown month (in any view).
    Calendar {
        field: DateField,
        anchor: Rect,
        view: CalendarView,
        shown: NaiveDate,
    },
    /// The 0–999 list of a years, months or days box. `scroll_pending` scrolls
    /// the selected value into place on the first frame.
    Offset {
        field: Offset,
        anchor: Rect,
        scroll_pending: bool,
    },
}

/// What a click in the calendar asks for.
enum CalendarAction {
    Pick(NaiveDate),
    Show(CalendarView, NaiveDate),
}

/// State of the date calculation mode.
pub(super) struct DateState {
    page: Page,
    from: NaiveDate,
    to: NaiveDate,
    start: NaiveDate,
    subtract: bool,
    years: u32,
    months: u32,
    days: u32,
    popup: Option<Popup>,
    /// Mouse wheel movement over the calendar that hasn't flipped a page yet.
    wheel: f32,
}

impl DateState {
    pub(super) fn new() -> Self {
        let today = today();
        Self {
            page: Page::Difference,
            from: today,
            to: today,
            start: today,
            subtract: false,
            years: 0,
            months: 0,
            days: 0,
            popup: None,
            wheel: 0.0,
        }
    }

    /// Closes an open drop-down or calendar. Returns `false` if none was open.
    pub(super) fn close_popup(&mut self) -> bool {
        self.popup.take().is_some()
    }

    /// The main result line; Ctrl+C copies it.
    pub(super) fn result_text(&self) -> String {
        match self.page {
            Page::Difference => dates::describe_difference(self.from, self.to).0,
            Page::AddSubtract => self
                .resulting_date()
                .map_or_else(|| "Date out of Bound".to_string(), dates::format_long_date),
        }
    }

    fn resulting_date(&self) -> Option<NaiveDate> {
        let (years, months, days) = (self.years, self.months, self.days);
        if self.subtract {
            dates::subtract(self.start, years, months, days)
        } else {
            dates::add(self.start, years, months, days)
        }
    }

    fn date(&self, field: DateField) -> NaiveDate {
        match field {
            DateField::From => self.from,
            DateField::To => self.to,
            DateField::Start => self.start,
        }
    }

    fn set_date(&mut self, field: DateField, date: NaiveDate) {
        let date = dates::clamp(date);
        match field {
            DateField::From => self.from = date,
            DateField::To => self.to = date,
            DateField::Start => self.start = date,
        }
    }

    fn offset(&self, field: Offset) -> u32 {
        match field {
            Offset::Years => self.years,
            Offset::Months => self.months,
            Offset::Days => self.days,
        }
    }

    fn set_offset(&mut self, field: Offset, value: u32) {
        let value = value.min(MAX_OFFSET);
        match field {
            Offset::Years => self.years = value,
            Offset::Months => self.months = value,
            Offset::Days => self.days = value,
        }
    }
}

impl CalculatorApp {
    /// Keyboard in date mode: Esc closes popups, Ctrl+C copies the result.
    pub(super) fn handle_date_keyboard(&mut self, ctx: &egui::Context) {
        for event in ctx.input(|i| i.events.clone()) {
            match event {
                Event::Key {
                    key: Key::Escape,
                    pressed: true,
                    ..
                } => {
                    if self.flyout != Flyout::None {
                        self.flyout = Flyout::None;
                    } else {
                        self.date.close_popup();
                    }
                }
                Event::Copy => ctx.copy_text(self.date.result_text()),
                _ => {}
            }
        }
    }

    /// Draws the mode below the header. `full` is the whole window.
    pub(super) fn draw_date_mode(&mut self, ui: &mut Ui, full: Rect, top: f32) {
        let left = full.left() + PAGE_MARGIN;
        let painter = ui.painter().clone();
        let selector = self.draw_page_selector(ui, pos2(left, top + 4.0));
        let content = top + 36.0;
        let result_size = if full.width() >= WIDE_LAYOUT_WIDTH {
            20.0
        } else {
            14.0
        };
        let wrap_width = full.width() - 2.0 * PAGE_MARGIN;

        match self.date.page {
            Page::Difference => {
                caption(&painter, pos2(left, content + 17.0), "From");
                self.draw_date_picker(ui, DateField::From, pos2(left, content + 41.0));
                caption(&painter, pos2(left, content + 97.0), "To");
                self.draw_date_picker(ui, DateField::To, pos2(left, content + 121.0));
                caption(&painter, pos2(left, content + 177.0), "Difference");
                let (result, in_days) = dates::describe_difference(self.date.from, self.date.to);
                let pos = pos2(left, content + 197.0);
                let bottom = paragraph(&painter, pos, result, result_size, wrap_width);
                if let Some(in_days) = in_days {
                    caption(&painter, pos2(left, bottom + 8.0), &in_days);
                }
            }
            Page::AddSubtract => {
                caption(&painter, pos2(left, content + 17.0), "From");
                self.draw_date_picker(ui, DateField::Start, pos2(left, content + 41.0));

                let radios = content + 96.0;
                let subtract = self.date.subtract;
                let (add_clicked, add_right) = radio(
                    ui,
                    Id::new("date-add"),
                    pos2(left, radios),
                    "Add",
                    !subtract,
                );
                let subtract_pos = pos2(add_right + 20.0, radios);
                let (subtract_clicked, _) = radio(
                    ui,
                    Id::new("date-subtract"),
                    subtract_pos,
                    "Subtract",
                    subtract,
                );
                if add_clicked {
                    self.date.subtract = false;
                }
                if subtract_clicked {
                    self.date.subtract = true;
                }

                let labels = radios + CONTROL_HEIGHT + 29.0;
                for (i, field) in Offset::ALL.into_iter().enumerate() {
                    let x = left + i as f32 * (COMBO_WIDTH + COMBO_GAP);
                    caption(&painter, pos2(x, labels), field.label());
                    let combo = Rect::from_min_size(
                        pos2(x, labels + 22.0),
                        vec2(COMBO_WIDTH, CONTROL_HEIGHT),
                    );
                    self.draw_offset_combo(ui, field, combo);
                }

                let date_label = labels + 22.0 + CONTROL_HEIGHT + 35.0;
                caption(&painter, pos2(left, date_label), "Date");
                let pos = pos2(left, date_label + 20.0);
                paragraph(
                    &painter,
                    pos,
                    self.date.result_text(),
                    result_size,
                    wrap_width,
                );
            }
        }

        self.draw_date_popup(ui, full, selector);
    }

    // -------------------------------------------------------------- controls

    /// The borderless drop-down at the top that switches pages.
    fn draw_page_selector(&mut self, ui: &Ui, text_pos: Pos2) -> Rect {
        let painter = ui.painter();
        let label = painter.layout_no_wrap(
            self.date.page.title().to_string(),
            FontId::proportional(14.0),
            TEXT,
        );
        let rect = Rect::from_min_size(
            pos2(text_pos.x - 12.0, text_pos.y),
            vec2(12.0 + label.size().x + 38.0, CONTROL_HEIGHT),
        );
        let response = ui.interact(rect, Id::new("date-page"), Sense::CLICK);
        painter.rect_filled(rect, RADIUS, subtle_fill(&response));
        let label_pos = pos2(text_pos.x, rect.center().y - label.size().y / 2.0);
        painter.galley(label_pos, label, TEXT);
        let chevron = self.icon(ICON_CHEVRON_DOWN, "˅", 12.0);
        paint_centered(
            painter,
            pos2(rect.right() - 20.0, rect.center().y),
            chevron,
            TEXT,
        );
        if response.clicked() {
            self.date.popup = Some(Popup::Pages);
        }
        rect
    }

    /// A date with a calendar glyph; a click opens the calendar.
    fn draw_date_picker(&mut self, ui: &Ui, field: DateField, text_pos: Pos2) {
        let date = self.date.date(field);
        let painter = ui.painter();
        let label =
            painter.layout_no_wrap(dates::format_date(date), FontId::proportional(14.0), TEXT);
        let rect = Rect::from_min_size(
            pos2(text_pos.x - 12.0, text_pos.y),
            vec2(12.0 + label.size().x + 36.0, CONTROL_HEIGHT),
        );
        let response = ui.interact(rect, Id::new("date-picker").with(field), Sense::CLICK);
        painter.rect_filled(rect, RADIUS, subtle_fill(&response));
        let glyph_center = pos2(text_pos.x + label.size().x + 16.0, rect.center().y);
        painter.galley(
            pos2(text_pos.x, rect.center().y - label.size().y / 2.0),
            label,
            TEXT,
        );
        let glyph = self.icon(ICON_CALENDAR, "▦", 16.0);
        paint_centered(painter, glyph_center, glyph, TEXT);
        if response.clicked() {
            self.date.popup = Some(Popup::Calendar {
                field,
                anchor: rect,
                view: CalendarView::Month,
                shown: first_of_month(date),
            });
        }
    }

    /// A years, months or days box; a click opens the 0–999 list.
    fn draw_offset_combo(&mut self, ui: &Ui, field: Offset, rect: Rect) {
        let response = ui.interact(rect, Id::new("date-offset").with(field), Sense::CLICK);
        let fill = if response.is_pointer_button_down_on() {
            CONTROL_PRESSED
        } else if response.hovered() {
            CONTROL_HOVER
        } else {
            CONTROL
        };
        let painter = ui.painter();
        painter.rect_filled(rect, RADIUS, fill);
        painter.rect_stroke(
            rect,
            RADIUS,
            Stroke::new(1.0, CONTROL_BORDER),
            StrokeKind::Inside,
        );
        painter.text(
            pos2(rect.left() + 12.0, rect.center().y),
            Align2::LEFT_CENTER,
            self.date.offset(field).to_string(),
            FontId::proportional(14.0),
            TEXT,
        );
        let chevron = self.icon(ICON_CHEVRON_DOWN, "˅", 12.0);
        let chevron_pos = pos2(rect.right() - 18.0, rect.center().y);
        paint_centered(painter, chevron_pos, chevron, TEXT_SECONDARY);
        if response.clicked() {
            self.date.popup = Some(Popup::Offset {
                field,
                anchor: rect,
                scroll_pending: true,
            });
        }
    }

    // ---------------------------------------------------------------- popups

    fn draw_date_popup(&mut self, ui: &mut Ui, full: Rect, selector: Rect) {
        let Some(popup) = self.date.popup else {
            self.date.wheel = 0.0;
            return;
        };
        if matches!(popup, Popup::Calendar { .. }) {
            ui.painter().rect_filled(full, 0.0, DIM);
        }
        // A click anywhere outside the popup closes it ("light dismiss").
        let backdrop = ui.interact(full, Id::new("date-popup-backdrop"), Sense::CLICK);
        match popup {
            Popup::Pages => self.draw_page_list(ui, full, selector),
            Popup::Calendar {
                field,
                anchor,
                view,
                shown,
            } => self.draw_calendar(ui, full, field, anchor, view, shown),
            Popup::Offset {
                field,
                anchor,
                scroll_pending,
            } => self.draw_offset_list(ui, full, field, anchor, scroll_pending),
        }
        if backdrop.clicked() {
            self.date.popup = None;
        }
    }

    fn draw_page_list(&mut self, ui: &Ui, full: Rect, selector: Rect) {
        const ITEM_HEIGHT: f32 = 36.0;
        const STRIDE: f32 = ITEM_HEIGHT + 2.0;
        let selected = Page::ALL.iter().position(|&page| page == self.date.page);
        let selected = selected.unwrap_or(0) as f32;
        // The selected item sits right over the drop-down, as in WinUI.
        let item_top = selector.center().y - ITEM_HEIGHT / 2.0;
        let top = (item_top - 4.0 - selected * STRIDE).max(full.top() + 4.0);
        let size = vec2(
            276.0_f32.min(full.width() - 10.0),
            8.0 + Page::ALL.len() as f32 * STRIDE - 2.0,
        );
        let rect = Rect::from_min_size(pos2(full.left() + 5.0, top), size);
        popup_frame(ui.painter(), rect, LIST_POPUP);
        ui.interact(rect, Id::new("date-page-list"), Sense::CLICK);

        for (i, page) in Page::ALL.into_iter().enumerate() {
            let item = Rect::from_min_size(
                rect.min + vec2(4.0, 4.0 + i as f32 * STRIDE),
                vec2(rect.width() - 8.0, ITEM_HEIGHT),
            );
            let id = Id::new("date-page-item").with(i);
            if list_item(ui, id, item, page.title(), page == self.date.page) {
                self.date.page = page;
                self.date.popup = None;
            }
        }
    }

    fn draw_offset_list(
        &mut self,
        ui: &mut Ui,
        full: Rect,
        field: Offset,
        anchor: Rect,
        scroll_pending: bool,
    ) {
        let selected = self.date.offset(field) as usize;
        let fits = ((full.height() - 16.0) / LIST_ITEM_HEIGHT).floor() as usize;
        let visible = LIST_VISIBLE_ITEMS.min(fits).max(1);
        let height = visible as f32 * LIST_ITEM_HEIGHT + 8.0;
        // Like WinUI, put the selected value over the box when possible.
        let above = selected.min(visible / 2);
        let lowest_top = (full.bottom() - 4.0 - height).max(full.top() + 4.0);
        let top = (anchor.top() - 4.0 - above as f32 * LIST_ITEM_HEIGHT)
            .clamp(full.top() + 4.0, lowest_top);
        let rect = Rect::from_min_size(pos2(anchor.left(), top), vec2(anchor.width(), height));
        popup_frame(ui.painter(), rect, LIST_POPUP);
        ui.interact(rect, Id::new("date-offset-list"), Sense::CLICK);

        let mut chosen = None;
        ui.scope_builder(UiBuilder::new().max_rect(rect.shrink(4.0)), |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut area = egui::ScrollArea::vertical()
                .id_salt(("date-offset-list", field))
                .auto_shrink(false);
            if scroll_pending {
                area = area.vertical_scroll_offset((selected - above) as f32 * LIST_ITEM_HEIGHT);
            }
            let rows = MAX_OFFSET as usize + 1;
            area.show_rows(ui, LIST_ITEM_HEIGHT, rows, |ui, range| {
                for value in range {
                    let size = vec2(ui.available_width(), LIST_ITEM_HEIGHT);
                    let (row, _) = ui.allocate_exact_size(size, Sense::hover());
                    let item = row.shrink2(vec2(0.0, 1.0));
                    let id = Id::new("date-offset-item").with(value);
                    if list_item(ui, id, item, &value.to_string(), value == selected) {
                        chosen = Some(value as u32);
                    }
                }
            });
        });

        if let Some(Popup::Offset { scroll_pending, .. }) = &mut self.date.popup {
            *scroll_pending = false;
        }
        if let Some(value) = chosen {
            self.date.set_offset(field, value);
            self.date.popup = None;
        }
    }

    // -------------------------------------------------------------- calendar

    fn draw_calendar(
        &mut self,
        ui: &Ui,
        full: Rect,
        field: DateField,
        anchor: Rect,
        view: CalendarView,
        shown: NaiveDate,
    ) {
        let width = CALENDAR_WIDTH.min(full.width() - 24.0);
        let height =
            CALENDAR_HEADER_HEIGHT + WEEKDAY_ROW_HEIGHT + 2.0 + 6.0 * CALENDAR_ROW_HEIGHT + 8.0;
        // Windows opens the calendar slightly above the date it edits.
        let top = (anchor.top() - 29.0)
            .min(full.bottom() - 8.0 - height)
            .max(full.top() + 8.0);
        let rect = Rect::from_min_size(pos2(full.left() + 12.0, top), vec2(width, height));

        let painter = ui.painter().clone();
        popup_frame(&painter, rect, CALENDAR);
        let header = rect.with_max_y(rect.top() + CALENDAR_HEADER_HEIGHT);
        let top_corners = CornerRadius {
            nw: 8,
            ne: 8,
            sw: 0,
            se: 0,
        };
        painter.rect_filled(header.shrink(1.0), top_corners, CALENDAR_HEADER);
        painter.hline(
            rect.x_range(),
            header.bottom(),
            Stroke::new(1.0, CALENDAR_LINE),
        );
        ui.interact(rect, Id::new("calendar"), Sense::CLICK);

        let mut action = None;

        // Header: the title zooms out, the arrows flip pages.
        let arrow_width = (width / 7.0).max(48.0);
        let title_button = Rect::from_min_max(
            pos2(rect.left() + 8.0, rect.top() + 4.0),
            pos2(rect.right() - 2.0 * arrow_width - 4.0, rect.top() + 44.0),
        );
        let zoom_out = match view {
            CalendarView::Month => Some(CalendarView::Year),
            CalendarView::Year => Some(CalendarView::Decade),
            CalendarView::Decade => None,
        };
        let id = Id::new("calendar-title");
        if flat_button(ui, id, title_button, zoom_out.is_some()) {
            action = zoom_out.map(|view| CalendarAction::Show(view, shown));
        }
        let title = match view {
            CalendarView::Month => shown.format("%B %Y").to_string(),
            CalendarView::Year => shown.year().to_string(),
            CalendarView::Decade => {
                let start = decade_start(shown);
                format!("{start} - {}", start + 9)
            }
        };
        painter.text(
            pos2(title_button.left() + 8.0, title_button.center().y),
            Align2::LEFT_CENTER,
            title,
            FontId::new(14.0, semibold()),
            TEXT,
        );

        let arrows = [
            (ICON_CARET_UP, "▲", false, rect.right() - 1.5 * arrow_width),
            (ICON_CARET_DOWN, "▼", true, rect.right() - 0.5 * arrow_width),
        ];
        for (glyph, fallback, forward, x) in arrows {
            let target = step_page(view, shown, forward);
            let button =
                Rect::from_center_size(pos2(x, rect.top() + 24.0), vec2(arrow_width - 4.0, 40.0));
            let id = Id::new("calendar-arrow").with(forward);
            if flat_button(ui, id, button, target.is_some()) {
                action = target.map(|shown| CalendarAction::Show(view, shown));
            }
            let color = if target.is_some() {
                CALENDAR_ARROW
            } else {
                TEXT_DISABLED
            };
            paint_centered(
                &painter,
                button.center(),
                self.icon(glyph, fallback, 8.0),
                color,
            );
        }

        // The mouse wheel flips pages too.
        if ui.rect_contains_pointer(rect) {
            self.date.wheel += ui.input(|i| wheel_delta(&i.events));
            if self.date.wheel.abs() >= WHEEL_STEP {
                let forward = self.date.wheel < 0.0;
                self.date.wheel = 0.0;
                if let Some(shown) = step_page(view, shown, forward) {
                    action = Some(CalendarAction::Show(view, shown));
                }
            }
        }

        let body = Rect::from_min_max(
            pos2(rect.left() + 8.0, header.bottom()),
            pos2(rect.right() - 8.0, rect.bottom() - 8.0),
        );
        let today = today();
        let selected = self.date.date(field);
        let picked = match view {
            CalendarView::Month => month_view(ui, &painter, body, shown, today, selected),
            CalendarView::Year => year_view(ui, body, shown, today, selected),
            CalendarView::Decade => decade_view(ui, body, shown, today, selected),
        };
        action = picked.or(action);

        match action {
            Some(CalendarAction::Pick(date)) => {
                self.date.set_date(field, date);
                self.date.popup = None;
            }
            Some(CalendarAction::Show(view, shown)) => {
                self.date.popup = Some(Popup::Calendar {
                    field,
                    anchor,
                    view,
                    shown,
                });
            }
            None => {}
        }
    }
}

/// Days of a month: 6 weeks starting on Sunday, with neighbor months dimmed.
fn month_view(
    ui: &Ui,
    painter: &Painter,
    body: Rect,
    shown: NaiveDate,
    today: NaiveDate,
    selected: NaiveDate,
) -> Option<CalendarAction> {
    let column = body.width() / 7.0;
    for (i, name) in WEEKDAYS.iter().enumerate() {
        let center = pos2(
            body.left() + (i as f32 + 0.5) * column,
            body.top() + 2.0 + WEEKDAY_ROW_HEIGHT / 2.0,
        );
        painter.text(
            center,
            Align2::CENTER_CENTER,
            name,
            FontId::new(13.0, semibold()),
            TEXT,
        );
    }

    let grid_top = body.top() + 2.0 + WEEKDAY_ROW_HEIGHT;
    let page_start = dates::month_page_start(shown.year(), shown.month());
    let mut action = None;
    for i in 0..42 {
        let day = page_start + Days::new(i);
        if !(MIN_DATE..=MAX_DATE).contains(&day) {
            continue;
        }
        let (row, col) = ((i / 7) as f32, (i % 7) as f32);
        let cell = Rect::from_min_size(
            pos2(
                body.left() + col * column,
                grid_top + row * CALENDAR_ROW_HEIGHT,
            ),
            vec2(column, CALENDAR_ROW_HEIGHT),
        );
        let state = CellState {
            today: day == today,
            selected: day == selected,
            dimmed: day.month() != shown.month(),
        };
        let id = Id::new("calendar-day").with(i);
        if calendar_cell(ui, id, cell, &day.day().to_string(), state) {
            action = Some(CalendarAction::Pick(day));
        }
    }
    action
}

/// Months of a year in a 4×4 grid; the first months of the next year are dimmed.
fn year_view(
    ui: &Ui,
    body: Rect,
    shown: NaiveDate,
    today: NaiveDate,
    selected: NaiveDate,
) -> Option<CalendarAction> {
    let january = NaiveDate::from_ymd_opt(shown.year(), 1, 1)?;
    let mut action = None;
    for i in 0..16 {
        let month = january.checked_add_months(Months::new(i))?;
        if !(first_of_month(MIN_DATE)..=first_of_month(MAX_DATE)).contains(&month) {
            continue;
        }
        let same_month = |date: NaiveDate| first_of_month(date) == month;
        let state = CellState {
            today: same_month(today),
            selected: same_month(selected),
            dimmed: month.year() != shown.year(),
        };
        let label = MONTHS[month.month0() as usize];
        let id = Id::new("calendar-month").with(i);
        if calendar_cell(ui, id, grid_cell(body, i as usize), label, state) {
            action = Some(CalendarAction::Show(CalendarView::Month, month));
        }
    }
    action
}

/// Years of a decade in a 4×4 grid; the years after the decade are dimmed.
fn decade_view(
    ui: &Ui,
    body: Rect,
    shown: NaiveDate,
    today: NaiveDate,
    selected: NaiveDate,
) -> Option<CalendarAction> {
    let start = decade_start(shown);
    let mut action = None;
    for i in 0..16 {
        let year = start + i;
        if !(MIN_DATE.year()..=MAX_DATE.year()).contains(&year) {
            continue;
        }
        let state = CellState {
            today: today.year() == year,
            selected: selected.year() == year,
            dimmed: year > start + 9,
        };
        let id = Id::new("calendar-year").with(i);
        let cell = grid_cell(body, i as usize);
        if calendar_cell(ui, id, cell, &year.to_string(), state) {
            let january = NaiveDate::from_ymd_opt(year, 1, 1)?;
            action = Some(CalendarAction::Show(CalendarView::Year, january));
        }
    }
    action
}

/// Cell `index` of the 4×4 grid used by the year and decade views.
fn grid_cell(body: Rect, index: usize) -> Rect {
    let size = vec2(body.width() / 4.0, (body.height() - 2.0) / 4.0);
    let (row, col) = ((index / 4) as f32, (index % 4) as f32);
    Rect::from_min_size(
        pos2(body.left() + col * size.x, body.top() + 2.0 + row * size.y),
        size,
    )
}

#[derive(Clone, Copy)]
struct CellState {
    today: bool,
    selected: bool,
    dimmed: bool,
}

/// A round calendar cell: today is filled with the accent color, the
/// selected date gets an accent ring. Returns `true` when clicked.
fn calendar_cell(ui: &Ui, id: Id, cell: Rect, label: &str, state: CellState) -> bool {
    let response = ui.interact(cell, id, Sense::CLICK);
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();
    let radius = (cell.width().min(cell.height()) / 2.0 - 1.0).min(28.0);
    let (fill, text) = if state.today {
        let fill = if pressed {
            ACCENT_PRESSED
        } else if hovered {
            ACCENT_HOVER
        } else {
            ACCENT
        };
        (fill, TEXT_ON_ACCENT)
    } else {
        let fill = if pressed {
            CALENDAR_PRESSED
        } else if hovered {
            CALENDAR_HOVER
        } else {
            Color32::TRANSPARENT
        };
        (fill, if state.dimmed { OTHER_MONTH } else { TEXT })
    };
    let painter = ui.painter();
    painter.circle_filled(cell.center(), radius, fill);
    if state.selected && !state.today {
        painter.circle_stroke(cell.center(), radius - 1.0, Stroke::new(2.0, ACCENT));
    }
    painter.text(
        cell.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(14.0),
        text,
    );
    response.clicked()
}

// -------------------------------------------------------------- helpers

/// A small gray label above a control.
fn caption(painter: &Painter, pos: Pos2, text: &str) {
    painter.text(
        pos,
        Align2::LEFT_TOP,
        text,
        FontId::proportional(12.0),
        TEXT_SECONDARY,
    );
}

/// Wrapped result text; returns its bottom edge.
fn paragraph(painter: &Painter, pos: Pos2, text: String, size: f32, wrap_width: f32) -> f32 {
    let galley = painter.layout(text, FontId::proportional(size), TEXT, wrap_width);
    let bottom = pos.y + galley.size().y;
    painter.galley(pos, galley, TEXT);
    bottom
}

/// A WinUI radio button. Returns whether it was clicked and its right edge.
fn radio(ui: &Ui, id: Id, pos: Pos2, label: &str, checked: bool) -> (bool, f32) {
    let painter = ui.painter();
    let galley = painter.layout_no_wrap(label.to_string(), FontId::proportional(14.0), TEXT);
    let width = (28.0 + galley.size().x).max(80.0);
    let rect = Rect::from_min_size(pos, vec2(width, CONTROL_HEIGHT));
    let response = ui.interact(rect, id, Sense::CLICK);
    let pressed = response.is_pointer_button_down_on();
    let hovered = response.hovered();

    let center = pos2(pos.x + 10.0, rect.center().y);
    if checked {
        let fill = if pressed {
            ACCENT_PRESSED
        } else if hovered {
            ACCENT_HOVER
        } else {
            ACCENT
        };
        painter.circle_filled(center, 10.0, fill);
        let dot = if pressed {
            4.0
        } else if hovered {
            6.0
        } else {
            5.0
        };
        painter.circle_filled(center, dot, TEXT_ON_ACCENT);
    } else {
        let fill = if pressed {
            SUBTLE_PRESSED
        } else if hovered {
            SUBTLE_HOVER
        } else {
            Color32::TRANSPARENT
        };
        painter.circle_filled(center, 10.0, fill);
        painter.circle_stroke(center, 9.5, Stroke::new(1.0, RADIO_BORDER));
        if pressed {
            painter.circle_filled(center, 5.0, TEXT_SECONDARY);
        }
    }
    let label_pos = pos2(pos.x + 28.0, rect.center().y - galley.size().y / 2.0);
    painter.galley(label_pos, galley, TEXT);
    (response.clicked(), rect.right())
}

/// An item of a drop-down list: highlighted when hovered or selected, with
/// the accent pill on the selected one. Returns `true` when clicked.
fn list_item(ui: &Ui, id: Id, rect: Rect, label: &str, selected: bool) -> bool {
    let response = ui.interact(rect, id, Sense::CLICK);
    let fill = if response.is_pointer_button_down_on() {
        LIST_ITEM_PRESSED
    } else if response.hovered() || selected {
        LIST_ITEM
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter();
    painter.rect_filled(rect, RADIUS, fill);
    if selected {
        let pill =
            Rect::from_center_size(pos2(rect.left() + 5.0, rect.center().y), vec2(3.0, 16.0));
        painter.rect_filled(pill, 2.0, ACCENT);
    }
    painter.text(
        pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        TEXT,
    );
    response.clicked()
}

/// A transparent button that only shows a background on hover.
fn flat_button(ui: &Ui, id: Id, rect: Rect, enabled: bool) -> bool {
    let sense = if enabled {
        Sense::CLICK
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    if enabled {
        let fill = if response.is_pointer_button_down_on() {
            CALENDAR_PRESSED
        } else if response.hovered() {
            CALENDAR_HOVER
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, RADIUS, fill);
    }
    enabled && response.clicked()
}

fn subtle_fill(response: &egui::Response) -> Color32 {
    if response.is_pointer_button_down_on() {
        SUBTLE_PRESSED
    } else if response.hovered() {
        SUBTLE_HOVER
    } else {
        Color32::TRANSPARENT
    }
}

/// Shadow, background and border of a popup.
fn popup_frame(painter: &Painter, rect: Rect, fill: Color32) {
    let shadow = Shadow {
        offset: [0, 8],
        blur: 24,
        spread: 0,
        color: Color32::from_black_alpha(100),
    };
    painter.add(shadow.as_shape(rect, 8));
    painter.rect_filled(rect, 8.0, fill);
    painter.rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, POPUP_BORDER),
        StrokeKind::Inside,
    );
}

fn paint_centered(painter: &Painter, center: Pos2, label: LayoutJob, color: Color32) {
    let galley = painter.layout_job(label);
    painter.galley(center - galley.size() / 2.0, galley, color);
}

/// Sum of vertical mouse wheel movement this frame, in points
/// (positive when scrolling up).
fn wheel_delta(events: &[Event]) -> f32 {
    events
        .iter()
        .map(|event| match event {
            Event::MouseWheel { unit, delta, .. } => match unit {
                MouseWheelUnit::Point => delta.y,
                MouseWheelUnit::Line => delta.y * WHEEL_STEP,
                MouseWheelUnit::Page => delta.y * WHEEL_STEP * 10.0,
            },
            _ => 0.0,
        })
        .sum()
}

/// The previous or next page of a calendar view, if it is in the supported range.
fn step_page(view: CalendarView, shown: NaiveDate, forward: bool) -> Option<NaiveDate> {
    let months = Months::new(match view {
        CalendarView::Month => 1,
        CalendarView::Year => 12,
        CalendarView::Decade => 120,
    });
    let next = if forward {
        shown.checked_add_months(months)
    } else {
        shown.checked_sub_months(months)
    }?;
    let in_range = match view {
        CalendarView::Month => {
            (first_of_month(MIN_DATE)..=first_of_month(MAX_DATE)).contains(&next)
        }
        CalendarView::Year => (MIN_DATE.year()..=MAX_DATE.year()).contains(&next.year()),
        CalendarView::Decade => {
            decade_start(next) + 9 >= MIN_DATE.year() && decade_start(next) <= MAX_DATE.year()
        }
    };
    in_range.then_some(next)
}

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

fn decade_start(date: NaiveDate) -> i32 {
    date.year() - date.year().rem_euclid(10)
}

fn today() -> NaiveDate {
    dates::clamp(Local::now().date_naive())
}
