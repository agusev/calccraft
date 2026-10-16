//! The interface's text in the user's language. The strings are the Windows Calculator's
//! own translations, imported into `locales/<language>.txt` by `tools/import-strings.ps1`,
//! and the code looks them up by their Windows resource keys (`tr("HistoryLabel.Text")`).
//! Like Windows, the language follows the Windows display language.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Every language the Windows Calculator is translated into, with its strings.
const LOCALES: &[(&str, &str)] = &[
    ("af-ZA", include_str!("../../locales/af-ZA.txt")),
    ("am-ET", include_str!("../../locales/am-ET.txt")),
    ("ar-SA", include_str!("../../locales/ar-SA.txt")),
    ("az-Latn-AZ", include_str!("../../locales/az-Latn-AZ.txt")),
    ("bg-BG", include_str!("../../locales/bg-BG.txt")),
    ("ca-ES", include_str!("../../locales/ca-ES.txt")),
    ("cs-CZ", include_str!("../../locales/cs-CZ.txt")),
    ("da-DK", include_str!("../../locales/da-DK.txt")),
    ("de-DE", include_str!("../../locales/de-DE.txt")),
    ("el-GR", include_str!("../../locales/el-GR.txt")),
    ("en-GB", include_str!("../../locales/en-GB.txt")),
    ("en-US", include_str!("../../locales/en-US.txt")),
    ("es-ES", include_str!("../../locales/es-ES.txt")),
    ("es-MX", include_str!("../../locales/es-MX.txt")),
    ("et-EE", include_str!("../../locales/et-EE.txt")),
    ("eu-ES", include_str!("../../locales/eu-ES.txt")),
    ("fa-IR", include_str!("../../locales/fa-IR.txt")),
    ("fi-FI", include_str!("../../locales/fi-FI.txt")),
    ("fil-PH", include_str!("../../locales/fil-PH.txt")),
    ("fr-CA", include_str!("../../locales/fr-CA.txt")),
    ("fr-FR", include_str!("../../locales/fr-FR.txt")),
    ("gl-ES", include_str!("../../locales/gl-ES.txt")),
    ("he-IL", include_str!("../../locales/he-IL.txt")),
    ("hi-IN", include_str!("../../locales/hi-IN.txt")),
    ("hr-HR", include_str!("../../locales/hr-HR.txt")),
    ("hu-HU", include_str!("../../locales/hu-HU.txt")),
    ("id-ID", include_str!("../../locales/id-ID.txt")),
    ("is-IS", include_str!("../../locales/is-IS.txt")),
    ("it-IT", include_str!("../../locales/it-IT.txt")),
    ("ja-JP", include_str!("../../locales/ja-JP.txt")),
    ("kk-KZ", include_str!("../../locales/kk-KZ.txt")),
    ("km-KH", include_str!("../../locales/km-KH.txt")),
    ("kn-IN", include_str!("../../locales/kn-IN.txt")),
    ("ko-KR", include_str!("../../locales/ko-KR.txt")),
    ("lo-LA", include_str!("../../locales/lo-LA.txt")),
    ("lt-LT", include_str!("../../locales/lt-LT.txt")),
    ("lv-LV", include_str!("../../locales/lv-LV.txt")),
    ("mk-MK", include_str!("../../locales/mk-MK.txt")),
    ("ml-IN", include_str!("../../locales/ml-IN.txt")),
    ("ms-MY", include_str!("../../locales/ms-MY.txt")),
    ("nb-NO", include_str!("../../locales/nb-NO.txt")),
    ("nl-NL", include_str!("../../locales/nl-NL.txt")),
    ("pl-PL", include_str!("../../locales/pl-PL.txt")),
    ("pt-BR", include_str!("../../locales/pt-BR.txt")),
    ("pt-PT", include_str!("../../locales/pt-PT.txt")),
    ("ro-RO", include_str!("../../locales/ro-RO.txt")),
    ("ru-RU", include_str!("../../locales/ru-RU.txt")),
    ("sk-SK", include_str!("../../locales/sk-SK.txt")),
    ("sl-SI", include_str!("../../locales/sl-SI.txt")),
    ("sq-AL", include_str!("../../locales/sq-AL.txt")),
    ("sr-Latn-RS", include_str!("../../locales/sr-Latn-RS.txt")),
    ("sv-SE", include_str!("../../locales/sv-SE.txt")),
    ("ta-IN", include_str!("../../locales/ta-IN.txt")),
    ("te-IN", include_str!("../../locales/te-IN.txt")),
    ("th-TH", include_str!("../../locales/th-TH.txt")),
    ("tr-TR", include_str!("../../locales/tr-TR.txt")),
    ("uk-UA", include_str!("../../locales/uk-UA.txt")),
    ("vi-VN", include_str!("../../locales/vi-VN.txt")),
    ("zh-CN", include_str!("../../locales/zh-CN.txt")),
    ("zh-TW", include_str!("../../locales/zh-TW.txt")),
];

