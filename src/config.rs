use std::{
    io::Write,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use bookmarks::Bookmarks;
use commit::Commit;
use jj_cli::command_error::CommandError;
use metrics::Metrics;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use state::State;
use symbol::Symbol;
use util::Glob;

pub mod util;

mod bookmarks;
mod commit;
mod metrics;
mod state;
mod symbol;

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct Config {
    #[serde(flatten, default)]
    global: GlobalConfig,
    /// Modules that will be rendered.
    #[serde(rename = "module", default = "default_modules")]
    modules: Vec<ModuleConfig>,
}

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct GlobalConfig {
    /// Text that will be printed between each Module.
    #[serde(default = "default_separator")]
    module_separator: String,
    /// Timeout after which the process is teminated.
    #[serde(default)]
    timeout: Option<u64>,
    /// Controls the behaviour of the bookmark finding algorithm.
    #[serde(default)]
    pub bookmarks: BookmarkConfig,
    /// Controls whether color gets reset at the end.
    #[serde(default = "default_reset_color")]
    pub reset_color: bool,
}

fn default_separator() -> String {
    " ".to_string()
}

fn default_reset_color() -> bool {
    true
}

fn default_modules() -> Vec<ModuleConfig> {
    vec![
        ModuleConfig::Symbol(Default::default()),
        ModuleConfig::Bookmarks(Default::default()),
        ModuleConfig::Commit(Default::default()),
        ModuleConfig::State(Default::default()),
        ModuleConfig::Metrics(Default::default()),
    ]
}

#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
pub struct BookmarkConfig {
    /// Controls how far we are looking back to find bookmarks.
    #[serde(default = "default_search_depth")]
    pub search_depth: usize,
    /// Exclude certain bookmarks from the search (supports globs).
    #[serde(default)]
    #[cfg(feature = "json-schema")]
    pub exclude: Vec<String>,
    #[serde(default)]
    #[cfg(not(feature = "json-schema"))]
    pub exclude: Vec<Glob>,
}

impl Default for BookmarkConfig {
    fn default() -> Self {
        Self {
            search_depth: default_search_depth(),
            exclude: Default::default(),
        }
    }
}

fn default_search_depth() -> usize {
    100
}

impl Config {
    pub fn print(
        &self,
        command_helper: &&jj_cli::cli_util::CommandHelper,
        state: &mut crate::State,
        data: &mut crate::JJData,
    ) -> Result<(), CommandError> {
        let done = Arc::new(AtomicBool::new(false));

        let done2 = done.clone();
        if let Some(timeout) = self.global.timeout {
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(timeout));
                if !done2.load(std::sync::atomic::Ordering::Relaxed) {
                    _ = util::Style::default().print(&mut std::io::stdout(), None, &mut None);
                    print!(" ");
                    let _ = std::io::stdout().flush();
                    std::process::exit(0);
                }
            });
        }
        let mut io = std::io::stdout();
        let mut prev_style = None;
        for module in self.modules.iter() {
            match module {
                ModuleConfig::Bookmarks(bookmarks) => {
                    bookmarks.parse(command_helper, state, data, &self.global)?;
                    let mut io = io.lock();
                    bookmarks.print(
                        &mut io,
                        data,
                        &self.global.module_separator,
                        &mut prev_style,
                    )?;
                }
                ModuleConfig::Commit(commit_desc) => {
                    commit_desc.parse(command_helper, state, data, &self.global)?;
                    let mut io = io.lock();
                    commit_desc.print(
                        &mut io,
                        data,
                        &self.global.module_separator,
                        &mut prev_style,
                    )?
                }
                ModuleConfig::State(commit_warnings) => {
                    commit_warnings.parse(command_helper, state, data, &self.global)?;
                    let mut io = io.lock();
                    commit_warnings.print(
                        &mut io,
                        data,
                        &self.global.module_separator,
                        &mut prev_style,
                    )?
                }
                ModuleConfig::Metrics(commit_diff) => {
                    commit_diff.parse(command_helper, state, data, &self.global)?;
                    let mut io = io.lock();
                    commit_diff.print(
                        &mut io,
                        data,
                        &self.global.module_separator,
                        &mut prev_style,
                    )?
                }
                ModuleConfig::Symbol(symbol) => {
                    symbol.parse(command_helper, state, data, &self.global)?;
                    let mut io = io.lock();
                    symbol.print(
                        &mut io,
                        data,
                        &self.global.module_separator,
                        &mut prev_style,
                    )?
                }
            }
        }
        if self.global.reset_color {
            util::Style::default().print(&mut io, None, &mut prev_style)?;
        }
        Ok(())
    }
}

/// A module that prints some info about the current jj repo.
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "type")]
enum ModuleConfig {
    Symbol(Symbol),
    Bookmarks(Bookmarks),
    Commit(Commit),
    State(State),
    Metrics(Metrics),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            global: GlobalConfig {
                timeout: Default::default(),
                module_separator: default_separator(),
                bookmarks: Default::default(),
                reset_color: Default::default(),
            },
            modules: default_modules(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;

    #[test]
    fn parse_provided_config() {
        let config = include_str!("../starship-jj.toml");
        let c: Config = toml::from_str(config).unwrap();

        assert_debug_snapshot!(c);
    }

    #[test]
    fn parse_generated_config() {
        let d = Config::default();
        let s = toml::to_string(&d).unwrap();
        let c: Config = toml::from_str(&s).unwrap();

        assert_debug_snapshot!(c);
    }

    #[test]
    fn parse_minimal_config1() {
        let minimal = r#""#;
        let _c: Config = toml::from_str(minimal).unwrap();
    }
    #[test]
    fn parse_minimal_config2() {
        let minimal = r#"
[[module]]
type = "Bookmarks"

[[module]]
type = "Commit"

[[module]]
type = "State"

[[module]]
type = "Metrics"
        "#;
        let _c: Config = toml::from_str(minimal).unwrap();
    }
    #[test]
    fn parse_minimal_config3() {
        let minimal = r#"
[[module]]
type = "Bookmarks"

[[module]]
type = "Commit"

[[module]]
type = "State"
[module.conflict]
text = "(CONFLICT)"
[module.divergent]
text = "(DIVERGENT)"
[module.hidden]
text = "(HIDDEN)"
[module.immutable]
text = "(IMMUTABLE)"
[module.empty]
text = "(EMPTY)"

[[module]]
type = "Metrics"
[module.changed_files]
[module.added_lines]
[module.removed_lines]
        "#;
        let _c: Config = toml::from_str(minimal).unwrap();
    }
}
