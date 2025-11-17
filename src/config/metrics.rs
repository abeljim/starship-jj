use std::io::Write;

use jj_cli::command_error::CommandError;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::util::{Color, Style};

/// Prints the amount of changes in the working copy.
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct Metrics {
    /// Controls how the changes are rendered. Use {added}, {removed} and {changed} to render the number of changes.
    #[serde(default = "default_template")]
    template: String,

    // added_files: Style,
    // removed_files: Style,
    /// Controls how the number of changed files is rendered.
    #[serde(default = "default_changed_files")]
    changed_files: Metric,

    /// Controls how the number of added lines is rendered.
    #[serde(default = "default_added_lines")]
    added_lines: Metric,

    /// Controls how the number of removed lines is rendered.
    #[serde(default = "default_removed_lines")]
    removed_lines: Metric,

    #[serde(flatten, default = "default_style")]
    style: Style,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            style: default_style(),
            template: default_template(),
            changed_files: default_changed_files(),
            added_lines: default_added_lines(),
            removed_lines: default_removed_lines(),
        }
    }
}

fn default_removed_lines() -> Metric {
    Metric {
        style: default_removed_style(),
        prefix: "-".to_string(),
        ..Default::default()
    }
}

fn default_removed_style() -> Style {
    Style {
        color: Some(Color::Red),
        ..Default::default()
    }
}

fn default_added_lines() -> Metric {
    Metric {
        style: default_added_style(),
        prefix: "+".to_string(),
        ..Default::default()
    }
}

fn default_added_style() -> Style {
    Style {
        color: Some(Color::Green),
        ..Default::default()
    }
}

fn default_changed_files() -> Metric {
    Metric {
        style: default_changed_style(),
        ..Default::default()
    }
}

fn default_changed_style() -> Style {
    Style {
        color: Some(Color::Cyan),
        ..Default::default()
    }
}

fn default_template() -> String {
    "[{changed} {added}{removed}]".to_string()
}

fn default_style() -> Style {
    Style {
        color: Some(Color::Magenta),
        ..Default::default()
    }
}

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug, Default)]
struct Metric {
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(flatten)]
    style: Style,
}
impl Metric {
    fn format(
        &self,
        number: usize,
        global_style: &Style,
        fallback: impl Into<Option<Style>>,
        prev_style: &mut Option<nu_ansi_term::Style>,
    ) -> String {
        format!(
            "{}{}{}{}{}",
            self.style.format(fallback, prev_style),
            self.prefix,
            number,
            self.suffix,
            global_style.format(default_style(), prev_style),
        )
    }
}

#[derive(Debug, Serialize)]
struct Context {
    added: String,
    removed: String,
    changed: String,
}

impl Metrics {
    pub fn print(
        &self,
        io: &mut impl Write,
        data: &crate::JJData,
        module_separator: &str,
        prev_style: &mut Option<nu_ansi_term::Style>,
    ) -> Result<(), CommandError> {
        let Some(diff) = &data.commit.diff else {
            return Ok(());
        };

        let context = Context {
            added: self.added_lines.format(
                diff.lines_added,
                &self.style,
                default_added_style(),
                &mut None,
            ),
            removed: self.removed_lines.format(
                diff.lines_removed,
                &self.style,
                default_removed_style(),
                &mut None,
            ),
            changed: self.changed_files.format(
                diff.files_changed,
                &self.style,
                default_changed_style(),
                &mut None,
            ),
        };
        let mut tiny_template = tinytemplate::TinyTemplate::new();
        tiny_template
            .add_template("template", &self.template)
            .map_err(|e| {
                CommandError::with_message(
                    jj_cli::command_error::CommandErrorKind::Internal,
                    "template",
                    e,
                )
            })?;
        let s = tiny_template.render("template", &context).map_err(|e| {
            CommandError::with_message(
                jj_cli::command_error::CommandErrorKind::Internal,
                "template",
                e,
            )
        })?;

        *prev_style = None;
        self.style.print(io, default_style(), prev_style)?;

        write!(io, "{s}{module_separator}")?;

        Ok(())
    }

    pub(crate) fn parse(
        &self,
        command_helper: &jj_cli::cli_util::CommandHelper,
        state: &mut crate::State,
        data: &mut crate::JJData,
        _global: &super::GlobalConfig,
    ) -> Result<(), CommandError> {
        if data.commit.diff.is_some() {
            return Ok(());
        }

        let mut diff = crate::CommitDiff::default();

        let Some(stats) = state.diff_stats(command_helper)? else {
            return Ok(());
        };

        diff.files_changed = stats.entries().len();
        diff.lines_added = stats.count_total_added();
        diff.lines_removed = stats.count_total_removed();

        data.commit.diff = Some(diff);

        Ok(())
    }
}
