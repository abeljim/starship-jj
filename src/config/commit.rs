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
    /// A prefix that will be printed when the current commit is empty and the previous commit is shown
    /// usually becasue of Squash Workflow
    #[serde(default = "default_previous_message_symbol")]
    previous_message_symbol: char,
    /// Maximum length the commit text will be truncated to.
    #[serde(default = "default_max_length")]
    max_length: Option<usize>,
    /// Show the previous commits description in case current is empty
    /// This will also print the previous_message_symbol
    show_previous_if_empty: bool,
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

fn default_previous_message_symbol() -> char {
    '⇣'
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
            show_previous_if_empty: false,
            style: Default::default(),
            max_length: default_max_length(),
            empty_text: default_empty_text(),
            surround_with_quotes: true,
            previous_message_symbol: default_previous_message_symbol(),
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

        self.style.print(io, None, prev_style)?;

        if !desc.is_empty() {
            crate::print_ansi_truncated(
                self.max_length,
                io,
                first_line,
                self.surround_with_quotes,
            )?;
        } else {
            crate::print_ansi_truncated(
                self.max_length,
                io,
                &self.empty_text,
                self.surround_with_quotes,
            )?;
        }
        if data.commit.ahead {
            write!(io, "{}", self.previous_message_symbol)?;
        }
        write!(io, "{module_separator}")?;
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

        let description = commit.description().to_string();
        if description.is_empty() && self.show_previous_if_empty {
            let parents = state.parent_commits(command_helper)?;
            if parents.len() != 1 {
                return Ok(());
            };

            let parent = parents.first().expect("We already checked the vec length");

            data.commit.desc = Some(parent.description().to_string());
            data.commit.ahead = true;
        } else {
            data.commit.desc = Some(description);
        }
        Ok(())
    }
}
