use std::{
    collections::{BTreeMap, HashSet},
    io::Write,
    path::PathBuf,
    process::ExitCode,
};

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
    bookmarks: Option<BTreeMap<String, usize>>,
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
    bookmarks: &mut BTreeMap<String, usize>,
) -> Result<(), CommandError> {
    // First check if @ has bookmarks
    let wc_revs = workspace_helper
        .parse_revset(&Ui::null(), &RevisionArg::from("@".to_string()))?;
    let wc_ids: Vec<CommitId> = wc_revs
        .evaluate_to_commit_ids()?
        .collect::<Result<Vec<_>, _>>()?;

    if let Some(wc_id) = wc_ids.first() {
        // Check for bookmarks on @
        let has_bookmarks = collect_bookmarks_for_commit(wc_id, view, config, bookmarks, 0);
        if has_bookmarks {
            return Ok(());
        }
    }

    // No bookmarks on @, use tug logic to find target
    let revs = workspace_helper.parse_revset(
        &Ui::null(),
        &RevisionArg::from("latest((heads(::@- & bookmarks())))".to_string()),
    )?;

    let commit_ids: Vec<CommitId> = revs
        .evaluate_to_commit_ids()?
        .collect::<Result<Vec<_>, _>>()?;

    if commit_ids.is_empty() {
        return Ok(());
    }

    let target_id = &commit_ids[0];

    // Calculate distance from target to @ using revset: count(target::@) - 1
    // (subtract 1 because target::@ includes target itself)
    let distance_revs = workspace_helper.parse_revset(
        &Ui::null(),
        &RevisionArg::from(format!("{}::@", target_id.hex())),
    )?;
    let distance = distance_revs
        .evaluate_to_commit_ids()?
        .collect::<Result<Vec<_>, _>>()?
        .len()
        .saturating_sub(1);

    collect_bookmarks_for_commit(target_id, view, config, bookmarks, distance);

    Ok(())
}

/// Helper to collect bookmarks for a commit. Returns true if any bookmarks were found.
fn collect_bookmarks_for_commit(
    commit_id: &CommitId,
    view: &View,
    config: &BookmarkConfig,
    bookmarks: &mut BTreeMap<String, usize>,
    distance: usize,
) -> bool {
    let mut found = false;

    // Local bookmarks
    for (name, _) in view.local_bookmarks_for_commit(commit_id) {
        let name_str = name.as_str();
        #[cfg(not(feature = "json-schema"))]
        let excluded = config.exclude.iter().any(|glob| glob.matches(name_str));
        #[cfg(feature = "json-schema")]
        let excluded = false;
        if !excluded {
            bookmarks.insert(name_str.to_string(), distance);
            found = true;
        }
    }

    // Remote bookmarks (if no local with same name)
    let local_names: HashSet<_> = bookmarks.keys().cloned().collect();
    for (symbol, remote_ref) in view.all_remote_bookmarks() {
        if remote_ref.target.added_ids().any(|id| id == commit_id) {
            if !local_names.contains(symbol.name.as_str()) {
                let name = format!("{}@{}", symbol.name.as_str(), symbol.remote.as_str());
                #[cfg(not(feature = "json-schema"))]
                let excluded = config.exclude.iter().any(|glob| glob.matches(&name));
                #[cfg(feature = "json-schema")]
                let excluded = false;
                if !excluded {
                    bookmarks.insert(name, distance);
                    found = true;
                }
            }
        }
    }

    found
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
