use crate::AppResult;
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

mod args;
mod desktop_args;
mod documents;
mod plugin_args;
mod trash;
pub use args::*;
pub use desktop_args::*;
use documents::*;
pub use plugin_args::*;
use trash::*;
#[derive(Clone, Debug, Parser)]
#[command(name = "fileblade _backend", disable_version_flag = true)]
pub struct BackendCli {
    #[arg(long, global = true, default_value = "")]
    pub actor: String,
    #[command(subcommand)]
    pub command: BackendCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum BackendCommand {
    Agents,
    Recover,
    ProjectRoot(ProjectRootArgs),
    ChildrenBatch(ChildrenArgs),
    ChildrenWindow(ChildrenWindowArgs),
    GitMetadataBatch(PathsArgs),
    Search(SearchArgs),
    IndexInvalidate,
    FrecencyVisit(PathsArgs),
    FrecencyList(FrecencyListArgs),
    ArchiveList(PathArg),
    ArchiveExtract(ArchiveExtractArgs),
    Preview(PreviewArgs),
    Thumbnail(ThumbnailArgs),
    ThumbnailRender(ThumbnailRenderArgs),
    BinPut(BinPutArgs),
    BinRemove(BinRemoveArgs),
    BinList(BinModuleArgs),
    BinRestore(BinRestoreArgs),
    BinPurge(BinEntryArgs),
    Clipboard(ClipboardArgs),
    ClipboardWrite(ClipboardWriteArgs),
    ClipboardText(PathsArgs),
    KeybindingsPrepare,
    PreferencesRead,
    PreferencesSet(crate::preferences::Changes),
    StateRead,
    StateWrite(DocumentArgs),
    LayoutRead,
    LayoutWrite(DocumentArgs),
    SaveTarget(PathArg),
    ReadText(ReadTextArgs),
    Quicknav(QuicknavArgs),
    Visit(PathArg),
    Stat(PathArg),
    StatBatch(PathsArgs),
    Applications(ApplicationsArgs),
    BladeModules(BladeModulesArgs),
    ModuleDirs(ModuleDirsArgs),
    HelperRead(HelperArgs),
    HelperWrite(HelperArgs),
    Mounts,
    MountVolume(SourceArgs),
    UnmountVolume(SourceArgs),
    EjectVolume(SourceArgs),
    ActionList(ActionListArgs),
    ActionRun(ActionRunArgs),
    Copy(TransferArgs),
    Move(TransferArgs),
    Rename(RenameArgs),
    Create(CreateArgs),
    Color(ColorArgs),
    Trash(JournalPathsArgs),
    TrashList(TrashListArgs),
    TrashRestore(TrashRestoreArgs),
    TrashDelete(TrashDeleteArgs),
    TrashEmpty,
    TrashPrune(TrashRetentionArgs),
    Journal(JournalArgs),
    Undo(HistoryStepArgs),
    Redo(HistoryStepArgs),
    SetDefault(SetDefaultArgs),
    PluginCatalog,
    PluginInstall,
    PluginInstallStatus,
    HyprOption(HyprOptionArgs),
    UpdateCheck(UpdateArgs),
    ActiveWindow,
    FocusWindow(FocusWindowArgs),
    PlaceBladeWindow(PlaceBladeWindowArgs),
    FocusDirection(FocusDirectionArgs),
    HoverTarget(HoverTargetArgs),
    DimWindows(DimWindowsArgs),
    WindowDispatch(WindowDispatchArgs),
    DropContext(DropContextArgs),
    DropRun(DropRunArgs),
    DropPaste(DropPasteArgs),
    Launch(LaunchArgs),
}

pub fn parse<I, T>(arguments: I) -> Result<BackendCommand, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    parse_cli(arguments).map(|cli| cli.command)
}

pub fn parse_cli<I, T>(arguments: I) -> Result<BackendCli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    BackendCli::try_parse_from(arguments)
}

pub fn mutating(command: &BackendCommand) -> bool {
    matches!(
        command,
        BackendCommand::Copy(_)
            | BackendCommand::Move(_)
            | BackendCommand::Rename(_)
            | BackendCommand::Create(_)
            | BackendCommand::Color(_)
            | BackendCommand::Trash(_)
            | BackendCommand::TrashRestore(_)
            | BackendCommand::TrashDelete(_)
            | BackendCommand::TrashEmpty
            | BackendCommand::TrashPrune(_)
            | BackendCommand::Undo(_)
            | BackendCommand::Redo(_)
            | BackendCommand::BinRestore(_)
            | BackendCommand::BinPut(_)
            | BackendCommand::BinRemove(_)
            | BackendCommand::BinPurge(_)
            | BackendCommand::ArchiveExtract(_)
            | BackendCommand::PluginInstall
            | BackendCommand::MountVolume(_)
            | BackendCommand::UnmountVolume(_)
            | BackendCommand::EjectVolume(_)
            | BackendCommand::ActionRun(_)
            | BackendCommand::HelperWrite(_)
            | BackendCommand::PreferencesSet(_)
            | BackendCommand::KeybindingsPrepare
    )
}

