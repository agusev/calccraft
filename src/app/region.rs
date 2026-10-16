//! The user's regional formats (Settings → Time & language → Language & region →
//! Regional format): numbers, the list separator, dates, month and weekday names, and
//! the first day of the week. As in the Windows Calculator they come from the user's
//! locale, separately from the display language of the strings. The environment
//! variable `CALCCRAFT_REGION` (for example `de-DE`) overrides it.

use std::sync::OnceLock;

use chrono::{NaiveDate, Weekday};

use crate::dates;
use crate::locale::NumberFormat;

/// A region's formats.
pub(super) struct Formats {
    pub numbers: NumberFormat,
    /// Joins the units of a date difference: Windows uses the list separator and a space.
    pub list_separator: String,
    pub first_day: Weekday,
    /// The shortest weekday names, Sunday first (`Su`, `Mo`, …).
    pub weekdays: [String; 7],
    /// The abbreviated month names (`Jan`, `Feb`, …).
    pub months: [String; 12],
    /// Formats dates with Windows' `GetDateFormatEx`, or chrono in en-US without it.
    dates: DateFormats,
}

static FORMATS: OnceLock<Formats> = OnceLock::new();

/// The user's formats, read from Windows the first time they're needed.
pub(super) fn formats() -> &'static Formats {
    FORMATS.get_or_init(|| {
        #[cfg(windows)]
        if let Some(formats) = windows::load() {
            return formats;
        }
        Formats::english()
    })
}

enum DateFormats {
    /// chrono's en-US formats, the same as `dates::format_date`.
    English,
    #[cfg(windows)]
    Windows(windows::Locale),
}

impl Formats {
    /// en-US, for when Windows can't tell.
    fn english() -> Self {
        let names = |list: &[&str]| list.iter().map(|name| name.to_string()).collect::<Vec<_>>();
        Self {
            numbers: NumberFormat::default(),
            list_separator: dates::ENGLISH.separator.to_string(),
            first_day: Weekday::Sun,
            weekdays: names(&["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"])
                .try_into()
                .expect("7 days"),
            months: names(&[
                "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
            ])
            .try_into()
            .expect("12 months"),
            dates: DateFormats::English,
        }
    }

    /// A date as the date pickers show it: `October 15, 2026`, `15. Oktober 2026`.
    pub fn date(&self, date: NaiveDate) -> String {
        match &self.dates {
            DateFormats::English => dates::format_date(date),
            #[cfg(windows)]
            DateFormats::Windows(locale) => locale
                .format(date, 0, Some(&locale.picker_pattern))
                .unwrap_or_else(|| dates::format_date(date)),
        }
    }

    /// A date as the add/subtract result shows it, the region's long date:
    /// `Thursday, October 15, 2026`.
    pub fn long_date(&self, date: NaiveDate) -> String {
        match &self.dates {
            DateFormats::English => dates::format_long_date(date),
            #[cfg(windows)]
            DateFormats::Windows(locale) => locale
                .format(date, windows::DATE_LONGDATE, None)
                .unwrap_or_else(|| dates::format_long_date(date)),
        }
    }

    /// The month and year of the calendar's title: `October 2026`.
    pub fn year_month(&self, date: NaiveDate) -> String {
        match &self.dates {
            DateFormats::English => date.format("%B %Y").to_string(),
            #[cfg(windows)]
            DateFormats::Windows(locale) => locale
                .format(date, windows::DATE_YEARMONTH, None)
                .unwrap_or_else(|| date.format("%B %Y").to_string()),
        }
    }
}

#[cfg(windows)]
mod windows {
    use chrono::{Datelike, NaiveDate, Weekday};
    use windows_sys::Win32::Foundation::SYSTEMTIME;
    pub(super) use windows_sys::Win32::Globalization::{DATE_LONGDATE, DATE_YEARMONTH};
    use windows_sys::Win32::Globalization::{
        GetDateFormatEx, GetLocaleInfoEx, LOCALE_IFIRSTDAYOFWEEK, LOCALE_RETURN_NUMBER,
        LOCALE_SABBREVMONTHNAME1, LOCALE_SDECIMAL, LOCALE_SGROUPING, LOCALE_SLIST,
        LOCALE_SLONGDATE, LOCALE_SSHORTESTDAYNAME1, LOCALE_STHOUSAND,
    };

