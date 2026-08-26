#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum STGText<'a> {
    Decoded(&'a str),
    Raw(&'a [u8]),
}

impl<'a> STGText<'a> {
    pub const fn decoded(self) -> Option<&'a str> {
        match self {
            Self::Decoded(value) => Some(value),
            Self::Raw(_) => None,
        }
    }

    pub const fn raw(self) -> Option<&'a [u8]> {
        match self {
            Self::Decoded(_) => None,
            Self::Raw(value) => Some(value),
        }
    }
}
