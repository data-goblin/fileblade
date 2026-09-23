use crate::command::CommandSpec;
use crate::common::{own_binary, parse_path, path_text};
use crate::paths::xdg_home;
use crate::secure::{self, read_bounded_nofollow};
use crate::{AppError, AppResult};
use base64::{Engine as _, prelude::BASE64_STANDARD};
use image::{DynamicImage, ImageDecoder, ImageEncoder, ImageFormat, ImageReader, Limits};
use serde_json::{Value, json};
use std::io::{Cursor, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const INPUT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_SOURCE_EDGE: u32 = 16_384;
pub const MAX_SOURCE_PIXELS: u64 = 64_000_000;
pub const MAX_OUTPUT_EDGE: u32 = 1024;
pub const DECODE_MEMORY_BYTES: u64 = 512 * 1024 * 1024;
const RENDER_TIMEOUT: Duration = Duration::from_secs(8);
const CACHE_BYTES: usize = 8 * 1024 * 1024;
const RENDER_OUTPUT_BYTES: usize = CACHE_BYTES.div_ceil(3) * 4 + 64 * 1024;

struct EncodedThumbnail {
    bytes: Vec<u8>,
    value: Value,
}

pub fn cache_dir() -> PathBuf {
    xdg_home("XDG_CACHE_HOME", "~/.cache").join("fileblade/thumbnails")
}

pub fn cache_path(key: &str) -> PathBuf {
    cache_dir().join(format!("{}.png", digest(key)))
}

pub fn thumbnail(
    raw_path: &str,
    key: &str,
    width: u32,
    height: u32,
    cancelled: &AtomicBool,
) -> Value {
    let path = match parse_path(raw_path) {
        Ok(path) => path,
        Err(error) => return failure(raw_path, &error.to_string()),
    };
    let text = path_text(&path);
    let (width, height) = bounded_size(width, height);
    let version = match source_version(&path) {
        Ok(version) => version,
        Err(error) => return failure(&text, &error.to_string()),
    };
    let output = cache_path(&format!(
        "oriented-icc-v2\n{text}\n{version}\n{key}\n{width}x{height}"
    ));
    if let Some(ready) = cached(&output, width, height) {
        return ready;
    }
    let input = match read_bounded_nofollow(&path, INPUT_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return failure(&text, &format!("{text} is missing")),
        Err(error) => return failure(&text, &error.to_string()),
    };
    match render_in_child(&path, &output, width, height, &version, input, cancelled) {
        Ok(value) if source_version(&path).is_ok_and(|current| current == version) => value,
        Ok(_) => failure(&text, "Source changed while creating the thumbnail"),
        Err(error) => failure(&text, &error.to_string()),
    }
}

pub fn render(raw_path: &str, output: &Path, width: u32, height: u32) -> AppResult<Value> {
    let rendered = render_encoded(raw_path, width, height, None)?;
    secure::write_private_atomic(output, &rendered.bytes)
        .map_err(|error| AppError::Invalid(error.to_string()))?;
    Ok(rendered_value(output, rendered, false))
}

pub fn render_worker(raw_path: &str, output: &Path, width: u32, height: u32) -> AppResult<Value> {
    Ok(rendered_value(
        output,
        render_encoded(raw_path, width, height, Some(worker_input()?))?,
        true,
    ))
}

fn render_encoded(
    raw_path: &str,
    width: u32,
    height: u32,
    input: Option<Vec<u8>>,
) -> AppResult<EncodedThumbnail> {
    let path = parse_path(raw_path)?;
    let text = path_text(&path);
    let (width, height) = bounded_size(width, height);
    confine_memory()?;
    let bytes = match input {
        Some(bytes) => bytes,
        None => read_bounded_nofollow(&path, INPUT_BYTES)
            .map_err(|error| AppError::Invalid(error.to_string()))?
            .ok_or_else(|| AppError::Invalid(format!("{text} is missing")))?,
    };
    let format = image::guess_format(&bytes).ok().filter(|format| {
        matches!(
            format,
            ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
        )
    });
    let Some(format) = format else {
        return render_poster(&bytes, width, height).map_err(|error| {
            AppError::Invalid(format!(
                "{text} is not a PNG, JPEG, or WebP image; poster: {error}"
            ))
        });
    };
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_EDGE);
    limits.max_image_height = Some(MAX_SOURCE_EDGE);
    limits.max_alloc = Some(DECODE_MEMORY_BYTES / 2);
    reader.limits(limits);
    let (source_width, source_height) = reader
        .into_dimensions()
        .map_err(|error| AppError::Invalid(format!("{text}: {error}")))?;
    if u64::from(source_width) * u64::from(source_height) > MAX_SOURCE_PIXELS {
        return Err(AppError::Invalid(format!(
            "{text} has {source_width}x{source_height} pixels, above the {MAX_SOURCE_PIXELS} limit"
        )));
    }
    let mut reader = ImageReader::with_format(Cursor::new(&bytes), format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_EDGE);
    limits.max_image_height = Some(MAX_SOURCE_EDGE);
    limits.max_alloc = Some(DECODE_MEMORY_BYTES / 2);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|error| AppError::Invalid(format!("{text}: {error}")))?;
    let orientation = decoder
        .orientation()
        .map_err(|error| AppError::Invalid(format!("{text}: {error}")))?;
    let profile = decoder
        .icc_profile()
        .map_err(|error| AppError::Invalid(format!("{text}: {error}")))?;
    if let Some(profile) = &profile {
        let space = if decoder.color_type().has_color() {
            b"RGB "
        } else {
            b"GRAY"
        };
        if profile.len() > 1024 * 1024 || profile.get(16..20) != Some(space.as_slice()) {
            return Err(AppError::Invalid(
                "Preview colour profile is unsupported or above the size limit".into(),
            ));
        }
    }
    let mut decoded = DynamicImage::from_decoder(decoder)
        .map_err(|error| AppError::Invalid(format!("{text}: {error}")))?;
    decoded.apply_orientation(orientation);
    let scaled = if decoded.width() > width || decoded.height() > height {
        decoded.thumbnail(width, height)
    } else {
        decoded
    };
    let mut encoded = Cursor::new(Vec::new());
    if let Some(profile) = profile {
        let mut encoder = image::codecs::png::PngEncoder::new(&mut encoded);
        encoder
            .set_icc_profile(profile)
            .map_err(|error| AppError::Invalid(error.to_string()))?;
        scaled
            .write_with_encoder(encoder)
            .map_err(|error| AppError::Invalid(error.to_string()))?;
    } else {
        scaled
            .to_rgba8()
            .write_to(&mut encoded, ImageFormat::Png)
            .map_err(|error| AppError::Invalid(error.to_string()))?;
    }
    if encoded.get_ref().len() > CACHE_BYTES {
        return Err(AppError::Invalid(
            "Thumbnail output exceeds the cache limit".into(),
        ));
    }
    Ok(EncodedThumbnail {
        bytes: encoded.into_inner(),
        value: json!({
            "ok": true,
            "width": scaled.width(),
            "height": scaled.height(),
            "source_width": source_width,
            "source_height": source_height
        }),
    })
}

