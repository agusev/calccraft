//! Dark and light color palettes.
//!
//! Keypad colors follow the Windows Calculator's `App.xaml`: white at a fixed
//! opacity over the window background. The purple tint of the backgrounds comes
//! from the reference screenshots, and the accent shades are the purple
//! Windows accent palette those screenshots were taken with.

use eframe::egui::Color32;

/// Every color the interface paints with, for one theme.
pub(super) struct Palette {
    pub background: Color32,
    pub digit: Color32,
    pub digit_hover: Color32,
    pub digit_pressed: Color32,
    pub digit_disabled: Color32,
    pub operator: Color32,
    pub operator_hover: Color32,
    pub operator_pressed: Color32,
    pub operator_disabled: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    pub accent_disabled: Color32,
    /// Transparent buttons (memory row, header icons) on hover and press.
    pub subtle_hover: Color32,
    pub subtle_pressed: Color32,
    /// Outline of the keypad buttons.
    pub border: Color32,
    /// History and memory panels and the navigation pane.
    pub flyout: Color32,
    pub flyout_border: Color32,
    /// Darkens the page behind the navigation pane.
    pub backdrop: Color32,
    pub text: Color32,
    pub text_pressed: Color32,
    pub text_secondary: Color32,
    pub text_disabled: Color32,
    pub text_on_accent: Color32,
    /// Cards on the Settings page.
    pub card: Color32,
    pub card_hover: Color32,
    pub card_pressed: Color32,
    pub card_border: Color32,
    pub radio_ring: Color32,
    pub radio_fill: Color32,
    pub radio_dot: Color32,
    pub link: Color32,
    pub link_hover: Color32,
    pub link_pressed: Color32,
}

/// `color` at `opacity` over `base`, as an opaque color.
const fn over(base: Color32, color: Color32, opacity: f32) -> Color32 {
    const fn channel(base: u8, top: u8, opacity: f32) -> u8 {
        (base as f32 + (top as f32 - base as f32) * opacity + 0.5) as u8
    }
    Color32::from_rgb(
        channel(base.r(), color.r(), opacity),
        channel(base.g(), color.g(), opacity),
        channel(base.b(), color.b(), opacity),
    )
}

const fn white(base: Color32, opacity: f32) -> Color32 {
    over(base, Color32::WHITE, opacity)
}

const fn black(base: Color32, opacity: f32) -> Color32 {
    over(base, Color32::BLACK, opacity)
}

// The purple Windows accent palette of the reference screenshots.
const ACCENT_LIGHT_3: Color32 = Color32::from_rgb(0xF0, 0xC0, 0xF4);
const ACCENT_LIGHT_2: Color32 = Color32::from_rgb(0xDB, 0x9E, 0xE5);
const ACCENT_DARK_1: Color32 = Color32::from_rgb(0x8E, 0x3A, 0xA7);
const ACCENT_DARK_2: Color32 = Color32::from_rgb(0x69, 0x27, 0x82);

const DARK_BACKGROUND: Color32 = Color32::from_rgb(0x29, 0x1A, 0x29);
/// Not sampled: Mica in light mode with the same purple wallpaper tint.
const LIGHT_BACKGROUND: Color32 = Color32::from_rgb(0xF3, 0xEB, 0xF3);

pub(super) const DARK: Palette = Palette {
    background: DARK_BACKGROUND,
    digit: white(DARK_BACKGROUND, 0.125),
    digit_hover: white(DARK_BACKGROUND, 0.0852),
    digit_pressed: white(DARK_BACKGROUND, 0.0359),
    digit_disabled: white(DARK_BACKGROUND, 0.0359),
    operator: white(DARK_BACKGROUND, 0.0852),
    operator_hover: white(DARK_BACKGROUND, 0.1256),
    operator_pressed: white(DARK_BACKGROUND, 0.0852),
    operator_disabled: white(DARK_BACKGROUND, 0.0359),
    accent: ACCENT_LIGHT_2,
    accent_hover: over(DARK_BACKGROUND, ACCENT_LIGHT_2, 0.9),
    accent_pressed: over(DARK_BACKGROUND, ACCENT_LIGHT_2, 0.8),
    accent_disabled: white(DARK_BACKGROUND, 0.1581),
    subtle_hover: white(DARK_BACKGROUND, 0.0605),
    subtle_pressed: white(DARK_BACKGROUND, 0.0419),
    border: Color32::from_rgb(0x38, 0x2A, 0x39),
    flyout: Color32::from_rgb(0x2F, 0x21, 0x2F),
    flyout_border: Color32::from_rgb(0x45, 0x37, 0x45),
    backdrop: Color32::from_black_alpha(0x6B),
    text: Color32::WHITE,
    text_pressed: Color32::from_rgb(0xCF, 0xC8, 0xCF),
    text_secondary: Color32::from_rgb(0xC5, 0xBE, 0xC5),
    text_disabled: Color32::from_rgb(0x6C, 0x62, 0x6D),
    text_on_accent: Color32::from_rgb(0x1A, 0x0F, 0x1B),
    card: Color32::from_rgb(0x34, 0x26, 0x35),
    card_hover: white(DARK_BACKGROUND, 0.0852),
    card_pressed: white(DARK_BACKGROUND, 0.0419),
    card_border: Color32::from_rgb(0x22, 0x18, 0x25),
    radio_ring: Color32::from_rgb(0x9D, 0x97, 0x9F),
    radio_fill: Color32::from_rgb(0x2C, 0x23, 0x33),
    radio_dot: Color32::BLACK,
    link: ACCENT_LIGHT_3,
    link_hover: over(DARK_BACKGROUND, ACCENT_LIGHT_3, 0.8),
    link_pressed: over(DARK_BACKGROUND, ACCENT_LIGHT_3, 0.6),
};