    use super::{DateFormats, Formats};
    use crate::locale::{NumberFormat, without_weekday};

    /// A locale for the Windows date and locale functions.
    pub(super) struct Locale {
        /// The zero-terminated locale name, or `None` for the user's own locale, which
        /// includes the changes made to it in Windows Settings.
        name: Option<Vec<u16>>,
        /// The date pickers' pattern, zero-terminated.
        pub picker_pattern: Vec<u16>,
    }

    impl Locale {
        fn name(&self) -> *const u16 {
            self.name
                .as_ref()
                .map_or(std::ptr::null(), |name| name.as_ptr())
        }

        /// A locale setting as text (`GetLocaleInfoEx`).
        fn info(&self, setting: u32) -> Option<String> {
            let mut buffer = [0u16; 128];
            // SAFETY: the buffer and its length match; the name is null (the user's
            // locale) or zero-terminated.
            let length = unsafe {
                GetLocaleInfoEx(
                    self.name(),
                    setting,
                    buffer.as_mut_ptr(),
                    buffer.len() as i32,
                )
            };
            (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
        }

        /// A numeric locale setting.
        fn number(&self, setting: u32) -> Option<u32> {
            let mut value = 0u32;
            // SAFETY: with LOCALE_RETURN_NUMBER the function writes one u32, which is two
            // UTF-16 units long.
            let length = unsafe {
                GetLocaleInfoEx(
                    self.name(),
                    setting | LOCALE_RETURN_NUMBER,
                    (&mut value as *mut u32).cast(),
                    2,
                )
            };
            (length > 0).then_some(value)
        }

        /// A date in a format (`GetDateFormatEx`): flags like `DATE_LONGDATE`, or a
        /// zero-terminated pattern.
        pub(super) fn format(
            &self,
            date: NaiveDate,
            flags: u32,
            pattern: Option<&[u16]>,
        ) -> Option<String> {
            let time = SYSTEMTIME {
                wYear: u16::try_from(date.year()).ok()?,
                wMonth: date.month() as u16,
                wDayOfWeek: date.weekday().num_days_from_sunday() as u16,
                wDay: date.day() as u16,
                wHour: 0,
                wMinute: 0,
                wSecond: 0,
                wMilliseconds: 0,
            };
            let pattern = pattern.map_or(std::ptr::null(), <[u16]>::as_ptr);
            let mut buffer = [0u16; 128];
            // SAFETY: the time is a valid date (1601–9999), the buffer and its length
            // match, and the name and the pattern are null or zero-terminated.
            let length = unsafe {
                GetDateFormatEx(
                    self.name(),
                    flags,
                    &time,
                    pattern,
                    buffer.as_mut_ptr(),
                    buffer.len() as i32,
                    std::ptr::null(),
                )
            };
            (length > 0).then(|| String::from_utf16_lossy(&buffer[..length as usize - 1]))
        }
    }

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(Some(0)).collect()
    }

