use super::*;
use crate::extension_template::image::{
    BANNER_FILE, BANNER_PNG, HOST_LOGO_PNG, banner_svg, host_logo_svg, manifest_name, rasterize,
};
use crate::extension_template::{self, Request, check};
use std::path::Path;

const MIN_PNG_WIDTH: u32 = 96;
const MAX_PNG_WIDTH: u32 = 8192;

#[derive(Clone, Debug, Subcommand)]
pub enum ExtensionCommand {
    /// Write a starter FileBlade extension (an Omarchy plugin with one blade module) into a new directory.
    Template(ExtensionTemplateArgs),
    /// Check an extension directory against the FileBlade extension contract.
    Check(ExtensionCheckArgs),
    /// Write the extension banner, outlined so the file needs no fonts installed.
    Image(ExtensionImageArgs),
}

#[derive(Clone, Debug, Args)]
pub struct ExtensionCheckArgs {
    /// Extension directory to check; defaults to the working directory.
    pub directory: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub struct ExtensionImageArgs {
    /// Extension name as shown in the README, for example 'Agent Skills'; defaults to the manifest name without its 'FileBlade ' prefix.
    #[arg(long)]
    pub name: Option<String>,
    /// Full subtitle to outline instead of '<NAME> EXTENSION'.
    #[arg(long)]
    pub text: Option<String>,
    /// Manifest read when --name is absent.
    #[arg(long, default_value = "manifest.json")]
    pub manifest: String,
    /// Output directory.
    #[arg(long, default_value = "assets")]
    pub out: String,
    /// Also rasterize the banner with rsvg-convert.
    #[arg(long)]
    pub png: bool,
    /// Also rasterize the plain FileBlade logo the host guard shows (assets/fileblade-logo.png).
    #[arg(long = "host-logo")]
    pub host_logo: bool,
    /// PNG width in pixels.
    #[arg(long, default_value_t = 1920)]
    pub width: u32,
    /// Print the banner SVG instead of writing files.
    #[arg(long)]
    pub stdout: bool,
}

#[derive(Clone, Debug, Args)]
pub struct ExtensionTemplateArgs {
    /// Omarchy plugin id as publisher.name, for example acme.fileblade-weather.
    pub id: String,
    /// Directory to create; defaults to ./<id>.
    pub directory: Option<String>,
    /// Extension name for tabs and the README; defaults to the module id in title case.
    #[arg(long)]
    pub name: Option<String>,
    /// Blade module id; defaults to the plugin name without its fileblade- prefix.
    #[arg(long)]
    pub module: Option<String>,
    /// Manifest author and LICENSE holder; defaults to $USER.
    #[arg(long)]
    pub author: Option<String>,
    /// One line for the manifest and the module picker.
    #[arg(long)]
    pub description: Option<String>,
    /// Git URL used by the README install command; defaults to https://github.com/<publisher>/<name>.git.
    #[arg(long)]
    pub repository: Option<String>,
    /// Write into a directory that already has files, replacing only the template's own files.
    #[arg(long)]
    pub force: bool,
}

pub(super) fn extension(action: ExtensionCommand) -> AppResult<PublicResult> {
    match action {
        ExtensionCommand::Template(options) => template(options),
        ExtensionCommand::Check(options) => run_check(options),
        ExtensionCommand::Image(options) => image(options),
    }
}

fn template(options: ExtensionTemplateArgs) -> AppResult<PublicResult> {
    let request = Request {
        id: options.id,
        name: options.name,
        module: options.module,
        author: options.author,
        description: options.description,
        repository: options.repository,
    };
    let scaffold = extension_template::scaffold(&request)?;
    let directory = options.directory.unwrap_or_else(|| scaffold.id.clone());
    let directory = absolute_directory(&directory)?;
    let files = extension_template::render(&scaffold)?;
    let written = extension_template::write(Path::new(&directory), &files, options.force)?;
    let next = extension_template::next_steps(&scaffold, &directory);
    let mut lines = vec![format!(
        "Created {} ({}) in {directory}",
        scaffold.id, scaffold.name
    )];
    lines.extend(written.iter().map(|path| format!("  {path}")));
    lines.push(String::new());
    lines.push("Next:".to_string());
    lines.extend(next.iter().map(|step| format!("  {step}")));
    let document = json!({
        "ok": true,
        "id": scaffold.id,
        "name": scaffold.name,
        "module": format!("{}/{}", scaffold.id, scaffold.module),
        "directory": directory,
        "files": written,
        "next": next,
    });
    Ok(PublicResult::lines(lines, document))
}

fn absolute_directory(value: &str) -> AppResult<String> {
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    absolute
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::invalid("the directory must be valid UTF-8"))
}

fn run_check(options: ExtensionCheckArgs) -> AppResult<PublicResult> {
    let directory = options.directory.unwrap_or_else(|| ".".to_string());
    let directory = absolute_directory(&directory)?;
    let checks = check::run(Path::new(&directory))?;
    let lines = vec![format!("contract: {} checks ok", checks.len())];
    let document = json!({"ok": true, "directory": directory, "checks": checks});
    Ok(PublicResult::lines(lines, document))
}

fn image(options: ExtensionImageArgs) -> AppResult<PublicResult> {
    let name = match &options.name {
        Some(name) => name.clone(),
        None => manifest_name(Path::new(&options.manifest))?,
    };
    let svg = banner_svg(&name, options.text.as_deref())?;
    if options.stdout {
        return Ok(PublicResult::lines(
            vec![svg.trim_end_matches('\n').to_string()],
            json!({"ok": true, "svg": svg}),
        ));
    }
    let width = options.width.clamp(MIN_PNG_WIDTH, MAX_PNG_WIDTH);
    let out = PathBuf::from(absolute_directory(&options.out)?);
    std::fs::create_dir_all(&out)?;
    let banner = out.join(BANNER_FILE);
    std::fs::write(&banner, svg.as_bytes())?;
    let mut written = vec![path_string(&banner)?];
    if options.png {
        let target = out.join(BANNER_PNG);
        rasterize(&svg, &target, width)?;
        written.push(path_string(&target)?);
    }
    if options.host_logo {
        let target = out.join(HOST_LOGO_PNG);
        rasterize(&host_logo_svg(), &target, width)?;
        written.push(path_string(&target)?);
    }
    let document = json!({"ok": true, "files": written});
    Ok(PublicResult::lines(written, document))
}

fn path_string(path: &Path) -> AppResult<String> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| AppError::invalid("the output path must be valid UTF-8"))
}
