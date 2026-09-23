use crate::backend::{self, BackendCommand, Edge};
use crate::command::{CommandSpec, which};
use crate::common::{own_binary, parse_path, path_text};
use crate::server::ServeArgs;
use crate::{AppError, AppResult};
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use fileblade_output::{Format, Output};
use serde_json::{Map, Value, json};
use std::collections::HashSet;
use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

mod args;
mod blade;
mod branches;
mod doctor;
mod extension;
mod files;
mod host_status;
mod integration;
mod ipc;
mod launch;
mod plugins;
mod queries;
mod space;
mod usage;
pub use args::*;
pub use blade::*;
pub use branches::*;
use doctor::*;
pub use extension::*;
use files::*;
pub use host_status::*;
use ipc::*;
use launch::*;
pub use plugins::*;
use queries::*;
pub use usage::*;
const READ_TARGET: &str = "data-goblin.fileblade";
const CONTROL_TARGET: &str = "data-goblin.fileblade.control";
const IPC_TIMEOUT: Duration = Duration::from_secs(5);
const BACKEND_TIMEOUT: Duration = Duration::from_secs(15);
const MUTATION_BACKEND_TIMEOUT: Duration = Duration::from_secs(300);
const MAX_IPC_STDOUT: usize = 8 * 1024 * 1024;
const MAX_IPC_STDERR: usize = 256 * 1024;
const MAX_WAIT_SECONDS: f64 = 300.0;
const MAX_POLL_SECONDS: f64 = 905.0;

