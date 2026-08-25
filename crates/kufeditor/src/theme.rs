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
}

impl Theme {
    #[allow(
        clippy::unreadable_literal,
        reason = "RGB hex colors are read as six-digit design tokens"
    )]
    pub fn forged_steel() -> Self {
        Self {
            background: rgb(0x12171a).into(),
            surface: rgb(0x192023).into(),
            raised: rgb(0x222a2d).into(),
            border: rgb(0x313a3d).into(),
            text: rgb(0xe3e0d7).into(),
            text_dim: rgb(0x908b7d).into(),
            accent: rgb(0xc6a15b).into(),
            accent_dim: rgb(0x393224).into(),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::forged_steel()
    }
}