fn rendered_value(output: &Path, rendered: EncodedThumbnail, worker: bool) -> Value {
    let EncodedThumbnail { bytes, mut value } = rendered;
    value["path"] = Value::String(path_text(output));
    if worker {
        value["png_base64"] = Value::String(BASE64_STANDARD.encode(bytes));
    }
    value
}

fn poster_command(program: &str, bytes: &[u8]) -> CommandSpec {
    let command = CommandSpec::new(program)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("MALLOC_ARENA_MAX", "2")
        .args([
            "-v", "error", "-max_alloc", "67108864",
            "-protocol_whitelist", "pipe,fd",
            "-format_whitelist", "bmp_pipe,gif,gif_pipe,tiff_pipe,ico,pbm_pipe,pgm_pipe,pgmyuv_pipe,ppm_pipe,pam_pipe,pfm_pipe,svg_pipe,mov,matroska,webm,jpegxl_pipe,hdr_pipe,exr_pipe,psd_pipe,avi,mpeg,mpegts,ogg,flv,asf",
            "-codec_whitelist", "bmp,png,gif,tiff,pbm,pgm,pgmyuv,ppm,pam,pfm,librsvg,hevc,av1,libdav1d,libaom-av1,libjxl,libjxl_anim,hdr,exr,psd,h264,vp8,vp9,ffv1,mpeg1video,mpeg2video,mpeg4,theora,flv,wmv1,wmv2,wmv3,vc1,mjpeg",
            "-max_streams", "16", "-threads", "1",
        ])
        .args(["-frame_size", &bytes.len().to_string(), "-max_pixels", &MAX_SOURCE_PIXELS.to_string()])
        .seekable_stdin(bytes)
        .timeout(Duration::from_secs(4))
        .resource_limits(0, DECODE_MEMORY_BYTES)
        .limits(CACHE_BYTES, 16 * 1024)
        .stop_on_output_limit();
    if bytes.starts_with(b"P7") {
        command.args(["-f", "pam_pipe"])
    } else {
        command
    }
}

