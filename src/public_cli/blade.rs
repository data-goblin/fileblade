use super::*;
use crate::backend::value_name;

pub(super) fn blade(action: BladeCommand) -> AppResult<PublicResult> {
    let response = match action {
        BladeCommand::Focus(value) => ipc("focusBlade", &[value_name(value.edge)])?,
        BladeCommand::ToggleFocus(value) => ipc("toggleBladeFocus", &[value_name(value.edge)])?,
        BladeCommand::Open(value) => ipc("openBlade", &[value_name(value.edge)])?,
        BladeCommand::Close(value) => ipc("closeBlade", &[value_name(value.edge)])?,
        BladeCommand::Toggle(value) => ipc("toggleBlade", &[value_name(value.edge)])?,
        BladeCommand::Settings(value) => ipc("toggleBladeSettings", &[value_name(value.edge)])?,
        BladeCommand::Undock(value) => ipc("undockBlade", &[value_name(value.edge)])?,
        BladeCommand::Dock(value) => ipc("dockBlade", &[value_name(value.edge)])?,
        BladeCommand::ToggleDock(value) => ipc("toggleBladeDock", &[value_name(value.edge)])?,
        BladeCommand::Width(value) => ipc(
            "setBladeWidth",
            &[value_name(value.edge), value.pixels.to_string()],
        )?,
        BladeCommand::Set(value) => {
            let modules = Value::Array(
                value
                    .modules
                    .split(',')
                    .map(str::trim)
                    .filter(|module| !module.is_empty())
                    .map(|module| json!({"module": module}))
                    .collect(),
            );
            ipc(
                "setBladeSlots",
                &[value_name(value.edge), encoded_document(&modules)?],
            )?
        }
        BladeCommand::Add(value) => ipc("addBladeModule", &[value_name(value.edge), value.module])?,
        BladeCommand::SlotModule(value) => ipc(
            "setSlotModule",
            &[
                value_name(value.edge),
                value.index.to_string(),
                value.module,
            ],
        )?,
        BladeCommand::TabSet(value) => ipc(
            "setBladeTab",
            &[
                value_name(value.edge),
                value.slot.to_string(),
                value.tab.to_string(),
            ],
        )?,
        BladeCommand::TabCycle(value) => ipc(
            "cycleBladeTab",
            &[
                value_name(value.edge),
                value.slot.to_string(),
                value.delta.to_string(),
            ],
        )?,
        BladeCommand::TabRemove(value) => ipc(
            "removeBladeTab",
            &[
                value_name(value.edge),
                value.slot.to_string(),
                value.tab.to_string(),
            ],
        )?,
        BladeCommand::TabInto(value) => ipc(
            "tabBladeSlot",
            &[
                value_name(value.source_edge),
                value.source_slot.to_string(),
                value_name(value.target_edge),
                value.target_slot.to_string(),
                value.tab.map(|tab| tab.to_string()).unwrap_or_default(),
                value.insert_at.map(|at| at.to_string()).unwrap_or_default(),
            ],
        )?,
        BladeCommand::Remove(value) => ipc(
            "removeBladeSlot",
            &[value_name(value.edge), value.index.to_string()],
        )?,
        BladeCommand::CollapseSlot(value) => ipc(
            "setBladeSlotCollapsed",
            &[
                value_name(value.edge),
                value.index.to_string(),
                "true".to_string(),
            ],
        )?,
        BladeCommand::ExpandSlot(value) => ipc(
            "setBladeSlotCollapsed",
            &[
                value_name(value.edge),
                value.index.to_string(),
                "false".to_string(),
            ],
        )?,
        BladeCommand::ToggleSlot(value) => ipc(
            "toggleBladeSlotCollapsed",
            &[value_name(value.edge), value.index.to_string()],
        )?,
        BladeCommand::Animations(value) => ipc(
            "setBladeAnimations",
            &[match value.state {
                BladeAnimationState::On => "on",
                BladeAnimationState::Off => "off",
                BladeAnimationState::Toggle => "toggle",
            }
            .to_string()],
        )?,
        BladeCommand::BarPlacement(value) => ipc(
            "setBarPlacement",
            &[match value.placement {
                BarPlacement::Below => "below",
                BarPlacement::Beside => "beside",
                BarPlacement::Toggle => "toggle",
            }
            .to_string()],
        )?,
        BladeCommand::Move(value) => ipc(
            "moveBladeModule",
            &[
                value.module,
                value_name(value.edge),
                value.index.to_string(),
            ],
        )?,
        BladeCommand::MoveSlot(value) => ipc(
            "moveBladeSlot",
            &[
                value_name(value.source_edge),
                value.source_index.to_string(),
                value_name(value.target_edge),
                value.index.to_string(),
            ],
        )?,
        BladeCommand::Release => ipc("releaseBladeFocus", &[])?,
    };
    Ok(PublicResult::one(response))
}

