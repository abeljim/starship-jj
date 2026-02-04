use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
};

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

        self.style.print(io, default_style(), prev_style)?;

        let mut ordered: BTreeMap<usize, BTreeSet<&String>> = BTreeMap::new();

        for (name, behind) in bookmarks {
            ordered
                .entry(*behind)
                .and_modify(|s| {
                    s.insert(name);
                })
                .or_insert_with(|| {
                    let mut s = BTreeSet::new();
                    s.insert(name);
                    s
                });
        }

        let mut counter = 0;
        'outer: for (behind, bookmarks) in ordered {
            for name in bookmarks {
                if let Some(number) = self.max_bookmarks
                    && counter >= number
                {
                    write!(io, "{}…{module_separator}", self.separator)?;
                    // set counter to 0 so we don't print the module separator twice
                    counter = 0;
                    break 'outer;
                }
                if counter > 0 {
                    write!(io, "{}", self.separator)?;
                }
                crate::print_ansi_truncated(self.max_length, io, name, self.surround_with_quotes)?;

                if behind != 0 {
                    match self.behind_symbol {
                        Some(s) => write!(io, "{s}{behind}")?,
                        None => write!(io, "{behind}")?,
                    }
                }
                counter += 1;
            }
        }
        if counter != 0 {
            write!(io, "{module_separator}")?;
        }

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

        let mut bookmarks = BTreeMap::new();

        crate::find_parent_bookmarks(workspace_helper, view, &global.bookmarks, &mut bookmarks)?;

        data.bookmarks = crate::BookmarkData {
            bookmarks: Some(bookmarks),
        };
        Ok(())
    }
}
