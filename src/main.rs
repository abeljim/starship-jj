use std::{cmp::Ordering, collections::HashSet, io::Write, path::PathBuf, process::ExitCode};

use ::config::Environment;
use args::{ConfigCommands, CustomCommand, StarshipCommands};
use config::BookmarkConfig;
use etcetera::BaseStrategy as _;
use jj_cli::{
    cli_util::{CliRunner, CommandHelper, RevisionArg, WorkspaceCommandHelper},
    command_error::{CommandError, user_error},
    ui::Ui,
};
use jj_lib::{
    backend::{ChangeId, CommitId},
    object_id::ObjectId,
    view::View,
};

pub use state::State;
use unicode_width::UnicodeWidthStr as _;

mod args;
mod config;
mod state;

pub mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn starship(
    ui: &mut Ui,
    command_helper: &CommandHelper,
    command: CustomCommand,
) -> Result<(), CommandError> {
    #[cfg(feature = "json-schema")]
    {
        let schema = schemars::schema_for!(config::Config);
        println!("{}", serde_json::to_string_pretty(&schema).unwrap());
        return Ok(());
    }

    let CustomCommand::Starship(args) = command;
    match args.command {
        StarshipCommands::Prompt { starship_config } => {
            print_prompt(command_helper, &starship_config)?
        }
        StarshipCommands::Config(ConfigCommands::Path) => {
            let config_dir = get_config_path()?;

            writeln!(ui.stdout(), "{config_dir}")?;
        }
        StarshipCommands::Config(ConfigCommands::Default) => {
            let c = toml::to_string_pretty(&config::Config::default()).map_err(user_error)?;

            writeln!(ui.stdout(), "{c}")?;
        }
    }

    Ok(())
}

fn get_config_path() -> Result<String, CommandError> {
    let config_dir = etcetera::choose_base_strategy()
        .ok()
        .map(|s| s.config_dir())
        .ok_or_else(|| user_error("Failed to find config dir"))?;
    let config_dir = config_dir.join("starship-jj/starship-jj.toml");
    let config_dir = config_dir
        .to_str()
        .ok_or_else(|| user_error("The config path is not valid UTF-8"))?;
    Ok(config_dir.to_string())
}

#[derive(Default)]
struct JJData {
    bookmarks: BookmarkData,
    commit: CommitData,
}

