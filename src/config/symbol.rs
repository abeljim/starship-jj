use std::io::Write;

use super::util::{Color, Style};
use jj_cli::command_error::CommandError;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Prints an indicator.
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct Symbol {
    /// Text that will be rendered between each bookmark.
    #[serde(default = "default_symbol")]
    symbol: String,
    /// Controls how the symbol is rendered.
    #[serde(flatten)]
    style: Style,
}
impl Symbol {
    pub fn print(
        &self,
        io: &mut impl Write,
        _data: &crate::JJData,
        module_separator: &str,
        prev_style: &mut Option<nu_ansi_term::Style>,
    ) -> Result<(), CommandError> {
        self.style.print(io, default_style(), prev_style)?;

        write!(io, "{}{module_separator}", self.symbol)?;
        Ok(())
    }

    pub(crate) fn parse(
        &self,
        _command_helper: &jj_cli::cli_util::CommandHelper,
        _state: &mut crate::State,
        _data: &mut crate::JJData,
        _global: &super::GlobalConfig,
    ) -> Result<(), CommandError> {
        Ok(())
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Self {
            symbol: default_symbol(),
            style: default_style(),
        }
    }
}
fn default_style() -> Style {
    Style {
        color: Some(Color::Blue),
        ..Default::default()
    }
}

fn default_symbol() -> String {
    "󱗆 ".to_string()
}
