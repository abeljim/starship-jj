use std::io::Write;

use jj_cli::command_error::CommandError;
use jj_lib::id_prefix::IdPrefixIndex;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config::util::Color;

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
    #[serde(default)]
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
    /// Controls if and how the Change Id should be shown
    change: Option<Style>,
    /// Controls if and how the Commit Id should be shown
    commit: Option<Style>,
    /// Controls how the non unique part of  Ids should be shown
    #[serde(default = "default_non_unique_style")]
    non_unique: Style,
}

fn default_non_unique_style() -> Style {
    Style {
        color: Some(Color::Black),
        ..Default::default()
    }
}

fn default_unique_change_style() -> Style {
    Style {
        color: Some(Color::Magenta),
        ..Default::default()
    }
}

fn default_unique_commit_style() -> Style {
    Style {
        color: Some(Color::Blue),
        ..Default::default()
    }
}

fn default_previous_message_symbol() -> char {
    '⇣'
}
fn default_max_length() -> Option<usize> {
    Some(20)
}
fn default_empty_text() -> String {
    "󰆇".to_string()
}

fn default_surround_with_quotes() -> bool {
    false
}

impl Default for Commit {
    fn default() -> Self {
        Self {
            show_previous_if_empty: false,
            style: Default::default(),
            max_length: default_max_length(),
            empty_text: default_empty_text(),
            surround_with_quotes: false,
            previous_message_symbol: default_previous_message_symbol(),
            commit: None,
            change: None,
            non_unique: default_non_unique_style(),
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
        let mut first = true;
        if let (Some(change), Some((change_id, change_idx))) =
            (&self.change, &data.commit.change_id)
        {
            change.print(io, default_unique_change_style(), prev_style)?;
            let short_change_id = &change_id.to_string()[..8];
            let (unique, non_unique) = short_change_id.split_at(*change_idx);
            write!(io, "{unique}")?;
            self.non_unique
                .print(io, default_non_unique_style(), prev_style)?;
            write!(io, "{non_unique}")?;
            first = false;
        }
        if let (Some(commit), Some((commit_id, commit_idx))) =
            (&self.commit, &data.commit.commit_id)
        {
            if !first {
                write!(io, " ")?;
            }
            commit.print(io, default_unique_commit_style(), prev_style)?;
            let short_commit_id = &commit_id.to_string()[..8];
            let (unique, non_unique) = short_commit_id.split_at(*commit_idx);
            write!(io, "{unique}")?;
            self.non_unique
                .print(io, default_non_unique_style(), prev_style)?;
            write!(io, "{non_unique}")?;
            first = false;
        }

        let Some(desc) = data.commit.desc.as_ref() else {
            return Ok(());
        };

        if !first {
            write!(io, " ")?;
        }
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
        global: &super::GlobalConfig,
    ) -> Result<(), CommandError> {
        self.resolve_desc(command_helper, state, data, global)?;

        if self.commit.is_some() {
            self.resolve_commit_id(command_helper, state, data, global)?;
        }
        if self.change.is_some() {
            self.resolve_change_id(command_helper, state, data, global)?;
        }

        Ok(())
    }

    fn resolve_commit_id(
        &self,
        command_helper: &jj_cli::cli_util::CommandHelper,
        state: &mut crate::State,
        data: &mut crate::JJData,
        _global: &super::GlobalConfig,
    ) -> Result<(), CommandError> {
        if data.commit.commit_id.is_some() {
            return Ok(());
        }
        let repo = state.repo(command_helper)?;
        let Some(commit) = state.commit(command_helper)? else {
            return Ok(());
        };
        let commit_id = commit.id().clone();
        let commit_idx =
            IdPrefixIndex::empty().shortest_commit_prefix_len(repo.as_ref(), &commit_id)?;
        data.commit.commit_id = Some((commit_id, commit_idx));
        Ok(())
    }

    fn resolve_change_id(
        &self,
        command_helper: &jj_cli::cli_util::CommandHelper,
        state: &mut crate::State,
        data: &mut crate::JJData,
        _global: &super::GlobalConfig,
    ) -> Result<(), CommandError> {
        if data.commit.change_id.is_some() {
            return Ok(());
        }
        let repo = state.repo(command_helper)?;
        let Some(commit) = state.commit(command_helper)? else {
            return Ok(());
        };
        let change_id = commit.change_id().clone();
        let change_idx =
            IdPrefixIndex::empty().shortest_change_prefix_len(repo.as_ref(), &change_id)?;
        data.commit.change_id = Some((change_id, change_idx));
        Ok(())
    }

    fn resolve_desc(
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
