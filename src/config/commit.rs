use std::io::Write;

use jj_cli::command_error::CommandError;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::util::Style;

/// Prints the working copy's commit text.
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct Commit {
    /// Maximum length the commit text will be truncated to.
    #[serde(default = "default_max_length")]
    max_length: Option<usize>,
    /// The text that should be printed when the current revision has no description yet.
    #[serde(default = "default_empty_text")]
    empty_text: String,
    /// Controls how the commit text is rendered.
    #[serde(flatten)]
    style: Style,
    /// Render quotes around the description.
    #[serde(default = "default_surround_with_quotes")]
    surround_with_quotes: bool,
}

fn default_max_length() -> Option<usize> {
    Some(24)
}
fn default_empty_text() -> String {
    "(no description set)".to_string()
}

fn default_surround_with_quotes() -> bool {
    true
}

impl Default for Commit {
    fn default() -> Self {
        Self {
            style: Default::default(),
            max_length: default_max_length(),
            empty_text: default_empty_text(),
            surround_with_quotes: true,
        }
    }
}

impl Commit {
    pub fn print(
        &self,
        io: &mut impl Write,
        data: &crate::JJData,
        module_separator: &str,
        prev_style: &mut Option<nu_ansi_term::Style>,
    ) -> Result<(), CommandError> {
        let Some(desc) = data.commit.desc.as_ref() else {
            return Ok(());
        };

        let first_line = desc
            .split_once(['\r', '\n'])
            .map(|(line, _rest)| line)
            .unwrap_or(desc);

        if !first_line.is_empty() {
            self.style.print(io, None, prev_style)?;

            crate::print_ansi_truncated(
                self.max_length,
                io,
                first_line,
                self.surround_with_quotes,
            )?;
            write!(io, "{module_separator}")?;
        } else {
            self.style.print(io, None, prev_style)?;
            crate::print_ansi_truncated(
                self.max_length,
                io,
                &self.empty_text,
                self.surround_with_quotes,
            )?;
            write!(io, "{module_separator}")?;
        }
        Ok(())
    }
    pub(crate) fn parse(
        &self,
        command_helper: &jj_cli::cli_util::CommandHelper,
        state: &mut crate::State,
        data: &mut crate::JJData,
        _global: &super::GlobalConfig,
    ) -> Result<(), CommandError> {
        if data.commit.desc.is_some() {
            return Ok(());
        }
        let Some(commit) = state.commit(command_helper)? else {
            return Ok(());
        };
        data.commit.desc = Some(commit.description().to_string());
        Ok(())
    }
}