#[derive(Default)]
struct BookmarkData {
    bookmarks: Option<Vec<Bookmark>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Bookmark {
    name: String,
    distance: usize,
    kind: BookmarkKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BookmarkKind {
    Tracked,
    Untracked,
}

#[derive(Default)]
struct CommitData {
    desc: Option<String>,
    warnings: CommitWarnings,
    diff: Option<CommitDiff>,
    ahead: bool,
    commit_id: Option<(CommitId, usize)>,
    change_id: Option<(ChangeId, usize)>,
}

#[derive(Default)]
struct CommitWarnings {
    hidden: Option<bool>,
    conflict: Option<bool>,
    divergent: Option<bool>,
    immutable: Option<bool>,
    empty: Option<bool>,
}

#[derive(Default)]
struct CommitDiff {
    // files_added : usize,
    // files_removed : usize,
    files_changed: usize,
    lines_added: usize,
    lines_removed: usize,
}
impl CommitDiff {
    fn is_empty(&self) -> bool {
        self.files_changed == 0 && self.lines_added == 0 && self.lines_removed == 0
    }
}

fn print_prompt(
    command_helper: &CommandHelper,
    config_path: &Option<PathBuf>,
) -> Result<(), CommandError> {
    let _ = dotenvy::dotenv();
    let mut b = ::config::Config::builder();

    if let Some(config_path) = config_path {
        b = b.add_source(::config::File::new(
            config_path.to_str().ok_or(CommandError::new(
                jj_cli::command_error::CommandErrorKind::User,
                "Invalid Config Path",
            ))?,
            ::config::FileFormat::Toml,
        ));
    } else {
        let config_dir = get_config_path()?;
        if std::fs::exists(&config_dir)? {
            b = b.add_source(::config::File::new(&config_dir, ::config::FileFormat::Toml));
        } else {
            b = b.add_source(
                ::config::Config::try_from(&config::Config::default())
                    .expect("Config not serializable?"),
            );
        }
    };

    b = b.add_source(
        Environment::with_prefix("SJJ")
            .separator("__")
            .prefix_separator("__")
            .try_parsing(true),
    );

    let c = b.build().map_err(|err| {
        CommandError::with_message(
            jj_cli::command_error::CommandErrorKind::User,
            "Failed to parse Config",
            err,
        )
    })?;

    let config: config::Config = c.try_deserialize().map_err(|err| {
        CommandError::with_message(
            jj_cli::command_error::CommandErrorKind::User,
            "Failed to parse Config",
            err,
        )
    })?;

    let mut state = State::new(!command_helper.global_args().ignore_working_copy);
    let mut data = JJData::default();

    config.print(&command_helper, &mut state, &mut data)?;

    Ok(())
}

fn find_parent_bookmarks(
    workspace_helper: &WorkspaceCommandHelper,
    view: &View,
    config: &BookmarkConfig,
    bookmarks: &mut Vec<Bookmark>,
) -> Result<(), CommandError> {
    // First check if @ has bookmarks
    let wc_revs =
        workspace_helper.parse_revset(&Ui::null(), &RevisionArg::from("@".to_string()))?;
    let wc_ids: Vec<CommitId> = wc_revs
        .evaluate_to_commit_ids()?
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(wc_id) = wc_ids.first() {
        // Check for bookmarks on @
        let wc_bookmarks = collect_bookmarks_for_commit(wc_id, view, config, 0);
        if let Some(bookmark) = select_bookmark(wc_bookmarks) {
            bookmarks.push(bookmark);
            return Ok(());
        }
    }

    let mut selected_bookmark = None;

    // No bookmarks on @, use tug logic to find tracked target
    let revs = workspace_helper.parse_revset(
        &Ui::null(),
        &RevisionArg::from("latest((heads(::@- & bookmarks())))".to_string()),
    )?;

    let commit_ids: Vec<CommitId> = revs
        .evaluate_to_commit_ids()?
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(target_id) = commit_ids.first()
        && let Some(distance) = distance_to_working_copy(workspace_helper, target_id)?
        && distance <= config.search_depth
    {
        for bookmark in collect_bookmarks_for_commit(target_id, view, config, distance)
            .into_iter()
            .filter(|bookmark| bookmark.kind == BookmarkKind::Tracked)
        {
            choose_bookmark(&mut selected_bookmark, bookmark);
        }
    }

    if let Some(bookmark) = find_nearest_untracked_bookmark(workspace_helper, view, config)? {
        choose_bookmark(&mut selected_bookmark, bookmark);
    }

    if let Some(bookmark) = selected_bookmark {
        bookmarks.push(bookmark);
    }

    Ok(())
}

fn distance_to_working_copy(
    workspace_helper: &WorkspaceCommandHelper,
    target_id: &CommitId,
) -> Result<Option<usize>, CommandError> {
    let distance_revs = workspace_helper.parse_revset(
        &Ui::null(),
        &RevisionArg::from(format!("{}::@", target_id.hex())),
    )?;
    let count = distance_revs
        .evaluate_to_commit_ids()?
        .collect::<Result<Vec<_>, _>>()?
        .len();

    if count == 0 {
        return Ok(None);
    }

    // Subtract one because target::@ includes target itself.
    Ok(Some(count.saturating_sub(1)))
}

fn find_nearest_untracked_bookmark(
    workspace_helper: &WorkspaceCommandHelper,
    view: &View,
    config: &BookmarkConfig,
) -> Result<Option<Bookmark>, CommandError> {
    let mut selected_bookmark = None;

    for (symbol, remote_ref) in view.all_remote_bookmarks() {
        if remote_ref.is_tracked() {
            continue;
        }

        let name = format!("{}@{}", symbol.name.as_str(), symbol.remote.as_str());
        if bookmark_excluded(config, &name) {
            continue;
        }

        for commit_id in remote_ref.target.added_ids() {
            let Some(distance) = distance_to_working_copy(workspace_helper, commit_id)? else {
                continue;
            };
            if distance > config.search_depth {
                continue;
            }

            choose_bookmark(
                &mut selected_bookmark,
                Bookmark {
                    name: name.clone(),
                    distance,
                    kind: BookmarkKind::Untracked,
                },
            );
        }
    }

    Ok(selected_bookmark)
}

fn collect_bookmarks_for_commit(
    commit_id: &CommitId,
    view: &View,
    config: &BookmarkConfig,
    distance: usize,
) -> Vec<Bookmark> {
    let mut bookmarks = Vec::new();
    let mut local_names = HashSet::new();

    // Local bookmarks
    for (name, _) in view.local_bookmarks_for_commit(commit_id) {
        let name_str = name.as_str();
        if !bookmark_excluded(config, name_str) {
            bookmarks.push(Bookmark {
                name: name_str.to_string(),
                distance,
                kind: BookmarkKind::Tracked,
            });
            local_names.insert(name_str.to_string());
        }
    }

    // Remote bookmarks (if no local with same name)
    for (symbol, remote_ref) in view.all_remote_bookmarks() {
        if remote_ref.target.added_ids().any(|id| id == commit_id)
            && !local_names.contains(symbol.name.as_str())
        {
            let name = format!("{}@{}", symbol.name.as_str(), symbol.remote.as_str());
            if !bookmark_excluded(config, &name) {
                let kind = if remote_ref.is_tracked() {
                    BookmarkKind::Tracked
                } else {
                    BookmarkKind::Untracked
                };
                bookmarks.push(Bookmark {
                    name,
                    distance,
                    kind,
                });
            }
        }
    }

    bookmarks
}

#[cfg(not(feature = "json-schema"))]
fn bookmark_excluded(config: &BookmarkConfig, name: &str) -> bool {
    config.exclude.iter().any(|glob| glob.matches(name))
}

#[cfg(feature = "json-schema")]
fn bookmark_excluded(_config: &BookmarkConfig, _name: &str) -> bool {
    false
}

fn select_bookmark(bookmarks: impl IntoIterator<Item = Bookmark>) -> Option<Bookmark> {
    bookmarks.into_iter().min_by(compare_bookmarks)
}

fn choose_bookmark(selected: &mut Option<Bookmark>, bookmark: Bookmark) {
    match selected {
        Some(current) if compare_bookmarks(&bookmark, current) != Ordering::Less => {}
        _ => *selected = Some(bookmark),
    }
}

fn compare_bookmarks(left: &Bookmark, right: &Bookmark) -> Ordering {
    left.distance
        .cmp(&right.distance)
        .then_with(|| left.kind.cmp(&right.kind))
        .then_with(|| left.name.cmp(&right.name))
}

#[cfg(test)]
mod bookmark_selection_tests {
    use super::*;

    fn bookmark(name: &str, distance: usize, kind: BookmarkKind) -> Bookmark {
        Bookmark {
            name: name.to_string(),
            distance,
            kind,
        }
    }

    #[test]
    fn selects_nearest_bookmark() {
        let selected = select_bookmark([
            bookmark("main", 5, BookmarkKind::Tracked),
            bookmark("topic@origin", 2, BookmarkKind::Untracked),
        ]);

        assert_eq!(
            selected,
            Some(bookmark("topic@origin", 2, BookmarkKind::Untracked))
        );
    }

    #[test]
    fn selects_tracked_bookmark_when_distance_ties() {
        let selected = select_bookmark([
            bookmark("topic@origin", 3, BookmarkKind::Untracked),
            bookmark("main", 3, BookmarkKind::Tracked),
        ]);

        assert_eq!(selected, Some(bookmark("main", 3, BookmarkKind::Tracked)));
    }

    #[test]
    fn selects_lexicographic_bookmark_when_kind_and_distance_tie() {
        let selected = select_bookmark([
            bookmark("zeta@origin", 3, BookmarkKind::Untracked),
            bookmark("alpha@origin", 3, BookmarkKind::Untracked),
        ]);

        assert_eq!(
            selected,
            Some(bookmark("alpha@origin", 3, BookmarkKind::Untracked))
        );
    }
}

fn main() -> ExitCode {
    let start = std::time::Instant::now();
    let print_timing = std::env::var("STARSHIP_JJ_TIMING").is_ok();
    let clirunner = CliRunner::init();
    let clirunner = clirunner.name("starship-jj");
    let clirunner = clirunner.version(&format!(
        "{} {}",
        crate::built_info::PKG_VERSION,
        crate::built_info::GIT_COMMIT_HASH_SHORT.unwrap_or_default()
    ));
    let clirunner = clirunner.add_subcommand(starship);
    let e = clirunner.run();
    let elapsed = start.elapsed();
    if print_timing {
        print!("{elapsed:?} ");
    }
    e.into()
}

fn print_ansi_truncated(
    max_length: Option<usize>,
    io: &mut impl Write,
    name: &str,
    surround_with_quotes: bool,
) -> Result<(), CommandError> {
    let maybe_quotes = if surround_with_quotes { "\"" } else { "" };

    match max_length {
        Some(max_len) if name.width() > max_len => {
            let ansi_max_len = name
                .char_indices()
                .map(|(i, _)| i)
                .take_while(|i| name[..*i].width() < max_len)
                .last()
                .unwrap_or_default();

            write!(
                io,
                "{}{}…{}",
                maybe_quotes,
                &name[..ansi_max_len],
                maybe_quotes
            )?;
        }
        _ => {
            write!(io, "{maybe_quotes}{name}{maybe_quotes}")?;
        }
    }
    Ok(())
}
