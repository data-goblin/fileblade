use super::*;

#[derive(Clone, Debug, Args)]
pub struct HyprOptionArgs {
    #[arg(long, value_enum)]
    pub name: HyprOption,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum HyprOption {
    #[value(name = "general:border_size")]
    BorderSize,
    #[value(name = "animations:enabled")]
    AnimationsEnabled,
    #[value(name = "decoration:rounding")]
    Rounding,
    #[value(name = "general:gaps_out")]
    GapsOut,
}

#[derive(Clone, Debug, Args)]
pub struct FocusWindowArgs {
    #[arg(long)]
    pub address: String,
}

#[derive(Clone, Debug, Args)]
pub struct PlaceBladeWindowArgs {
    #[arg(long)]
    pub title: String,
    #[arg(long, value_enum, default_value_t = Edge::Left)]
    pub edge: Edge,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub width: i64,
    #[arg(long, default_value_t = 4.0, allow_hyphen_values = true)]
    pub timeout: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Edge {
    Left,
    Right,
}

#[derive(Clone, Debug, Args)]
pub struct FocusDirectionArgs {
    #[arg(long, value_parser = ["l", "r", "left", "right"])]
    pub direction: String,
    #[arg(long, value_parser = ["closed", "open", "window"], default_value = "closed")]
    pub left: String,
    #[arg(long, value_parser = ["closed", "open", "window"], default_value = "closed")]
    pub right: String,
    #[arg(long, value_parser = ["", "left", "right"], default_value = "")]
    pub from_blade: String,
    #[arg(long, action = ArgAction::Append)]
    pub blade_title: Vec<String>,
    #[arg(long)]
    pub empty_only: bool,
    #[arg(long, default_value = "")]
    pub left_monitor: String,
    #[arg(long, default_value = "")]
    pub right_monitor: String,
}

#[derive(Clone, Debug, Args)]
pub struct HoverTargetArgs {
    #[arg(long, action = ArgAction::Append)]
    pub blade_title: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct DimWindowsArgs {
    #[arg(long, value_parser = ["on", "off"])]
    pub state: String,
    #[arg(long, action = ArgAction::Append)]
    pub exclude_title: Vec<String>,
    #[arg(long)]
    pub after_exit: Option<u32>,
}

#[derive(Clone, Debug, Args)]
pub struct WindowDispatchArgs {
    #[arg(long, value_enum)]
    pub action: WindowAction,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub x: i64,
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub y: i64,
    #[arg(long, default_value = "")]
    pub direction: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum WindowAction {
    Close,
    Float,
    Resize,
    Swap,
    Focus,
}

#[derive(Clone, Debug, Args)]
pub struct DropContextArgs {
    #[arg(long, allow_hyphen_values = true)]
    pub x: Option<i64>,
    #[arg(long, allow_hyphen_values = true)]
    pub y: Option<i64>,
    #[arg(long, action = ArgAction::Append)]
    pub path: Vec<String>,
    #[arg(long, action = ArgAction::Append)]
    pub blade_title: Vec<String>,
}

#[derive(Clone, Debug, Args)]
pub struct DropRunArgs {
    #[arg(long)]
    pub action: String,
    #[arg(long, default_value = "")]
    pub placement: String,
    #[arg(long, action = ArgAction::Append)]
    pub path: Vec<String>,
    #[arg(long, default_value = "")]
    pub target: String,
    #[arg(long, default_value = "")]
    pub desktop_id: String,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Args)]
pub struct DropPasteArgs {
    #[arg(long, allow_hyphen_values = true)]
    pub x: Option<i64>,
    #[arg(long, allow_hyphen_values = true)]
    pub y: Option<i64>,
    #[arg(long, value_enum, default_value_t = PathForm::Absolute)]
    pub form: PathForm,
    #[arg(long, action = ArgAction::Append)]
    pub path: Vec<String>,
    #[arg(long, action = ArgAction::Append)]
    pub blade_title: Vec<String>,
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum PathForm {
    Absolute,
    Relative,
}

#[derive(Clone, Debug, Args)]
pub struct LaunchArgs {
    #[arg(long)]
    pub path: String,
    #[arg(long, value_enum, default_value_t = LaunchMode::Default)]
    pub mode: LaunchMode,
    #[arg(long, default_value = "")]
    pub desktop_id: String,
    #[arg(long, default_value_t = 6.0, allow_hyphen_values = true)]
    pub timeout: f64,
    #[arg(long, default_value_t = 0)]
    pub line: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum LaunchMode {
    Default,
    Editor,
    Reveal,
    Application,
    Url,
}

pub(crate) fn value_name(value: impl ValueEnum) -> String {
    value
        .to_possible_value()
        .map(|value| value.get_name().to_string())
        .unwrap_or_default()
}