#[derive(Clone, Debug, Parser)]
#[command(
    name = "fileblade",
    version,
    about = "Inspect and control the live Omarchy FileBlade filesystem selection."
)]
pub struct Cli {
    #[arg(short = 'o', long = "output", value_enum, default_value_t = OutputFormat::Text, global = true)]
    pub output: OutputFormat,
    #[arg(long, global = true)]
    pub quiet: bool,
    #[command(subcommand)]
    pub command: RootCommand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum OutputFormat {
    Text,
    Json,
}

impl From<OutputFormat> for Format {
    fn from(value: OutputFormat) -> Self {
        match value {
            OutputFormat::Text => Self::Text,
            OutputFormat::Json => Self::Json,
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
pub enum RootCommand {
    Native(crate::native::Args),
    #[command(name = "_companion-mutate", hide = true)]
    CompanionMutate,
    #[command(name = "_backend", hide = true)]
    Backend {
        #[command(subcommand)]
        command: BackendCommand,
    },
    #[command(hide = true)]
    Serve(ServeArgs),
    #[command(name = "exec-hex", hide = true)]
    ExecHex(ExecHexArgs),
    Preferences(crate::preferences::Changes),
    Status,
    Control {
        method: String,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        arguments: Vec<String>,
    },
    Popout {
        module: Option<String>,
    },
    Doctor,
    Selection,
    Favorites,
    Show,
    Hide,
    Toggle,
    Refresh,
    Up,
    Home,
    /// Open the directory the default screenshot tool writes to.
    Screenshots,
    Quicknav(QuicknavArgs),
    Pick(PickArgs),
    Shell(ShellArgs),
    Log(LogArgs),
    Recent(RecentArgs),
    List(ListArgs),
    Extract(ExtractArgs),
    ArchiveList(PathValue),
    Preview(PathValue),
    FocusLocation,
    ClearLocationError,
    CloseMenu,
    ClearClipboard,
    Back(WaitTenArgs),
    Forward(WaitTenArgs),
    Hidden(HiddenArgs),
    Select(RequiredPaths),
    Menu(MenuArgs),
    Copy(OptionalPaths),
    Cut(OptionalPaths),
    CopyPath(OptionalPaths),
    Paste(PasteArgs),
    CopyTo(TransferArgs),
    MoveTo(TransferArgs),
    Rename(RenameArgs),
    NewFile(CreateArgs),
    NewFolder(CreateArgs),
    Trash(TrashArgs),
    TrashList(TrashListArgs),
    TrashRestore(TrashRestoreArgs),
    TrashDelete(TrashDeleteArgs),
    TrashEmpty(TrashEmptyArgs),
    Undo(HistoryStepArgs),
    Redo(HistoryStepArgs),
    History(HistoryArgs),
    Operation(OperationArgs),
    CancelOperation(CancelOperationArgs),
    GitRefresh(WaitTenArgs),
    Tree(LimitArgs),
    Results(LimitArgs),
    Applications(PathValue),
    Open(OpenArgs),
    Edit(PathWaitArgs),
    Reveal(PathWaitArgs),
    OpenWith(OpenWithArgs),
    Wheel(PointPathsArgs),
    CloseWheel,
    PastePath(PastePathArgs),
    DropActions(PointPathsArgs),
    DropRun(DropRunArgs),
    Color(ColorArgs),
    ClearColor(OptionalPaths),
    FolderColor(PathValue),
    Pin(RequiredPaths),
    Unpin(RequiredPaths),
    Root(RootArgs),
    Navigate(NavigateArgs),
    Expand(PathValue),
    Collapse(PathValue),
    Width(PixelsArgs),
    BladeWidth(PixelsArgs),
    Blades,
    Modules,
    /// Print how full the drive holding a folder is; defaults to the open FileBlade root.
    Space(space::SpaceArgs),
    RescanModules,
    ModuleDirs(ModuleDirsArgs),
    Actions,
    Action(ActionArgs),
    SearchDeep(OnOffArgs),
    GitDetails(GitDetailsArgs),
    /// Choose repository summary fields; omit all fields to hide the summary.
    GitSummary(GitSummaryArgs),
    CycleMetric,
    SetFolderColor(PathColorArgs),
    ClearFolderColor(PathValue),
    ColorScope(ColorScopeArgs),
    ToggleFavorite(PathValue),
    Properties {
        #[command(subcommand)]
        action: PropertiesCommand,
    },
    Settings,
    TogglePath(PathValue),
    Blade {
        #[command(subcommand)]
        action: BladeCommand,
    },
    /// Open the Branches module, close it, or list branches and worktrees.
    Branches {
        #[command(subcommand)]
        action: Option<BranchesCommand>,
    },
    /// Report whether FileBlade is installed, enabled and answering, for a companion extension.
    HostStatus(HostStatusArgs),
    /// Scaffold a FileBlade extension.
    Extension {
        #[command(subcommand)]
        action: ExtensionCommand,
    },
    /// Install FileBlade into another tool.
    Install {
        #[command(subcommand)]
        action: integration::InstallCommand,
    },
    /// Print the current selection as agent context. Always succeeds; prints an
    /// empty context when FileBlade is not running.
    AgentContext(integration::AgentContextArgs),
    /// Read or forget the recorded daily skill and MCP use history.
    Usage {
        #[command(subcommand)]
        action: UsageCommand,
    },
    Focus(FocusArgs),
    Search(SearchArgs),
    ClearSearch,
    Column(ColumnArgs),
    Columns(ColumnsArgs),
}

pub fn parse<I, T>(arguments: I) -> Result<Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    Cli::try_parse_from(arguments)
}

pub fn run(command: RootCommand) -> AppResult<PublicResult> {
    run_command(command)
}

fn run_command(command: RootCommand) -> AppResult<PublicResult> {
    match command {
        RootCommand::Native(_)
        | RootCommand::Backend { .. }
        | RootCommand::Serve(_)
        | RootCommand::ExecHex(_)
        | RootCommand::CompanionMutate => Err(AppError::invalid(
            "internal command routed through the public CLI",
        )),
        RootCommand::Preferences(changes) => {
            let settings =
                if changes.trash_retention_days.is_some() || changes.agent_management.is_some() {
                    crate::preferences::change(&changes)?
                } else {
                    crate::preferences::read()?
                };
            Ok(PublicResult::one(json!({"ok":true,"settings":settings})))
        }
        RootCommand::Status => json_ipc("status", &[]),
        RootCommand::Doctor => doctor(),
        RootCommand::HostStatus(options) => host_status(options),
        RootCommand::Selection => json_ipc("selection", &[]),
        RootCommand::Favorites => json_ipc("favorites", &[]),
        RootCommand::Show => simple_ipc("open", &[]),
        RootCommand::Hide => simple_ipc("close", &[]),
        RootCommand::Toggle => simple_ipc("toggle", &[]),
        RootCommand::Control { method, arguments } => simple_ipc(&method, &arguments),
        RootCommand::Popout { module } => {
            let method = if module.is_some() { "open" } else { "close" };
            let arguments: Vec<_> = module.into_iter().collect();
            let response = ipc_on("data-goblin.fileblade.popout", method, &arguments)?;
            if matches!(response.as_str(), "opened" | "closed") {
                Ok(PublicResult::one(response))
            } else {
                Err(AppError::command(response))
            }
        }
        RootCommand::Refresh => simple_ipc("refresh", &[]),
        RootCommand::Up => simple_ipc("up", &[]),
        RootCommand::Home => simple_ipc("home", &[]),
        RootCommand::Screenshots => simple_ipc("screenshots", &[]),
        RootCommand::Quicknav(options) => simple_ipc("quickNavChannel", &[options.channel]),
        RootCommand::Pick(options) => pick(options),
        RootCommand::Log(options) => log(options),
        RootCommand::Recent(options) => recent(options),
        RootCommand::List(options) => list(options),
        RootCommand::Extract(options) => {
            let mut arguments = vec![
                "archive-extract".to_string(),
                "--path".to_string(),
                absolute_path(&options.archive),
            ];
            if let Some(to) = options.to {
                arguments.extend(["--destination".to_string(), absolute_path(&to)]);
            }
            if options.merge {
                arguments.push("--merge".to_string());
            }
            Ok(PublicResult::checked(
                backend_json_with_timeout(&arguments, MUTATION_BACKEND_TIMEOUT)?,
                "extraction failed",
            ))
        }
        RootCommand::Preview(options) => Ok(PublicResult::checked(
            backend_json(&[
                "preview".to_string(),
                "--path".to_string(),
                absolute_path(&options.path),
            ])?,
            "preview failed",
        )),
        RootCommand::ArchiveList(options) => Ok(PublicResult::checked(
            backend_json(&[
                "archive-list".to_string(),
                "--path".to_string(),
                absolute_path(&options.path),
            ])?,
            "listing failed",
        )),
        RootCommand::Shell(options) => Ok(PublicResult::one(crate::shell_init::script(
            options.shell == ShellKind::Zsh,
        ))),
        RootCommand::FocusLocation => simple_ipc("focusLocation", &[]),
        RootCommand::ClearLocationError => simple_ipc("clearLocationError", &[]),
        RootCommand::CloseMenu => simple_ipc("hideActions", &[]),
        RootCommand::ClearClipboard => simple_ipc("clearClipboard", &[]),
        RootCommand::Back(options) => history_navigation("back", options),
        RootCommand::Forward(options) => history_navigation("forward", options),
        RootCommand::Hidden(options) => hidden(options),
        RootCommand::Select(options) => select(options.paths),
        RootCommand::Menu(options) => menu(options),
        RootCommand::Copy(options) => selection_clipboard(options.paths, false),
        RootCommand::Cut(options) => selection_clipboard(options.paths, true),
        RootCommand::CopyPath(options) => selection_paths_clipboard(options.paths),
        RootCommand::Paste(options) => paste(options),
        RootCommand::CopyTo(options) => transfer(options, true),
        RootCommand::MoveTo(options) => transfer(options, false),
        RootCommand::Rename(options) => rename(options),
        RootCommand::NewFile(options) => create(options, false),
        RootCommand::NewFolder(options) => create(options, true),
        RootCommand::Trash(options) => trash(options),
        RootCommand::TrashList(options) => trash_list(options),
        RootCommand::TrashRestore(options) => trash_restore(options),
        RootCommand::TrashDelete(options) => trash_delete(options),
        RootCommand::TrashEmpty(options) => trash_empty(options),
        RootCommand::Undo(options) => history_step("undo", options),
        RootCommand::Redo(options) => history_step("redo", options),
        RootCommand::History(options) => history(options),
        RootCommand::Operation(options) => json_ipc("operationResult", &[options.request_id]),
        RootCommand::CancelOperation(options) => cancel_operation(options),
        RootCommand::GitRefresh(options) => git_refresh(options),
        RootCommand::Tree(options) => json_ipc("tree", &[options.limit.to_string()]),
        RootCommand::Results(options) => json_ipc("searchResults", &[options.limit.to_string()]),
        RootCommand::Applications(options) => applications(options.path),
        RootCommand::Open(options) => open(options),
        RootCommand::Edit(options) => launch_method("editPath", options.path, options.wait),
        RootCommand::Reveal(options) => launch_method("revealPath", options.path, options.wait),
        RootCommand::OpenWith(options) => open_with(options),
        RootCommand::Wheel(options) => wheel(options),
        RootCommand::CloseWheel => simple_ipc("hideDropWheel", &[]),
        RootCommand::PastePath(options) => paste_path(options),
        RootCommand::DropActions(options) => drop_actions(options),
        RootCommand::DropRun(options) => drop_run(options),
        RootCommand::Color(options) => color(options.color, options.paths),
        RootCommand::ClearColor(options) => color("default".to_string(), options.paths),
        RootCommand::FolderColor(options) => json_ipc("folderColor", &[options.path]),
        RootCommand::Pin(options) => favorites_batch(options.paths, true),
        RootCommand::Unpin(options) => favorites_batch(options.paths, false),
        RootCommand::Root(options) => root(options),
        RootCommand::Navigate(options) => navigation("navigate", options),
        RootCommand::Expand(options) => expansion("expandPath", options.path),
        RootCommand::Collapse(options) => expansion("collapsePath", options.path),
        RootCommand::Width(options) => simple_ipc("setSidebarWidth", &[options.pixels.to_string()]),
        RootCommand::BladeWidth(options) => {
            simple_ipc("setPropertiesBladeWidth", &[options.pixels.to_string()])
        }
        RootCommand::Blades => json_ipc("blades", &[]),
        RootCommand::Space(options) => space::space(options),
        RootCommand::Modules => modules(),
        RootCommand::RescanModules => simple_ipc("rescanBladeModules", &[]),
        RootCommand::ModuleDirs(options) => module_dirs(options),
        RootCommand::Actions => actions(),
        RootCommand::Action(options) => action(options),
        RootCommand::Blade { action } => blade(action),
        RootCommand::Branches { action } => branches(action),
        RootCommand::Extension { action } => extension(action),
        RootCommand::Install { action } => match action {
            integration::InstallCommand::Integration(options) => integration::integration(&options),
        },
        RootCommand::AgentContext(options) => integration::agent_context(&options),
        RootCommand::Usage { action } => usage(action),
        RootCommand::Focus(options) => simple_ipc("focusDirection", &[options.direction]),
        RootCommand::Search(options) => search(options),
        RootCommand::ClearSearch => simple_ipc("clearSearch", &[]),
        RootCommand::Column(options) => simple_ipc(
            "setPriorityProperty",
            &[options.property.as_str().to_string()],
        ),
        RootCommand::Columns(options) => simple_ipc("setPriorityColumns", &[options.properties]),
        RootCommand::SearchDeep(options) => simple_ipc(
            "setSearchDeep",
            &[matches!(options.state, OnOff::On).to_string()],
        ),
        RootCommand::GitDetails(options) => {
            simple_ipc("setGitStatusDetails", &[options.details.join(",")])
        }
        RootCommand::GitSummary(options) => {
            simple_ipc("setGitSummaryFields", &[options.fields.join(",")])
        }
        RootCommand::CycleMetric => simple_ipc("cyclePriorityProperty", &[]),
        RootCommand::SetFolderColor(options) => simple_ipc(
            "setFolderColor",
            &[absolute_path(&options.path), options.color],
        ),
        RootCommand::ClearFolderColor(options) => {
            simple_ipc("clearFolderColor", &[absolute_path(&options.path)])
        }
        RootCommand::ColorScope(options) => simple_ipc(
            "setFolderColorScope",
            &[format!("{:?}", options.scope).to_lowercase()],
        ),
        RootCommand::ToggleFavorite(options) => {
            simple_ipc("toggleFavorite", &[absolute_path(&options.path)])
        }
        RootCommand::Properties { action } => match action {
            PropertiesCommand::Placement(options) => simple_ipc(
                "setPlacement",
                &[format!("{:?}", options.placement).to_lowercase()],
            ),
            PropertiesCommand::Focus => simple_ipc("focusProperties", &[]),
        },
        RootCommand::Settings => simple_ipc("toggleSettings", &[]),
        RootCommand::TogglePath(options) => {
            simple_ipc("togglePath", &[absolute_path(&options.path)])
        }
    }
}