/// Languages whose scripts egui can't draw: its text layout has neither bidirectional
/// text nor shaping. Arabic, Persian and Hebrew are written right to left (and the
/// Arabic script needs shaping too), and the Indic scripts, Khmer and Lao need shaping.
/// Windows users with one of these languages get the next language in their list, or
/// English.
const UNSUPPORTED: &[&str] = &[
    "ar-SA", "fa-IR", "he-IL", "hi-IN", "kn-IN", "km-KH", "lo-LA", "ml-IN", "ta-IN", "te-IN",
];

const ENGLISH: &str = "en-US";

/// The strings of one language, with English for anything missing.
struct Strings {
    language: &'static str,
    table: HashMap<&'static str, String>,
    english: HashMap<&'static str, String>,
}

static STRINGS: OnceLock<Strings> = OnceLock::new();

impl Strings {
    fn new(language: &'static str) -> Self {
        Self {
            language,
            table: parse(locale(language).unwrap_or_default()),
            english: parse(locale(ENGLISH).unwrap_or_default()),
        }
    }

    fn get(&'static self, key: &str) -> &'static str {
        match self.table.get(key).or_else(|| self.english.get(key)) {
            Some(text) => text,
            None => {
                debug_assert!(false, "no string {key:?}");
                ""
            }
        }
    }
}

fn locale(language: &str) -> Option<&'static str> {
    LOCALES
        .iter()
        .find(|(tag, _)| *tag == language)
        .map(|(_, text)| *text)
}

/// Reads `key=value` lines; values write line breaks as `\n` and backslashes as `\\`.
fn parse(text: &'static str) -> HashMap<&'static str, String> {
    text.lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| {
            let mut unescaped = String::with_capacity(value.len());
            let mut chars = value.chars();
            while let Some(c) = chars.next() {
                if c != '\\' {
                    unescaped.push(c);
                } else if let Some(next) = chars.next() {
                    unescaped.push(if next == 'n' { '\n' } else { next });
                }
            }
            (key, unescaped)
        })
        .collect()
}

/// Picks the language of the interface: the first of the user's preferred languages
/// (most preferred first, like `en-GB` or `zh-Hans-CN`) that has a translation egui
/// can draw, or English. Returns it.
pub(super) fn init(preferred: &[String]) -> &'static str {
    let language = preferred
        .iter()
        .find_map(|tag| match_language(tag))
        .unwrap_or(ENGLISH);
    STRINGS.get_or_init(|| Strings::new(language)).language
}

fn strings() -> &'static Strings {
    STRINGS.get_or_init(|| Strings::new(ENGLISH))
}

/// The translation of `key`, from the Windows Calculator's resources.
pub(super) fn tr(key: &str) -> &'static str {
    strings().get(key)
}

/// A translated format string with `%1`, `%2`, … replaced by `args`, the way the
/// Windows Calculator fills them in with `FormatMessage`.
pub(super) fn tr_format(key: &str, args: &[&str]) -> String {
    format_message(tr(key), args)
}

fn format_message(format: &str, args: &[&str]) -> String {
    let mut text = format.to_string();
    // From the highest number down, so `%1` doesn't match the start of `%10`.
    for (i, arg) in args.iter().enumerate().rev() {
        text = text.replace(&format!("%{}", i + 1), arg);
    }
    text
}

/// A unit's name in the drop-down lists (`key` is `Unit::key`).
pub(super) fn unit_name(key: &str) -> &'static str {
    tr(&format!("UnitName_{key}"))
}

/// A unit's short name in the "About equal to" results.
pub(super) fn unit_abbreviation(key: &str) -> &'static str {
    tr(&format!("UnitAbbreviation_{key}"))
}

/// The translation that best matches a language tag, like Windows' resource lookup:
/// the same tag, or the same language in another region. Chinese goes by script, and
/// Spanish, Portuguese, French and English pick the regional variant that fits best.
fn match_language(tag: &str) -> Option<&'static str> {
    let tag = tag.to_ascii_lowercase();
    let mut parts = tag.split(['-', '_']);
    let language = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    let has = |part: &str| rest.contains(&part);
    let tags = || LOCALES.iter().map(|(tag, _)| *tag);
    let chosen = match language {
        "zh" if has("hant") || has("tw") || has("hk") || has("mo") => "zh-TW",
        "zh" => "zh-CN",
        "sr" if has("cyrl") => return None,
        "sr" => "sr-Latn-RS",
        "no" | "nn" => "nb-NO",
        "es" if rest.is_empty() || has("es") => "es-ES",
        "es" => "es-MX",
        "pt" if has("pt") => "pt-PT",
        "pt" => "pt-BR",
        "fr" if has("ca") => "fr-CA",
        "fr" => "fr-FR",
        "en" if rest.is_empty() || has("us") => ENGLISH,
        "en" => "en-GB",
        _ => tags()
            .find(|candidate| candidate.to_ascii_lowercase() == tag)
            .or_else(|| tags().find(|candidate| candidate.split('-').next() == Some(language)))?,
    };
    (!UNSUPPORTED.contains(&chosen)).then_some(chosen)
}

