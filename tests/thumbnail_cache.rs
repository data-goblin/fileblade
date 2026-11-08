use fileblade::thumbnail;
use fileblade::thumbnail_cache::{CAPACITY_BYTES, Eviction, evict};
use image::{ImageBuffer, Rgba};
use std::fs::{File, FileTimes};
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

fn cached(
    directory: &std::path::Path,
    index: u64,
    bytes: usize,
    age: Duration,
) -> std::path::PathBuf {
    let path = directory.join(format!("{index:032x}.png"));
    std::fs::write(&path, vec![0u8; bytes]).unwrap();
    let moment = SystemTime::now() - age;
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_accessed(moment).set_modified(moment))
        .unwrap();
    path
}

#[test]
fn the_thumbnail_cache_drops_the_least_recently_used_entries_past_its_cap() {
    let temporary = tempdir().unwrap();
    let directory = temporary.path();
    let day = Duration::from_secs(86_400);
    let oldest = cached(directory, 1, 400, day * 30);
    let older = cached(directory, 2, 400, day * 20);
    let recent = cached(directory, 3, 400, day);
    let newest = cached(directory, 4, 400, Duration::ZERO);
    std::fs::write(directory.join("keep-me.txt"), vec![0u8; 4000]).unwrap();
    let abandoned = directory.join(".fileblade-0123.tmp");
    std::fs::write(&abandoned, b"partial").unwrap();
    File::options()
        .write(true)
        .open(&abandoned)
        .unwrap()
        .set_times(FileTimes::new().set_modified(SystemTime::now() - day * 2))
        .unwrap();

    let untouched = evict(directory, 2000, 1000).unwrap();
    assert_eq!(untouched.examined, 4);
    assert_eq!(untouched.bytes_after, 1600);
    assert!(oldest.exists());

    let evicted = evict(directory, 1500, 800).unwrap();
    assert_eq!(
        evicted,
        Eviction {
            examined: 4,
            removed: 2,
            bytes_before: 1600,
            bytes_after: 800
        }
    );
    assert!(!oldest.exists() && !older.exists());
    assert!(recent.exists() && newest.exists());
    assert!(directory.join("keep-me.txt").exists());
    assert!(!abandoned.exists());
}

#[test]
fn rendering_outside_the_thumbnail_cache_leaves_that_directory_alone() {
    let temporary = tempdir().unwrap();
    let directory = temporary.path().join("pictures");
    std::fs::create_dir(&directory).unwrap();
    let neighbour = directory.join(format!("{:032x}.png", 7));
    File::create(&neighbour)
        .unwrap()
        .set_len(CAPACITY_BYTES + 1024 * 1024)
        .unwrap();
    let source = temporary.path().join("source.png");
    ImageBuffer::from_pixel(8, 8, Rgba([10u8, 20, 30, 255]))
        .save(&source)
        .unwrap();
    let output = directory.join("rendered.png");
    let value = thumbnail::render(source.to_str().unwrap(), &output, 64, 64).unwrap();
    assert_eq!(value["ok"], true, "{value}");
    std::thread::sleep(Duration::from_secs(2));
    assert!(neighbour.exists());
    assert!(output.exists());
}
