use super::*;

#[derive(Clone, Debug, Args)]
pub struct WaitTenArgs {
    #[arg(long)]
    pub wait: bool,
    #[arg(long, default_value_t = 10.0, allow_hyphen_values = true)]
    pub timeout: f64,
}

#[derive(Clone, Debug, Args)]
pub struct WaitThirtyArgs {
    #[arg(long)]
    pub wait: bool,
    #[arg(long, default_value_t = 30.0, allow_hyphen_values = true)]
    pub timeout: f64,
}

#[derive(Clone, Debug, Args)]
pub struct HiddenArgs {
    #[arg(value_enum)]
    pub state: HiddenState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum HiddenState {
    Show,
    Hide,
    Toggle,
}

#[derive(Clone, Debug, Args)]
pub struct RequiredPaths {
    #[arg(required = true)]
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct OptionalPaths {
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct MenuArgs {
    pub paths: Vec<String>,
    #[arg(long)]
    pub open_with: bool,
}

#[derive(Clone, Debug, Args)]
pub struct PasteArgs {
    pub destination: Option<String>,
    #[command(flatten)]
    pub wait: WaitThirtyArgs,
}

#[derive(Clone, Debug, Args)]
pub struct TransferArgs {
    pub destination: String,
    pub paths: Vec<String>,
    #[command(flatten)]
    pub wait: WaitThirtyArgs,
}

#[derive(Clone, Debug, Args)]
pub struct RenameArgs {
    pub name: String,
    pub path: Option<String>,
    #[command(flatten)]
    pub wait: WaitThirtyArgs,
}

#[derive(Clone, Debug, Args)]
pub struct CreateArgs {
    pub name: String,
    pub parent: Option<String>,
    #[command(flatten)]
    pub wait: WaitThirtyArgs,
}

#[derive(Clone, Debug, Args)]
pub struct TrashArgs {
    pub paths: Vec<String>,
    #[arg(long, required = true, action = ArgAction::SetTrue)]
    pub yes: bool,
    #[command(flatten)]
    pub wait: WaitThirtyArgs,
}

#[derive(Clone, Debug, Args)]
pub struct TrashListArgs {
    #[arg(long, default_value_t = 500, allow_hyphen_values = true)]
    pub limit: i64,
}

#[derive(Clone, Debug, Args)]
pub struct TrashRestoreArgs {
    #[arg(long)]
    pub id: String,
    #[arg(long, default_value = "")]
    pub destination: String,
    #[arg(long)]
    pub recreate_parent: bool,
}

#[derive(Clone, Debug, Args)]
pub struct TrashDeleteArgs {
    #[arg(long, action = ArgAction::Append, required = true)]
    pub id: Vec<String>,
    #[arg(long, required = true, action = ArgAction::SetTrue)]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
pub struct TrashEmptyArgs {
    #[arg(long, required = true, action = ArgAction::SetTrue)]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
pub struct HistoryStepArgs {
    #[arg(long)]
    pub drop: bool,
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub wait: WaitThirtyArgs,
}

#[derive(Clone, Debug, Args)]
pub struct HistoryArgs {
    #[arg(long)]
    pub refresh: bool,
}

#[derive(Clone, Debug, Args)]
pub struct OperationArgs {
    pub request_id: String,
}

#[derive(Clone, Debug, Args)]
pub struct CancelOperationArgs {
    pub request_id: Option<String>,
    #[arg(long, required = true, action = ArgAction::SetTrue)]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
pub struct LimitArgs {
    #[arg(long, default_value_t = 500, allow_hyphen_values = true)]
    pub limit: i64,
}

#[derive(Clone, Debug, Args)]
pub struct PathValue {
    pub path: String,
}

#[derive(Clone, Debug, Args)]
pub struct OpenArgs {
    pub path: Option<String>,
    #[command(flatten)]
    pub wait: WaitTenArgs,
}

#[derive(Clone, Debug, Args)]
pub struct PathWaitArgs {
    pub path: String,
    #[command(flatten)]
    pub wait: WaitTenArgs,
}

#[derive(Clone, Debug, Args)]
pub struct OpenWithArgs {
    pub path: String,
    pub desktop_id: String,
    #[command(flatten)]
    pub wait: WaitTenArgs,
}

#[derive(Clone, Debug, Args)]
pub struct PointPathsArgs {
    pub paths: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    pub at: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct PastePathArgs {
    pub paths: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    pub at: Option<String>,
    #[arg(long)]
    pub relative: bool,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Args)]
pub struct DropRunArgs {
    pub action: String,
    pub paths: Vec<String>,
    #[arg(long, allow_hyphen_values = true)]
    pub at: Option<String>,
    #[arg(long, default_value = "")]
    pub placement: String,
    #[arg(long, default_value = "")]
    pub desktop_id: String,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Args)]
pub struct ColorArgs {
    pub color: String,
    pub paths: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct NavigateArgs {
    pub path: String,
    #[command(flatten)]
    pub wait: WaitTenArgs,
}

#[derive(Clone, Debug, Args)]
pub struct RootArgs {
    pub path: Option<String>,
    #[command(flatten)]
    pub wait: WaitTenArgs,
}

#[derive(Clone, Debug, Args)]
pub struct PickArgs {
    #[arg(long, value_enum, default_value_t = PickMode::Open)]
    pub mode: PickMode,
    #[arg(long)]
    pub multiple: bool,
    #[arg(long)]
    pub root: Option<String>,
    #[arg(long)]
    pub query: Option<String>,
    #[arg(long = "extension", action = ArgAction::Append)]
    pub extensions: Vec<String>,
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub suggested_name: Option<String>,
    #[arg(long)]
    pub shell_quote: bool,
    #[arg(long, default_value_t = MAX_WAIT_SECONDS, allow_hyphen_values = true)]
    pub timeout: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PickMode {
    Open,
    Save,
    Folder,
}

#[derive(Clone, Debug, Args)]
pub struct LogArgs {
    #[arg(long, default_value_t = 50, allow_hyphen_values = true)]
    pub limit: i64,
    #[arg(long, default_value = "")]
    pub since: String,
    #[arg(long, default_value = "")]
    pub command: String,
}

#[derive(Clone, Debug, Args)]
pub struct RecentArgs {
    #[arg(long)]
    pub show_hidden: bool,
    #[arg(long, default_value_t = 50, allow_hyphen_values = true)]
    pub limit: i64,
    #[arg(long, default_value = "")]
    pub query: String,
}

#[derive(Clone, Debug, Args)]
pub struct ListArgs {
    #[arg(long, default_value = "list")]
    pub title: String,
    #[arg(long, default_value = "-")]
    pub from: String,
}

#[derive(Clone, Debug, Args)]
pub struct ExtractArgs {
    pub archive: String,
    #[arg(long)]
    pub to: Option<String>,
    #[arg(long)]
    pub merge: bool,
}

#[derive(Clone, Debug, Args)]
pub struct QuicknavArgs {
    #[arg(long, default_value = "folders")]
    pub channel: String,
}

#[derive(Clone, Debug, Args)]
pub struct ShellArgs {
    #[arg(value_enum)]
    pub shell: ShellKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum ShellKind {
    Bash,
    Zsh,
}

#[derive(Clone, Debug, Args)]
pub struct PixelsArgs {
    #[arg(allow_hyphen_values = true)]
    pub pixels: i64,
}

#[derive(Clone, Debug, Args)]
pub struct FocusArgs {
    #[arg(value_parser = ["l", "r", "u", "d", "left", "right", "up", "down"])]
    pub direction: String,
}

#[derive(Clone, Debug, Args)]
pub struct SearchArgs {
    pub query: String,
    #[arg(long)]
    pub wait: bool,
    #[arg(long)]
    pub case_sensitive: bool,
    #[arg(long)]
    pub regex: bool,
    #[arg(long)]
    pub tree: bool,
    #[arg(long)]
    pub shallow: bool,
    #[arg(long, default_value_t = 500, allow_hyphen_values = true)]
    pub limit: i64,
    #[arg(long, default_value_t = 10.0, allow_hyphen_values = true)]
    pub timeout: f64,
}

#[derive(Clone, Debug, Args)]
pub struct ColumnArgs {
    #[arg(value_enum)]
    pub property: Column,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Column {
    Off,
    None,
    Size,
    Type,
    Modified,
    Created,
    Repo,
    Branch,
    Worktree,
}

impl Column {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::None => "none",
            Self::Size => "size",
            Self::Type => "type",
            Self::Modified => "modified",
            Self::Created => "created",
            Self::Repo => "repo",
            Self::Branch => "branch",
            Self::Worktree => "worktree",
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct ColumnsArgs {
    pub properties: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum OnOff {
    On,
    Off,
}

#[derive(Clone, Debug, Args)]
pub struct OnOffArgs {
    #[arg(value_enum)]
    pub state: OnOff,
}

#[derive(Clone, Debug, Args)]
pub struct GitDetailsArgs {
    #[arg(required = true)]
    pub details: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct GitSummaryArgs {
    #[arg(value_delimiter = ',', value_parser = ["branch", "worktree", "ahead", "behind", "modified", "added", "untracked", "deleted", "renamed", "copied", "type_changed", "conflicted", "clean"])]
    pub fields: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct PathColorArgs {
    pub path: String,
    pub color: String,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ColorScope {
    Icon,
    Name,
    Row,
}

#[derive(Clone, Debug, Args)]
pub struct ColorScopeArgs {
    #[arg(value_enum)]
    pub scope: ColorScope,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Placement {
    Above,
    Right,
    Below,
}

#[derive(Clone, Debug, Args)]
pub struct PlacementArgs {
    #[arg(value_enum)]
    pub placement: Placement,
}

#[derive(Clone, Debug, Subcommand)]
pub enum PropertiesCommand {
    Placement(PlacementArgs),
    Focus,
}
