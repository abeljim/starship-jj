use glob::Pattern;
use jj_cli::command_error::CommandError;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(try_from = "&str", into = "String")]
pub struct Glob(glob::Pattern);
impl TryFrom<&str> for Glob {
    type Error = glob::PatternError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(Self(Pattern::new(value)?))
    }
}
impl From<Glob> for String {
    fn from(value: Glob) -> Self {
        value.0.as_str().to_string()
    }
}

impl Glob {
    pub fn matches(&self, haystack: &str) -> bool {
        self.0.matches(haystack)
    }
}

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct Style {
    /// Text Color
    pub color: Option<Color>,
    /// Background Color
    pub bg_color: Option<Color>,

    #[serde(flatten)]
    pub attributes: TextAttributess,
}

impl Style {
    fn merge_with_fallback(&self, fallback: Option<Self>) -> Self {
        let Some(fallback) = fallback else {
            return self.clone();
        };

        Self {
            color: self.color.or(fallback.color),
            bg_color: self.bg_color.or(fallback.bg_color),
            attributes: TextAttributess {
                bold: self.attributes.bold.or(fallback.attributes.bold),
                dimmed: self.attributes.dimmed.or(fallback.attributes.dimmed),
                italic: self.attributes.italic.or(fallback.attributes.italic),
                underline: self.attributes.underline.or(fallback.attributes.underline),
                blink: self.attributes.blink.or(fallback.attributes.blink),
                reverse: self.attributes.reverse.or(fallback.attributes.reverse),
                hidden: self.attributes.hidden.or(fallback.attributes.hidden),
                strikethrough: self
                    .attributes
                    .strikethrough
                    .or(fallback.attributes.strikethrough),
            },
        }
    }

    pub fn print(
        &self,
        io: &mut impl Write,
        fallback: impl Into<Option<Style>>,
        prev: &mut Option<nu_ansi_term::Style>,
    ) -> Result<(), CommandError> {
        let prefix = self.format(fallback, prev);

        write!(io, "{prefix}")?;

        Ok(())
    }

    pub fn format(
        &self,
        fallback: impl Into<Option<Style>>,
        prev: &mut Option<nu_ansi_term::Style>,
    ) -> String {
        let s: nu_ansi_term::Style = self.merge_with_fallback(fallback.into()).into();

        let prefix = match prev {
            Some(prev) => prev.infix(s).to_string(),
            None => s.prefix().to_string(),
        };

        *prev = Some(s);
        prefix
    }
}

impl From<&Style> for nu_ansi_term::Style {
    fn from(value: &Style) -> Self {
        nu_ansi_term::Style {
            foreground: value.color.map(Into::into),
            background: value.bg_color.map(Into::into),
            is_bold: value.attributes.bold.unwrap_or_default(),
            is_dimmed: value.attributes.dimmed.unwrap_or_default(),
            is_italic: value.attributes.italic.unwrap_or_default(),
            is_underline: value.attributes.underline.unwrap_or_default(),
            is_blink: value.attributes.blink.unwrap_or_default(),
            is_reverse: value.attributes.reverse.unwrap_or_default(),
            is_hidden: value.attributes.hidden.unwrap_or_default(),
            is_strikethrough: value.attributes.strikethrough.unwrap_or_default(),
            prefix_with_reset: true,
        }
    }
}
impl From<Style> for nu_ansi_term::Style {
    fn from(value: Style) -> Self {
        nu_ansi_term::Style::from(&value)
    }
}

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug, Clone, Copy, Default)]
#[allow(clippy::enum_variant_names)]
pub struct TextAttributess {
    #[serde(default)]
    bold: Option<bool>,
    #[serde(default)]
    dimmed: Option<bool>,
    #[serde(default)]
    italic: Option<bool>,
    #[serde(default)]
    underline: Option<bool>,
    #[serde(default)]
    blink: Option<bool>,
    #[serde(default)]
    reverse: Option<bool>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    strikethrough: Option<bool>,
}

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
#[allow(clippy::enum_variant_names)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    TrueColor { r: u8, g: u8, b: u8 },
}

impl From<Color> for nu_ansi_term::Color {
    fn from(value: Color) -> Self {
        match value {
            Color::Black => nu_ansi_term::Color::Black,
            Color::Red => nu_ansi_term::Color::Red,
            Color::Green => nu_ansi_term::Color::Green,
            Color::Yellow => nu_ansi_term::Color::Yellow,
            Color::Blue => nu_ansi_term::Color::Blue,
            Color::Magenta => nu_ansi_term::Color::Magenta,
            Color::Cyan => nu_ansi_term::Color::Cyan,
            Color::White => nu_ansi_term::Color::White,
            Color::BrightBlack => nu_ansi_term::Color::DarkGray,
            Color::BrightRed => nu_ansi_term::Color::LightRed,
            Color::BrightGreen => nu_ansi_term::Color::LightGreen,
            Color::BrightYellow => nu_ansi_term::Color::LightYellow,
            Color::BrightBlue => nu_ansi_term::Color::LightBlue,
            Color::BrightMagenta => nu_ansi_term::Color::LightMagenta,
            Color::BrightCyan => nu_ansi_term::Color::LightCyan,
            Color::BrightWhite => nu_ansi_term::Color::LightGray,
            Color::TrueColor { r, g, b } => nu_ansi_term::Color::Rgb(r, g, b),
        }
    }
}

// impl From<colored::Color> for Color {
//     fn from(value: colored::Color) -> Self {
//         match value {
//             colored::Color::Black => Color::Black,
//             colored::Color::Red => Color::Red,
//             colored::Color::Green => Color::Green,
//             colored::Color::Yellow => Color::Yellow,
//             colored::Color::Blue => Color::Blue,
//             colored::Color::Magenta => Color::Magenta,
//             colored::Color::Cyan => Color::Cyan,
//             colored::Color::White => Color::White,
//             colored::Color::BrightBlack => Color::BrightBlack,
//             colored::Color::BrightRed => Color::BrightRed,
//             colored::Color::BrightGreen => Color::BrightGreen,
//             colored::Color::BrightYellow => Color::BrightYellow,
//             colored::Color::BrightBlue => Color::BrightBlue,
//             colored::Color::BrightMagenta => Color::BrightMagenta,
//             colored::Color::BrightCyan => Color::BrightCyan,
//             colored::Color::BrightWhite => Color::BrightWhite,
//             colored::Color::TrueColor { r, g, b } => Color::TrueColor { r, g, b },
//         }
//     }
// }

#[cfg(test)]
mod tests {}
