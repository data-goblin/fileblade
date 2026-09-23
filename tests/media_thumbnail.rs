use image::{ImageBuffer, Rgb, Rgba};
use serde_json::Value;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn thumbnail(path: &Path, cache: &Path) -> Value {
    let result = Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .env("XDG_CACHE_HOME", cache)
        .args([
            "_backend",
            "thumbnail",
            "--path",
            path.to_str().unwrap(),
            "--key",
            "unchanged-client-stamp",
            "--width",
            "64",
            "--height",
            "64",
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn cache_tracks_replacement_edits_deletion_and_symlinks_without_a_new_client_stamp() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("photo.png");
    let cache = dir.path().join("cache");
    ImageBuffer::from_pixel(20, 10, Rgba([255u8, 0, 0, 80]))
        .save(&path)
        .unwrap();
    let first = thumbnail(&path, &cache);
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(thumbnail(&path, &cache)["cached"], true);
    ImageBuffer::from_pixel(10, 20, Rgba([0u8, 255, 0, 80]))
        .save(&path)
        .unwrap();
    let edited = thumbnail(&path, &cache);
    assert_eq!(edited["ok"], true, "{edited}");
    assert_ne!(edited["path"], first["path"]);
    assert_eq!(edited["height"], 20);
    let old = dir.path().join("old.png");
    fs::rename(&path, &old).unwrap();
    fs::copy(&old, &path).unwrap();
    let replaced = thumbnail(&path, &cache);
    assert_eq!(replaced["ok"], true, "{replaced}");
    assert_ne!(replaced["path"], edited["path"]);
    let decoded = image::open(replaced["path"].as_str().unwrap())
        .unwrap()
        .to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0, [0, 255, 0, 80]);
    fs::remove_file(&path).unwrap();
    assert_eq!(thumbnail(&path, &cache)["ok"], false);
    symlink(&old, &path).unwrap();
    assert_eq!(thumbnail(&path, &cache)["ok"], false);
}

#[test]
fn jpeg_orientation_is_applied_before_scaling_the_static_preview() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rotated.jpg");
    ImageBuffer::from_fn(20, 10, |x, _| {
        if x < 10 {
            Rgb([255u8, 0, 0])
        } else {
            Rgb([0u8, 255, 0])
        }
    })
    .save(&path)
    .unwrap();
    let jpeg = fs::read(&path).unwrap();
    let exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
    let mut oriented = jpeg[..2].to_vec();
    oriented.extend_from_slice(&[0xff, 0xe1]);
    oriented.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    oriented.extend_from_slice(exif);
    oriented.extend_from_slice(&jpeg[2..]);
    fs::write(&path, oriented).unwrap();
    let result = thumbnail(&path, &dir.path().join("cache"));
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        (result["width"].as_u64(), result["height"].as_u64()),
        (Some(10), Some(20))
    );
    let preview = image::open(result["path"].as_str().unwrap())
        .unwrap()
        .to_rgb8();
    assert!(preview.get_pixel(5, 2)[0] > 220);
    assert!(preview.get_pixel(5, 17)[1] > 220);
}

#[test]
fn ordinary_mp4_with_metadata_after_packets_produces_a_real_poster() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("ordinary.mp4");
    let output = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=640x400:rate=20",
            "-t",
            "2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = fs::read(&path).unwrap();
    assert!(source.len() > 32768);
    let mdat = source
        .windows(4)
        .position(|value| value == b"mdat")
        .unwrap();
    let moov = source
        .windows(4)
        .position(|value| value == b"moov")
        .unwrap();
    assert!(moov > mdat);
    let result = thumbnail(&path, &dir.path().join("cache"));
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(
        (result["width"].as_u64(), result["height"].as_u64()),
        (Some(64), Some(40))
    );
    let preview = image::open(result["path"].as_str().unwrap())
        .unwrap()
        .to_rgb8();
    assert!(
        preview
            .pixels()
            .any(|pixel| pixel != preview.get_pixel(0, 0))
    );
}