fn render_poster(bytes: &[u8], width: u32, height: u32) -> AppResult<EncodedThumbnail> {
    let probe = poster_command("ffprobe", bytes)
        .args([
            "-i",
            "fd:",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height",
            "-of",
            "json",
        ])
        .limits(16 * 1024, 16 * 1024)
        .run()?;
    if !probe.status.success() {
        return Err(AppError::Invalid(
            String::from_utf8_lossy(&probe.stderr).trim().into(),
        ));
    }
    let metadata: Value = serde_json::from_slice(&probe.stdout)
        .map_err(|error| AppError::Invalid(error.to_string()))?;
    let source_width = metadata["streams"][0]["width"].as_u64().unwrap_or(0);
    let source_height = metadata["streams"][0]["height"].as_u64().unwrap_or(0);
    if source_width == 0
        || source_height == 0
        || source_width > u64::from(MAX_SOURCE_EDGE)
        || source_height > u64::from(MAX_SOURCE_EDGE)
        || source_width * source_height > MAX_SOURCE_PIXELS
    {
        return Err(AppError::Invalid(
            "Poster dimensions are unavailable or above the source limit".into(),
        ));
    }
    let scale = format!(
        "scale=w='min({width},iw)':h='min({height},ih)':force_original_aspect_ratio=decrease"
    );
    let frame = poster_command("ffmpeg", bytes)
        .args([
            "-filter_threads",
            "1",
            "-i",
            "fd:",
            "-map",
            "0:v:0",
            "-frames:v",
            "1",
            "-vf",
            &scale,
            "-threads",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "-pix_fmt",
            "rgba",
            "pipe:1",
        ])
        .run()?;
    if !frame.status.success() {
        return Err(AppError::Invalid(
            String::from_utf8_lossy(&frame.stderr).trim().into(),
        ));
    }
    if frame.stdout_truncated || frame.stdout.len() > CACHE_BYTES {
        return Err(AppError::Invalid(
            "Poster output exceeds the cache limit".into(),
        ));
    }
    let (actual_width, actual_height) =
        ImageReader::with_format(Cursor::new(&frame.stdout), ImageFormat::Png)
            .into_dimensions()
            .map_err(|error| AppError::Invalid(error.to_string()))?;
    if actual_width == 0 || actual_height == 0 || actual_width > width || actual_height > height {
        return Err(AppError::Invalid(
            "Poster exceeds requested dimensions".into(),
        ));
    }
    Ok(EncodedThumbnail {
        bytes: frame.stdout,
        value: json!({
            "ok": true,
            "width": actual_width,
            "height": actual_height,
            "source_width": source_width,
            "source_height": source_height,
            "poster": true
        }),
    })
}