/// The user's preferred display languages, most preferred first. The environment
/// variable `CALCCRAFT_LANGUAGE` (for example `de-DE`) overrides them.
pub(super) fn preferred_languages() -> Vec<String> {
    if let Ok(language) = std::env::var("CALCCRAFT_LANGUAGE") {
        return vec![language];
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
        let (mut count, mut length) = (0u32, 0u32);
        // SAFETY: the first call only asks for the buffer length; the second one writes at
        // most `length` UTF-16 units into a buffer of that length.
        let ok = unsafe {
            GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut count,
                std::ptr::null_mut(),
                &mut length,
            ) != 0
        };
        if ok {
            let mut buffer = vec![0u16; length as usize];
            // SAFETY: as above.
            let ok = unsafe {
                GetUserPreferredUILanguages(
                    MUI_LANGUAGE_NAME,
                    &mut count,
                    buffer.as_mut_ptr(),
                    &mut length,
                ) != 0
            };
            if ok {
                // Zero-terminated names, and an empty one at the end.
                return String::from_utf16_lossy(&buffer)
                    .split('\0')
                    .filter(|name| !name.is_empty())
                    .map(str::to_string)
                    .collect();
            }
        }
    }
    Vec::new()
}

/// A Windows font with the script of a language that Segoe UI doesn't cover, and its
/// index in the font collection (the "UI" variants of the CJK fonts).
pub(super) fn script_font(language: &str) -> Option<(&'static str, u32)> {
    match language {
        "ja-JP" => Some(("YuGothM.ttc", 1)),
        "zh-CN" => Some(("msyh.ttc", 1)),
        "zh-TW" => Some(("msjh.ttc", 1)),
        "ko-KR" => Some(("malgun.ttf", 0)),
        "th-TH" => Some(("LeelawUI.ttf", 0)),
        "am-ET" => Some(("ebrima.ttf", 0)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_every_string() {
        let english = parse(locale(ENGLISH).unwrap());
        assert!(english.len() > 400);
        for (language, text) in LOCALES {
            let table = parse(text);
            for key in english.keys() {
                assert!(table.contains_key(key), "{language} has no {key}");
            }
        }
    }

    #[test]
    fn languages_match_like_windows() {
        assert_eq!(match_language("de-DE"), Some("de-DE"));
        assert_eq!(match_language("de-AT"), Some("de-DE"));
        assert_eq!(match_language("en-US"), Some("en-US"));
        assert_eq!(match_language("en-AU"), Some("en-GB"));
        assert_eq!(match_language("es-AR"), Some("es-MX"));
        assert_eq!(match_language("es-ES"), Some("es-ES"));
        assert_eq!(match_language("pt-PT"), Some("pt-PT"));
        assert_eq!(match_language("pt-AO"), Some("pt-BR"));
        assert_eq!(match_language("fr-CH"), Some("fr-FR"));
        assert_eq!(match_language("fr-CA"), Some("fr-CA"));
        assert_eq!(match_language("zh-Hans-CN"), Some("zh-CN"));
        assert_eq!(match_language("zh-Hant-HK"), Some("zh-TW"));
        assert_eq!(match_language("sr-Latn-RS"), Some("sr-Latn-RS"));
        assert_eq!(match_language("sr-Cyrl-RS"), None);
        assert_eq!(match_language("nb-NO"), Some("nb-NO"));
        assert_eq!(match_language("ja"), Some("ja-JP"));
        // Not drawable without bidirectional text and shaping.
        assert_eq!(match_language("ar-SA"), None);
        assert_eq!(match_language("he-IL"), None);
        assert_eq!(match_language("hi-IN"), None);
        // Not translated.
        assert_eq!(match_language("xx-YY"), None);
    }

    #[test]
    fn format_strings() {
        assert_eq!(format_message("Display is %1", &["5"]), "Display is 5");
        assert_eq!(
            format_message("%1 %2", &["Length", "Converter"]),
            "Length Converter"
        );
    }

    #[test]
    fn english_strings() {
        assert_eq!(tr("HistoryLabel.Text"), "History");
        assert_eq!(tr("negateButton.AutomationName"), "Positive negative");
        assert_eq!(tr("CEngine.99"), "Cannot divide by zero");
        assert_eq!(unit_name("Meter"), "Meters");
        assert_eq!(unit_abbreviation("Banana"), "bananas");
    }
}
