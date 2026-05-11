use std::io::Write;

use jj_cli::command_error::CommandError;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::util::{Color, Style};

/// Prints information about bookmarks in the working copy's ancestors.
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct Bookmarks {
    /// Text that will be rendered between each bookmark.
    #[serde(default = "default_separator")]
    separator: String,
    /// Controls how bookmarks are rendered.
    #[serde(flatten)]
    style: Style,
    /// Controls how untracked remote bookmarks are rendered.
    #[serde(default = "default_untracked_style")]
    untracked: Style,
    /// A suffix that will be printed when the given bookmark is behind the working copy.
    #[serde(default = "default_behind_symbol")]
    behind_symbol: Option<char>,
    /// Maximum amount of bookmarks that will be rendered.
    #[serde(default = "default_max_bookmarks")]
    max_bookmarks: Option<usize>,
    /// Maximum length the bookmark name will be truncated to.
    max_length: Option<usize>,
    /// Do not render quotes around bookmark names.
    #[serde(default = "default_surround_with_quotes")]
    surround_with_quotes: bool,
    /// Ignore Commits without a description.
    #[serde(default = "default_ignore_empty_commits")]
    ignore_empty_commits: IgnoreEmpty,
}

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub enum IgnoreEmpty {
    /// None=> [default] Count all commits even ones without description
    None,
    /// Current => Don't count the current commit if it's description is empty
    Current,
    /// All => Don't count any commits without description
    All,
}

fn default_ignore_empty_commits() -> IgnoreEmpty {
    IgnoreEmpty::None
}

fn default_style() -> Style {
    Style {
        color: Some(Color::Magenta),
        ..Default::default()
    }
}

fn default_untracked_style() -> Style {
    Style {
        color: Some(Color::Yellow),
        ..Default::default()
    }
}

fn default_behind_symbol() -> Option<char> {
    Some('⇡')
}

fn default_separator() -> String {
    " ".to_string()
}

fn default_max_bookmarks() -> Option<usize> {
    Some(1)
}

fn default_surround_with_quotes() -> bool {
    false
}

impl Default for Bookmarks {
    fn default() -> Self {
        Self {
            style: default_style(),
            untracked: default_untracked_style(),
            behind_symbol: default_behind_symbol(),
            max_bookmarks: default_max_bookmarks(),
            separator: default_separator(),
            max_length: Default::default(),
            surround_with_quotes: false,
            ignore_empty_commits: default_ignore_empty_commits(),
        }
    }
}

impl Bookmarks {
    pub fn print(
        &self,
        io: &mut impl Write,
        data: &crate::JJData,
        module_separator: &str,
        prev_style: &mut Option<nu_ansi_term::Style>,
    ) -> Result<(), CommandError> {
        let Some(bookmarks) = data.bookmarks.bookmarks.as_ref() else {
            unreachable!()
        };

        if self.max_bookmarks == Some(0) {
            return Ok(());
        }

        let Some(bookmark) = bookmarks.first() else {
            return Ok(());
        };

        match bookmark.kind {
            crate::BookmarkKind::Tracked => self.style.print(io, default_style(), prev_style)?,
            crate::BookmarkKind::Untracked => {
                self.untracked
                    .print(io, default_untracked_style(), prev_style)?;
            }
        }

        crate::print_ansi_truncated(
            self.max_length,
            io,
            &bookmark.name,
            self.surround_with_quotes,
        )?;

        if bookmark.distance != 0 {
            match self.behind_symbol {
                Some(s) => write!(io, "{s}{}", bookmark.distance)?,
                None => write!(io, "{}", bookmark.distance)?,
            }
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
        if data.bookmarks.bookmarks.is_some() {
            return Ok(());
        }

        let workspace_helper = state.workspace_helper(command_helper)?;
        let view = workspace_helper.repo().view();

        let mut bookmarks = Vec::new();

        crate::find_parent_bookmarks(workspace_helper, view, &global.bookmarks, &mut bookmarks)?;

        data.bookmarks = crate::BookmarkData {
            bookmarks: Some(bookmarks),
        };
        Ok(())
    }
}