fn render_in_child(
    path: &Path,
    output: &Path,
    width: u32,
    height: u32,
    expected_version: &str,
    input: Vec<u8>,
    cancelled: &AtomicBool,
) -> AppResult<Value> {
    let program = own_binary().map_err(|error| AppError::command(error.to_string()))?;
    let result = CommandSpec::new(program)
        .args([
            "_backend",
            "thumbnail-render",
            "--path",
            &path_text(path),
            "--target",
            &path_text(output),
            "--worker",
            "--width",
            &width.to_string(),
            "--height",
            &height.to_string(),
        ])
        .timeout(RENDER_TIMEOUT)
        .limits(RENDER_OUTPUT_BYTES, 16 * 1024)
        .stdin(input)
        .stop_on_output_limit()
        .run_cancellable(cancelled)?;
    if result.stdout_truncated {
        return Err(AppError::command(
            "thumbnail worker output exceeded its limit",
        ));
    }
    let stdout = String::from_utf8_lossy(&result.stdout);
    let mut value: Value = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line).ok())
        .unwrap_or(Value::Null);
    if result.status.success() && value["ok"] == true {
        let encoded = value["png_base64"]
            .as_str()
            .ok_or_else(|| AppError::command("thumbnail worker omitted its PNG"))?;
        let bytes = BASE64_STANDARD.decode(encoded).map_err(|error| {
            AppError::command(format!("thumbnail worker returned invalid PNG: {error}"))
        })?;
        if bytes.len() > CACHE_BYTES {
            return Err(AppError::command(
                "thumbnail worker PNG exceeded the cache limit",
            ));
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        if source_version(path).ok().as_deref() != Some(expected_version) {
            return Err(AppError::Invalid(
                "Source changed while creating the thumbnail".into(),
            ));
        }
        secure::write_private_atomic(output, &bytes)
            .map_err(|error| AppError::Invalid(error.to_string()))?;
        if let Some(object) = value.as_object_mut() {
            object.remove("png_base64");
        }
        return Ok(value);
    }
    let stderr = String::from_utf8_lossy(&result.stderr);
    let detail = stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.trim_start_matches("fileblade: ").to_string())
        .unwrap_or_else(|| match result.status.code() {
            Some(code) => format!("thumbnail helper exited with status {code}"),
            None => "thumbnail helper was stopped".to_string(),
        });
    Err(AppError::command(detail))
}

fn worker_input() -> AppResult<Vec<u8>> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((INPUT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| AppError::Invalid(error.to_string()))?;
    if bytes.len() > INPUT_BYTES {
        return Err(AppError::Invalid(
            "thumbnail worker input exceeds the source limit".into(),
        ));
    }
    Ok(bytes)
}

fn cached(output: &Path, max_width: u32, max_height: u32) -> Option<Value> {
    secure::ensure_private_directory(output.parent()?).ok()?;
    let bytes = secure::read_private_bounded(output, CACHE_BYTES).ok()??;
    if bytes.len() < 24 || !bytes.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR") {
        return None;
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    if width == 0 || height == 0 || width > max_width || height > max_height {
        return None;
    }
    Some(
        json!({"ok": true, "path": path_text(output), "width": width, "height": height, "cached": true}),
    )
}

fn bounded_size(width: u32, height: u32) -> (u32, u32) {
    (
        width.clamp(1, MAX_OUTPUT_EDGE),
        height.clamp(1, MAX_OUTPUT_EDGE),
    )
}

fn source_version(path: &Path) -> AppResult<String> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        AppError::Invalid(format!(
            "{} is missing or unavailable: {error}",
            path_text(path)
        ))
    })?;
    if !metadata.is_file() || metadata.len() > INPUT_BYTES as u64 {
        return Err(AppError::Invalid(
            "Thumbnail source must be a regular file within the input limit".into(),
        ));
    }
    Ok(format!(
        "{}:{}:{}:{}:{}:{}:{}",
        metadata.dev(),
        metadata.ino(),
        metadata.len(),
        metadata.mtime(),
        metadata.mtime_nsec(),
        metadata.ctime(),
        metadata.ctime_nsec()
    ))
}

fn confine_memory() -> AppResult<()> {
    use rustix::process::{Resource, Rlimit, setrlimit};
    let limit = Rlimit {
        current: Some(DECODE_MEMORY_BYTES),
        maximum: Some(DECODE_MEMORY_BYTES),
    };
    match setrlimit(Resource::As, limit) {
        Ok(()) => Ok(()),
        Err(rustix::io::Errno::PERM) | Err(rustix::io::Errno::INVAL) => Ok(()),
        Err(error) => Err(AppError::command(format!(
            "could not confine thumbnail memory: {error}"
        ))),
    }
}

fn digest(key: &str) -> String {
    let mut first: u64 = 0xcbf2_9ce4_8422_2325;
    let mut second: u64 = 0x84222325_cbf29ce4;
    for byte in key.bytes() {
        first ^= u64::from(byte);
        first = first.wrapping_mul(0x0000_0100_0000_01b3);
        second = second.rotate_left(5) ^ u64::from(byte);
        second = second.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    format!("{first:016x}{second:016x}")
}

fn failure(path: &str, error: &str) -> Value {
    json!({"ok": false, "path": path, "error": error})
}
