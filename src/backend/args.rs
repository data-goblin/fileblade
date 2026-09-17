use super::*;

#[derive(Clone, Debug, Args)]
pub struct PathArg {
    #[arg(long)]
    pub path: String,
}

#[derive(Clone, Debug, Args)]
pub struct SourceArgs {
    #[arg(long, required = true)]
    pub source: String,
}

#[derive(Clone, Debug, Args)]
pub struct PathsArgs {
    #[arg(long, action = ArgAction::Append, required = true)]
    pub path: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct ChildrenArgs {
    #[arg(long, action = ArgAction::Append, required = true)]
    pub path: Vec<String>,
    #[arg(long)]
    pub show_hidden: bool,
    #[arg(long, default_value_t = crate::listing::DIRECTORY_PAGE)]
    pub limit: usize,
    #[arg(long, default_value = "name")]
    pub sort: String,
    #[arg(long)]
    pub desc: bool,
    #[arg(long, default_value = "{}")]
    pub filter: String,
    #[arg(long)]
    pub include_created: bool,
    #[arg(long)]
    pub no_git: bool,
    #[arg(long)]
    pub fresh_git: bool,
}

#[derive(Clone, Debug, Args)]
pub struct ChildrenWindowArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub show_hidden: bool,
    #[arg(long, default_value_t = 0)]
    pub start: usize,
    #[arg(long, default_value_t = crate::listing::DIRECTORY_PAGE)]
    pub count: usize,
    #[arg(long, default_value = "name")]
    pub sort: String,
    #[arg(long)]
    pub desc: bool,
    #[arg(long, default_value = "{}")]
    pub filter: String,
    #[arg(long)]
    pub include_created: bool,
    #[arg(long)]
    pub fresh: bool,
    #[arg(long)]
    pub no_git: bool,
    #[arg(long)]
    pub fresh_git: bool,
}

pub(super) fn filter_document(text: &str) -> serde_json::Value {
    serde_json::from_str(text)
        .ok()
        .filter(serde_json::Value::is_object)
        .unwrap_or_else(|| serde_json::json!({}))
}

#[derive(Clone, Debug, Args)]
pub struct ChildArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub show_hidden: bool,
}

#[derive(Clone, Debug, Args)]
pub struct ProjectRootArgs {
    #[arg(long, default_value = "")]
    pub path: String,
}

#[derive(Clone, Debug, Args)]
pub struct SearchArgs {
    #[arg(long)]
    pub root: String,
    #[arg(long)]
    pub query: String,
    #[arg(long)]
    pub show_hidden: bool,
    #[arg(long, default_value_t = 200, allow_hyphen_values = true)]
    pub limit: i64,
    #[arg(long, action = ArgAction::Append)]
    pub repository: Vec<String>,
    #[arg(long)]
    pub case_sensitive: bool,
    #[arg(long)]
    pub regex: bool,
    #[arg(long)]
    pub fresh: bool,
    #[arg(long)]
    pub list: Option<String>,
    #[arg(long)]
    pub tree: bool,
    #[arg(long)]
    pub no_git: bool,
}

#[derive(Clone, Debug, Args)]
pub struct ArchiveExtractArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub destination: Option<String>,
    #[arg(long)]
    pub merge: bool,
}

#[derive(Clone, Debug, Args)]
pub struct PreviewArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value_t = 120, allow_hyphen_values = true)]
    pub lines: i64,
}

#[derive(Clone, Debug, Args)]
pub struct ThumbnailArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value = "")]
    pub key: String,
    #[arg(long, default_value_t = crate::thumbnail::MAX_OUTPUT_EDGE)]
    pub width: u32,
    #[arg(long, default_value_t = crate::thumbnail::MAX_OUTPUT_EDGE)]
    pub height: u32,
}

#[derive(Clone, Debug, Args)]
pub struct ThumbnailRenderArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub target: String,
    #[arg(long, default_value_t = crate::thumbnail::MAX_OUTPUT_EDGE)]
    pub width: u32,
    #[arg(long, default_value_t = crate::thumbnail::MAX_OUTPUT_EDGE)]
    pub height: u32,
}

#[derive(Clone, Debug, Args)]
pub struct FrecencyListArgs {
    #[arg(long)]
    pub show_hidden: bool,
    #[arg(long, default_value_t = 50, allow_hyphen_values = true)]
    pub limit: i64,
    #[arg(long, default_value = "")]
    pub query: String,
}

#[derive(Clone, Debug, Args)]
pub struct BinPutArgs {
    #[arg(long)]
    pub module: String,
    #[arg(long)]
    pub item: String,
}

