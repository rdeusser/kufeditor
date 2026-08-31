use gpui::{Hsla, rgb};

pub struct Theme {
    pub background: Hsla,
    pub surface: Hsla,
    pub raised: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub text_dim: Hsla,
    pub accent: Hsla,
    pub accent_dim: Hsla,
    pub success: Hsla,
    pub warning: Hsla,
    pub danger: Hsla,
}

impl Theme {
    #[allow(
        clippy::unreadable_literal,
        reason = "RGB hex colors are read as six-digit design tokens"
    )]
    pub fn native_utility() -> Self {
        Self {
            background: rgb(0x151b21).into(),
            surface: rgb(0x171d24).into(),
            raised: rgb(0x202832).into(),
            border: rgb(0x303a46).into(),
            text: rgb(0xe7ecf3).into(),
            text_dim: rgb(0x93a0af).into(),
            accent: rgb(0x90bff8).into(),
            accent_dim: rgb(0x26497f).into(),
            success: rgb(0xafe0a4).into(),
            warning: rgb(0xe5c07b).into(),
            danger: rgb(0xe06c75).into(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::native_utility()
    }
}

#[cfg(test)]
mod tests {
    use gpui::Hsla;

    use super::Theme;

    fn relative_luminance(color: Hsla) -> f32 {
        let color = color.to_rgb();
        let channel = |value: f32| {
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * channel(color.r) + 0.7152 * channel(color.g) + 0.0722 * channel(color.b)
    }

    fn contrast(first: Hsla, second: Hsla) -> f32 {
        let first = relative_luminance(first);
        let second = relative_luminance(second);
        (first.max(second) + 0.05) / (first.min(second) + 0.05)
    }

    #[test]
    fn native_utility_text_colors_meet_normal_text_contrast() {
        let theme = Theme::native_utility();

        assert!(contrast(theme.accent, theme.surface) >= 4.5);
        assert!(contrast(theme.text, theme.accent_dim) >= 4.5);
    }
}