    pub(super) fn load() -> Option<Formats> {
        let name = std::env::var("CALCCRAFT_REGION")
            .ok()
            .map(|name| wide(&name));
        let mut locale = Locale {
            name,
            picker_pattern: Vec::new(),
        };
        let numbers = NumberFormat::new(
            &locale.info(LOCALE_SDECIMAL)?,
            &locale.info(LOCALE_STHOUSAND).unwrap_or_default(),
            &locale
                .info(LOCALE_SGROUPING)
                .unwrap_or_else(|| "3;0".into()),
        );
        let list = locale.info(LOCALE_SLIST).unwrap_or_else(|| ",".into());
        // LOCALE_IFIRSTDAYOFWEEK counts from Monday.
        let first_day = match locale.number(LOCALE_IFIRSTDAYOFWEEK).unwrap_or(6) {
            0 => Weekday::Mon,
            1 => Weekday::Tue,
            2 => Weekday::Wed,
            3 => Weekday::Thu,
            4 => Weekday::Fri,
            5 => Weekday::Sat,
            _ => Weekday::Sun,
        };
        // The shortest day names start with Monday too.
        let monday_first: Vec<String> = (0..7)
            .map(|i| locale.info(LOCALE_SSHORTESTDAYNAME1 + i))
            .collect::<Option<_>>()?;
        let mut weekdays = monday_first;
        weekdays.rotate_right(1);
        let months: Vec<String> = (0..12)
            .map(|i| locale.info(LOCALE_SABBREVMONTHNAME1 + i))
            .collect::<Option<_>>()?;
        let long_date = locale
            .info(LOCALE_SLONGDATE)
            .unwrap_or_else(|| "dddd, MMMM d, yyyy".into());
        locale.picker_pattern = wide(&without_weekday(&long_date));
        Some(Formats {
            numbers,
            list_separator: format!("{list} "),
            first_day,
            weekdays: weekdays.try_into().ok()?,
            months: months.try_into().ok()?,
            dates: DateFormats::Windows(locale),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// Windows' own en-US and de-DE settings, without user changes.
        fn load_region(name: &str) -> (Locale, NumberFormat) {
            let locale = Locale {
                name: Some(wide(name)),
                picker_pattern: wide(&without_weekday(
                    &Locale {
                        name: Some(wide(name)),
                        picker_pattern: Vec::new(),
                    }
                    .info(LOCALE_SLONGDATE)
                    .unwrap(),
                )),
            };
            let numbers = NumberFormat::new(
                &locale.info(LOCALE_SDECIMAL).unwrap(),
                &locale.info(LOCALE_STHOUSAND).unwrap(),
                &locale.info(LOCALE_SGROUPING).unwrap(),
            );
            (locale, numbers)
        }

        #[test]
        fn windows_formats() {
            let date = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
            let (us, numbers) = load_region("en-US");
            assert_eq!(numbers, NumberFormat::default());
            assert_eq!(
                us.format(date, 0, Some(&us.picker_pattern)).unwrap(),
                "October 15, 2026"
            );
            assert_eq!(
                us.format(date, DATE_LONGDATE, None).unwrap(),
                "Thursday, October 15, 2026"
            );
            assert_eq!(
                us.format(date, DATE_YEARMONTH, None).unwrap(),
                "October 2026"
            );

            let (de, numbers) = load_region("de-DE");
            assert_eq!(numbers.localize("1,234,567.5"), "1.234.567,5");
            assert_eq!(
                de.format(date, 0, Some(&de.picker_pattern)).unwrap(),
                "15. Oktober 2026"
            );
            assert_eq!(de.number(LOCALE_IFIRSTDAYOFWEEK), Some(0));
        }
    }
}

/// The date difference's words in the interface language, joined with the region's
/// list separator.
pub(super) fn date_words() -> dates::Words<'static> {
    use super::strings::tr;
    dates::Words {
        same_dates: tr("Date_SameDates"),
        year: [tr("Date_Year"), tr("Date_Years")],
        month: [tr("Date_Month"), tr("Date_Months")],
        week: [tr("Date_Week"), tr("Date_Weeks")],
        day: [tr("Date_Day"), tr("Date_Days")],
        separator: &formats().list_separator,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locale::without_weekday;

    #[test]
    fn english_formats_match_the_date_module() {
        let english = Formats::english();
        let date = NaiveDate::from_ymd_opt(2026, 10, 15).unwrap();
        assert_eq!(english.date(date), "October 15, 2026");
        assert_eq!(english.long_date(date), "Thursday, October 15, 2026");
        assert_eq!(english.year_month(date), "October 2026");
        assert_eq!(english.weekdays[0], "Su");
        assert_eq!(without_weekday("dddd, MMMM d, yyyy"), "MMMM d, yyyy");
    }
}