#[derive(Clone, Debug, Args)]
pub struct BinModuleArgs {
    #[arg(long)]
    pub module: String,
}

#[derive(Clone, Debug, Args)]
pub struct BinRemoveArgs {
    #[command(flatten)]
    pub item: BinPutArgs,
    #[arg(long)]
    pub helper_route: String,
    #[arg(long, default_value = "[]")]
    pub arguments: String,
}

#[derive(Clone, Debug, Args)]
pub struct BinRestoreArgs {
    #[command(flatten)]
    pub entry: BinEntryArgs,
    #[arg(long)]
    pub helper_route: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct BinEntryArgs {
    #[arg(long)]
    pub module: String,
    #[arg(long)]
    pub id: String,
}

#[derive(Clone, Debug, Args)]
pub struct ClipboardArgs {
    #[arg(long, default_value_t = 512, allow_hyphen_values = true)]
    pub limit: i64,
}

#[derive(Clone, Debug, Args)]
pub struct ClipboardWriteArgs {
    #[arg(long, action = ArgAction::Append, required = true)]
    pub path: Vec<String>,
    #[arg(long)]
    pub cut: bool,
}

#[derive(Clone, Debug, Args)]
pub struct DocumentArgs {
    #[arg(long)]
    pub document: String,
}

#[derive(Clone, Debug, Args)]
pub struct ReadTextArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value_t = 256 * 1024, allow_hyphen_values = true)]
    pub limit: i64,
}

#[derive(Clone, Debug, Args)]
pub struct QuicknavArgs {
    #[arg(long, default_value = "~")]
    pub root: String,
    #[arg(long, default_value = "")]
    pub query: String,
    #[arg(long, default_value = "")]
    pub exclude: String,
    #[arg(long, default_value_t = 50, allow_hyphen_values = true)]
    pub limit: i64,
    #[arg(long)]
    pub show_hidden: bool,
}

#[derive(Clone, Debug, Args)]
pub struct ApplicationsArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, default_value = "")]
    pub mime: String,
}

#[derive(Clone, Debug, Args)]
pub struct BladeModulesArgs {
    #[arg(long)]
    pub plugin: String,
    #[arg(long, default_value = "")]
    pub user: String,
}

#[derive(Clone, Debug, Args)]
pub struct TransferArgs {
    #[arg(long)]
    pub destination: String,
    #[arg(long, action = ArgAction::Append, required = true)]
    pub source: Vec<String>,
    #[arg(long, default_value = "")]
    pub journal_id: String,
}

#[derive(Clone, Debug, Args)]
pub struct RenameArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub name_escaped: bool,
    #[arg(long, default_value = "")]
    pub journal_id: String,
}

#[derive(Clone, Debug, Args)]
pub struct CreateArgs {
    #[arg(long)]
    pub parent: String,
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub directory: bool,
    #[arg(long, default_value = "")]
    pub journal_id: String,
}

#[derive(Clone, Debug, Args)]
pub struct ColorArgs {
    #[arg(long, action = ArgAction::Append, required = true)]
    pub path: Vec<String>,
    #[arg(long, action = ArgAction::Append, required = true)]
    pub before: Vec<String>,
    #[arg(long, action = ArgAction::Append, required = true)]
    pub after: Vec<String>,
    #[arg(long, default_value = "")]
    pub journal_id: String,
}

#[derive(Clone, Debug, Args)]
pub struct JournalPathsArgs {
    #[arg(long, action = ArgAction::Append, required = true)]
    pub path: Vec<String>,
    #[arg(long)]
    pub follow_symlinks: bool,
    #[arg(long, default_value = "")]
    pub journal_id: String,
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
}

#[derive(Clone, Debug, Args)]
pub struct TrashRetentionArgs {
    #[arg(long, allow_hyphen_values = true)]
    pub days: i64,
}

#[derive(Clone, Debug, Args)]
pub struct JournalArgs {
    #[arg(long, default_value_t = 20, allow_hyphen_values = true)]
    pub limit: i64,
}

#[derive(Clone, Debug, Args)]
pub struct HistoryStepArgs {
    #[arg(long)]
    pub drop: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Clone, Debug, Args)]
pub struct SetDefaultArgs {
    #[arg(long)]
    pub mime: String,
    #[arg(long)]
    pub desktop_id: String,
}

#[derive(Clone, Debug, Args)]
pub struct UpdateArgs {
    #[arg(long = "repository")]
    pub repositories: Vec<String>,
    #[arg(long, default_value = "")]
    pub core: String,
}