pub(super) const LIGHT: Palette = Palette {
    background: LIGHT_BACKGROUND,
    digit: Color32::WHITE,
    digit_hover: white(LIGHT_BACKGROUND, 0.75),
    digit_pressed: white(LIGHT_BACKGROUND, 0.5),
    digit_disabled: white(LIGHT_BACKGROUND, 0.125),
    operator: white(LIGHT_BACKGROUND, 0.5),
    operator_hover: white(LIGHT_BACKGROUND, 0.25),
    operator_pressed: white(LIGHT_BACKGROUND, 0.125),
    operator_disabled: white(LIGHT_BACKGROUND, 0.125),
    accent: ACCENT_DARK_1,
    accent_hover: over(LIGHT_BACKGROUND, ACCENT_DARK_1, 0.9),
    accent_pressed: over(LIGHT_BACKGROUND, ACCENT_DARK_1, 0.8),
    accent_disabled: black(LIGHT_BACKGROUND, 0.2169),
    subtle_hover: black(LIGHT_BACKGROUND, 0.0373),
    subtle_pressed: black(LIGHT_BACKGROUND, 0.0241),
    border: black(LIGHT_BACKGROUND, 0.0578),
    flyout: white(LIGHT_BACKGROUND, 0.6),
    flyout_border: black(LIGHT_BACKGROUND, 0.0578),
    backdrop: Color32::from_black_alpha(0x4D),
    text: Color32::BLACK,
    text_pressed: black(LIGHT_BACKGROUND, 0.6063),
    text_secondary: black(LIGHT_BACKGROUND, 0.6063),
    text_disabled: black(LIGHT_BACKGROUND, 0.3614),
    text_on_accent: Color32::WHITE,
    card: white(LIGHT_BACKGROUND, 0.7),
    card_hover: white(LIGHT_BACKGROUND, 0.5),
    card_pressed: black(LIGHT_BACKGROUND, 0.0241),
    card_border: black(LIGHT_BACKGROUND, 0.0578),
    radio_ring: black(LIGHT_BACKGROUND, 0.4458),
    radio_fill: black(LIGHT_BACKGROUND, 0.0241),
    radio_dot: Color32::WHITE,
    link: ACCENT_DARK_2,
    link_hover: over(LIGHT_BACKGROUND, ACCENT_DARK_2, 0.8),
    link_pressed: over(LIGHT_BACKGROUND, ACCENT_DARK_2, 0.6),
};

#[cfg(test)]
mod tests {
    use super::*;

    /// The opacities from `App.xaml` reproduce the colors sampled from the
    /// dark reference screenshot (`#433644` and `#3B2D3C`) to within one step.
    #[test]
    fn opacities_match_sampled_dark_buttons() {
        let close = |a: Color32, b: Color32| {
            let [ar, ag, ab, _] = a.to_array();
            let [br, bg, bb, _] = b.to_array();
            ar.abs_diff(br) <= 1 && ag.abs_diff(bg) <= 1 && ab.abs_diff(bb) <= 1
        };
        assert!(close(DARK.digit, Color32::from_rgb(0x43, 0x36, 0x44)));
        assert!(close(DARK.operator, Color32::from_rgb(0x3B, 0x2D, 0x3C)));
        assert!(close(
            DARK.operator_hover,
            Color32::from_rgb(0x43, 0x36, 0x44)
        ));
        assert!(close(
            DARK.subtle_hover,
            Color32::from_rgb(0x35, 0x27, 0x35)
        ));
    }

    #[test]
    fn over_blends_channels() {
        assert_eq!(white(Color32::BLACK, 0.5), Color32::from_rgb(128, 128, 128));
        assert_eq!(black(Color32::WHITE, 0.0), Color32::WHITE);
        assert_eq!(white(Color32::BLACK, 1.0), Color32::WHITE);
    }
}