#[derive(Clone, Debug, Subcommand)]
pub enum BladeCommand {
    Focus(EdgeArg),
    SlotModule(BladeSlotModuleArgs),
    TabSet(BladeTabArgs),
    TabCycle(BladeTabCycleArgs),
    TabRemove(BladeTabArgs),
    TabInto(BladeTabIntoArgs),
    ToggleFocus(EdgeArg),
    Open(EdgeArg),
    Close(EdgeArg),
    Toggle(EdgeArg),
    Settings(EdgeArg),
    Undock(EdgeArg),
    Dock(EdgeArg),
    ToggleDock(EdgeArg),
    Width(BladeWidthArgs),
    Set(BladeSetArgs),
    Add(BladeModuleArgs),
    Remove(BladeIndexArgs),
    CollapseSlot(BladeIndexArgs),
    ExpandSlot(BladeIndexArgs),
    ToggleSlot(BladeIndexArgs),
    Animations(BladeAnimationArgs),
    BarPlacement(BarPlacementArgs),
    Move(BladeMoveArgs),
    MoveSlot(BladeMoveSlotArgs),
    Release,
}

#[derive(Clone, Debug, Args)]
pub struct EdgeArg {
    #[arg(value_enum)]
    pub edge: Edge,
}

#[derive(Clone, Debug, Args)]
pub struct BladeWidthArgs {
    #[arg(value_enum)]
    pub edge: Edge,
    #[arg(allow_hyphen_values = true)]
    pub pixels: i64,
}

#[derive(Clone, Debug, Args)]
pub struct BladeSetArgs {
    #[arg(value_enum)]
    pub edge: Edge,
    pub modules: String,
}

#[derive(Clone, Debug, Args)]
pub struct BladeModuleArgs {
    #[arg(value_enum)]
    pub edge: Edge,
    pub module: String,
}

#[derive(Clone, Debug, Args)]
pub struct BladeIndexArgs {
    #[arg(value_enum)]
    pub edge: Edge,
    #[arg(allow_hyphen_values = true)]
    pub index: i64,
}

#[derive(Clone, Debug, Args)]
pub struct BladeAnimationArgs {
    #[arg(value_enum)]
    pub state: BladeAnimationState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum BladeAnimationState {
    On,
    Off,
    Toggle,
}

#[derive(Clone, Debug, Args)]
pub struct BarPlacementArgs {
    #[arg(value_enum)]
    pub placement: BarPlacement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum BarPlacement {
    Below,
    Beside,
    Toggle,
}

#[derive(Clone, Debug, Args)]
pub struct BladeMoveArgs {
    pub module: String,
    #[arg(value_enum)]
    pub edge: Edge,
    #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
    pub index: i64,
}

#[derive(Clone, Debug, Args)]
pub struct BladeMoveSlotArgs {
    #[arg(value_enum)]
    pub source_edge: Edge,
    #[arg(allow_hyphen_values = true)]
    pub source_index: i64,
    #[arg(value_enum)]
    pub target_edge: Edge,
    #[arg(long, default_value_t = -1, allow_hyphen_values = true)]
    pub index: i64,
}

#[derive(Clone, Debug, Args)]
pub struct BladeSlotModuleArgs {
    #[arg(value_enum)]
    pub edge: Edge,
    pub index: u32,
    pub module: String,
}

#[derive(Clone, Debug, Args)]
pub struct BladeTabArgs {
    #[arg(value_enum)]
    pub edge: Edge,
    pub slot: u32,
    pub tab: u32,
}

#[derive(Clone, Debug, Args)]
pub struct BladeTabCycleArgs {
    #[arg(value_enum)]
    pub edge: Edge,
    pub slot: u32,
    #[arg(allow_hyphen_values = true, default_value_t = 1)]
    pub delta: i32,
}

#[derive(Clone, Debug, Args)]
pub struct BladeTabIntoArgs {
    #[arg(value_enum)]
    pub source_edge: Edge,
    pub source_slot: u32,
    #[arg(value_enum)]
    pub target_edge: Edge,
    pub target_slot: u32,
    #[arg(long)]
    pub tab: Option<u32>,
    #[arg(long)]
    pub insert_at: Option<u32>,
}