pub fn dispatch(
    command: BackendCommand,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(Value) -> AppResult<()>,
) -> AppResult<Value> {
    dispatch_command(command, cancelled, progress)
}

fn dispatch_command(
    command: BackendCommand,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(Value) -> AppResult<()>,
) -> AppResult<Value> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(crate::AppError::Cancelled);
    }
    let mutates = mutating(&command);
    let value = match command {
        BackendCommand::Agents => crate::agents::installed_agents(),
        BackendCommand::HelperRead(options) => options.execute(false, cancelled)?,
        BackendCommand::HelperWrite(options) => options.execute(true, cancelled)?,
        BackendCommand::Recover => crate::recovery::sweep(),
        BackendCommand::ProjectRoot(options) => crate::project::project_root(&options.path),
        BackendCommand::ChildrenBatch(options) => crate::filesystem::children_batch_paged(
            &options.path[..options.path.len().min(64)],
            options.show_hidden,
            &crate::filesystem::ChildrenPage {
                limit: options.limit,
                sort: options.sort.clone(),
                descending: options.desc,
                filter: filter_document(&options.filter),
                include_created: options.include_created,
                git_enabled: !options.no_git,
                fresh_git: options.fresh_git,
            },
            cancelled,
        ),
        BackendCommand::ChildrenWindow(options) => crate::listing::window(
            &crate::listing::WindowRequest {
                path: options.path.clone(),
                show_hidden: options.show_hidden,
                start: options.start,
                count: options
                    .count
                    .clamp(1, crate::filesystem::DIRECTORY_ENTRY_LIMIT),
                sort: options.sort.clone(),
                descending: options.desc,
                filter: filter_document(&options.filter),
                include_created: options.include_created,
                fresh: options.fresh,
                git_enabled: !options.no_git,
                fresh_git: options.fresh_git,
            },
            cancelled,
        ),
        BackendCommand::GitMetadataBatch(options) => crate::git::git_metadata_batch_cancellable(
            &options.path[..options.path.len().min(1000)],
            cancelled,
        ),
        BackendCommand::Search(options) => crate::search::search_streaming(
            &crate::search::SearchRequest {
                root: &options.root,
                query: &options.query,
                show_hidden: options.show_hidden,
                limit: limited(options.limit, crate::search::SEARCH_CANDIDATE_CAP),
                repository_roots: &options.repository[..options.repository.len().min(64)],
                options: crate::search::SearchOptions {
                    case_sensitive: options.case_sensitive,
                    regex: options.regex,
                },
                fresh: options.fresh,
                list: options.list.as_deref(),
                tree: options.tree,
                git_enabled: !options.no_git,
            },
            cancelled,
            progress,
        ),
        BackendCommand::IndexInvalidate => {
            crate::index::invalidate_all();
            crate::listing::invalidate_all();
            json!({"ok": true})
        }
        BackendCommand::FrecencyVisit(options) => {
            crate::frecency::visit(&options.path[..options.path.len().min(64)])
        }
        BackendCommand::ArchiveList(options) => crate::archive::list(&options.path, cancelled),
        BackendCommand::ArchiveExtract(options) => crate::archive::extract(
            &options.path,
            options.destination.as_deref(),
            options.merge,
            cancelled,
        ),
        BackendCommand::Preview(options) => {
            crate::preview::preview(&options.path, limited(options.lines, 2000), cancelled)
        }
        BackendCommand::Thumbnail(options) => crate::thumbnail::thumbnail(
            &options.path,
            &options.key,
            options.width,
            options.height,
            cancelled,
        ),
        BackendCommand::ThumbnailRender(options) => crate::thumbnail::render(
            &options.path,
            &crate::common::parse_path(&options.target)?,
            options.width,
            options.height,
        )?,
        BackendCommand::FrecencyList(options) => crate::frecency::list_with_hidden(
            limited(options.limit, crate::frecency::FRECENCY_CAP),
            &options.query,
            options.show_hidden,
        ),
        BackendCommand::BinPut(options) => {
            crate::artifact_bin::put(&options.module, &options.item, cancelled)
        }
        BackendCommand::BinRemove(options) => crate::artifact_bin::remove_with_helper(
            &options.item.module,
            &options.item.item,
            &options.helper_route,
            &options.arguments,
            cancelled,
        ),
        BackendCommand::BinList(options) => crate::artifact_bin::rows(&options.module),
        BackendCommand::BinRestore(options) => crate::artifact_bin::restore_with_helper(
            &options.entry.module,
            &options.entry.id,
            options.helper_route.as_deref(),
            cancelled,
        ),
        BackendCommand::BinPurge(options) => {
            crate::artifact_bin::purge(&options.module, &options.id)
        }
        BackendCommand::Clipboard(options) => {
            crate::desktop::clipboard_files_cancellable(limited(options.limit, 4096), cancelled)
        }
        BackendCommand::ClipboardWrite(options) => {
            clipboard_write(&options.path, options.cut, cancelled)
        }
        BackendCommand::ClipboardText(options) => clipboard_text(&options.path, cancelled),
        BackendCommand::KeybindingsPrepare => {
            json!({"ok":true,"text":crate::preferences::keybindings()?})
        }
        BackendCommand::PreferencesRead => {
            json!({"ok":true,"settings":crate::preferences::read()?})
        }
        BackendCommand::PreferencesSet(changes) => {
            json!({"ok":true,"settings":crate::preferences::change(&changes)?})
        }
        BackendCommand::StateRead => private_document_read(&state_path()),
        BackendCommand::StateWrite(options) => {
            private_document_write(&state_path(), &options.document)
        }
        BackendCommand::LayoutRead => private_document_read(&layout_path()),
        BackendCommand::LayoutWrite(options) => {
            private_document_write(&layout_path(), &options.document)
        }
        BackendCommand::SaveTarget(options) => {
            crate::filesystem::validate_save_target(&options.path)
        }
        BackendCommand::ReadText(options) => {
            crate::filesystem::read_text(&options.path, limited(options.limit, 1024 * 1024))
        }
        BackendCommand::Quicknav(options) => crate::quicknav::quicknav_cancellable(
            &options.query,
            &options.exclude,
            &options.root,
            options.show_hidden,
            limited(options.limit, 200),
            cancelled,
        ),
        BackendCommand::Visit(options) => {
            crate::quicknav::record_zoxide_visit_cancellable(&options.path, cancelled)
        }
        BackendCommand::Stat(options) => {
            crate::filesystem::stat_path_cancellable(&options.path, cancelled)
        }
        BackendCommand::StatBatch(options) => {
            crate::filesystem::stat_paths(&options.path[..options.path.len().min(512)])
        }
        BackendCommand::Applications(options) => {
            crate::desktop::applications_for_cancellable(&options.path, &options.mime, cancelled)
        }
        BackendCommand::BladeModules(options) => {
            crate::modules::discover_blade_modules(&options.plugin, &options.user)
        }
        BackendCommand::ModuleDirs(options) => crate::module_dirs::ensure(&options.module)?,
        BackendCommand::Mounts => crate::mounts::payload()?,
        BackendCommand::MountVolume(options) => crate::mounts::mount(&options.source)?,
        BackendCommand::UnmountVolume(options) => crate::mounts::unmount(&options.source)?,
        BackendCommand::EjectVolume(options) => crate::mounts::eject(&options.source)?,
        BackendCommand::ActionList(options) => {
            crate::actions::list(&options.provider, &options.user)
        }
        BackendCommand::ActionRun(options) => crate::actions::run(
            &crate::actions::RunRequest {
                source: options.source,
                plugin: options.plugin,
                plugin_dir: options.plugin_dir,
                action: options.action,
                context: options.context,
                root: options.root,
                paths: options.path,
                yes: options.yes,
                screen: options.screen,
            },
            cancelled,
        )?,
        BackendCommand::Copy(options) => transfer(
            true,
            &options.source,
            &options.destination,
            &options.journal_id,
            cancelled,
            progress,
        )?,
        BackendCommand::Move(options) => transfer(
            false,
            &options.source,
            &options.destination,
            &options.journal_id,
            cancelled,
            progress,
        )?,
        BackendCommand::Rename(options) => crate::operations::rename_path_with_name_format(
            &options.path,
            &options.name,
            &options.journal_id,
            options.name_escaped,
        ),
        BackendCommand::Create(options) => crate::operations::create_path(
            &options.parent,
            &options.name,
            options.directory,
            &options.journal_id,
        ),
        BackendCommand::Color(options) => crate::journal::record_colors(
            &options.path,
            &options.before,
            &options.after,
            &options.journal_id,
        )?,
        BackendCommand::Trash(options) => crate::operations::trash_paths(
            &options.path,
            &options.journal_id,
            options.follow_symlinks,
            cancelled,
        ),
        BackendCommand::TrashList(options) => trash_list(limited(options.limit, 1000), cancelled),
        BackendCommand::TrashRestore(options) => crate::trash::restore(
            &options.id,
            &options.destination,
            options.recreate_parent,
            cancelled,
        ),
        BackendCommand::TrashDelete(options) => {
            crate::trash::delete_permanently(&options.id, cancelled)
        }
        BackendCommand::TrashEmpty => trash_empty(cancelled, progress)?,
        BackendCommand::TrashPrune(options) => trash_prune(options.days, cancelled, progress)?,
        BackendCommand::Journal(options) => {
            crate::journal::journal_document(limited(options.limit, 100))
        }
        BackendCommand::Undo(options) => {
            crate::journal::undo(options.drop, options.force, cancelled)
        }
        BackendCommand::Redo(options) => {
            crate::journal::redo(options.drop, options.force, cancelled)
        }
        BackendCommand::PluginCatalog => crate::plugin_catalog::catalog()?,
        BackendCommand::PluginInstall => crate::plugin_install::install()?,
        BackendCommand::PluginInstallStatus => crate::plugin_install::status()?,
        BackendCommand::SetDefault(options) => crate::operations::set_default_application(
            &options.mime,
            &options.desktop_id,
            cancelled,
        ),
        BackendCommand::UpdateCheck(options) => crate::updates::check(
            &crate::updates::parse_specs(&options.repositories),
            &options.core,
            cancelled,
        ),
        BackendCommand::HyprOption(options) => {
            crate::hyprland::hypr_query(&format!("getoption {}", options.name.as_str()))?
        }
        BackendCommand::ActiveWindow => crate::hyprland::active_window(),
        BackendCommand::FocusWindow(options) => crate::hyprland::focus_window(&options.address),
        BackendCommand::PlaceBladeWindow(options) => crate::hyprland::place_blade_window(
            &crate::hyprland::PlaceBladeOptions {
                title: options.title,
                edge: options.edge.as_str().to_string(),
                width: options.width,
                timeout: seconds(options.timeout, 0.2, 900.0),
            },
            cancelled,
        ),
        BackendCommand::FocusDirection(options) => {
            crate::hyprland::focus_direction(&crate::hyprland::FocusDirectionOptions {
                direction: options.direction,
                left_state: options.left,
                right_state: options.right,
                from_blade: options.from_blade,
                blade_titles: options.blade_title,
                empty_only: options.empty_only,
                left_monitor: options.left_monitor,
                right_monitor: options.right_monitor,
            })
        }
        BackendCommand::HoverTarget(options) => crate::hyprland::hover_target(&options.blade_title),
        BackendCommand::DimWindows(options) => {
            if let Some(owner) = options.after_exit {
                if options.state != "off" {
                    return Err(crate::AppError::invalid(
                        "--after-exit requires --state off",
                    ));
                }
                crate::hyprland::restore_borders_after(owner)
            } else {
                crate::hyprland::dim_windows(&options.state, &options.exclude_title)
            }
        }
        BackendCommand::WindowDispatch(options) => crate::hyprland::window_dispatch(
            options.action.as_str(),
            options.x,
            options.y,
            &options.direction,
        ),
        BackendCommand::DropContext(options) => crate::drop_target::drop_context(
            options.x,
            options.y,
            &options.path,
            &options.blade_title,
        ),
        BackendCommand::DropRun(options) => {
            crate::drop_target::drop_run(&crate::drop_target::DropRunOptions {
                action: options.action,
                placement: options.placement,
                paths: options.path,
                target_json: options.target,
                desktop_id: options.desktop_id,
                dry_run: options.dry_run,
            })
        }
        BackendCommand::DropPaste(options) => {
            crate::drop_target::drop_paste(&crate::drop_target::DropPasteOptions {
                x: options.x,
                y: options.y,
                form: options.form.as_str().to_string(),
                paths: options.path,
                blade_titles: options.blade_title,
                dry_run: options.dry_run,
            })
        }
        BackendCommand::Launch(options) => {
            crate::hyprland::launch_path(&crate::hyprland::LaunchOptions {
                path: options.path,
                mode: options.mode.as_str().to_string(),
                desktop_id: options.desktop_id,
                timeout: seconds(options.timeout, 0.25, 20.0),
                line: options.line,
            })
        }
    };
    if mutates {
        crate::index::invalidate_all();
        crate::listing::invalidate_all();
        crate::frecency::remap_from(&value);
    }
    Ok(value)
}
