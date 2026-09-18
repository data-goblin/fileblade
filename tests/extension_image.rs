use fileblade::extension_template::image;
use regex::Regex;
use std::fs;
use std::process::Command;

fn fileblade() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fileblade"))
}

fn stdout_of(command: &mut Command) -> String {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn the_glyph_table_covers_the_outlined_alphabet() {
    let expected: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 -.&+/"
        .chars()
        .collect();
    let mut keys: Vec<char> = image::GLYPHS.iter().map(|(key, _)| *key).collect();
    keys.sort_unstable();
    let mut wanted = expected.clone();
    wanted.sort_unstable();
    assert_eq!(keys, wanted);
    let shape = Regex::new(r"^([MLHVQCZ][-0-9. ]*)+$").unwrap();
    for (key, path) in image::GLYPHS {
        assert!(
            path.is_empty() || shape.is_match(path),
            "glyph {key:?} is not an absolute SVG path"
        );
    }
    assert_eq!(image::glyph(' '), Some(""));
    assert_eq!(image::MAX_SUBTITLE, 23);
}

#[test]
fn geometry_matches_the_published_banners() {
    assert_eq!(
        image::subtitle_text("Agent Skills"),
        "AGENT SKILLS EXTENSION"
    );
    assert_eq!(image::subtitle_text("  git  "), "GIT EXTENSION");
    assert_eq!(image::subtitle_x("GIT EXTENSION"), 506.39);
    assert_eq!(image::subtitle_x("AGENT SKILLS EXTENSION"), 290.39);
    assert_eq!(
        image::shifted("M1.0 2.0 3.0 4.0H5.0V6.0Q7.0 8.0 9.0 10.0Z", 10.0),
        "M11.0 2.0 13.0 4.0H15.0V6.0Q17.0 8.0 19.0 10.0Z"
    );
    assert_eq!(image::number(-0.0), "0.0");
    assert_eq!(image::number(70.65), "70.65");
    assert_eq!(image::number(3.0), "3.0");
    let single = image::subtitle_path("A").unwrap();
    let first = image::subtitle_path("AA").unwrap();
    assert!(first.starts_with(&format!("{single} ")));
    let leading: f64 = Regex::new(r"M(-?[0-9.]+)")
        .unwrap()
        .captures(&single)
        .unwrap()[1]
        .parse()
        .unwrap();
    assert!(first.contains(&format!("M{}", image::number(60.0 + leading))));
}

#[test]
fn the_banner_and_the_host_logo_carry_their_masks_and_colours() {
    let svg = image::banner_svg("Weather", None).unwrap();
    assert!(svg.starts_with(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 960 272\" width=\"960\" height=\"272\""
    ));
    assert!(svg.contains("aria-label=\"FileBlade weather extension\""));
    assert_eq!(
        svg.matches("id=\"fileblade-extension-logo-mask\"").count(),
        1
    );
    assert!(
        !svg.replace("fileblade-extension-logo-mask", "")
            .contains("fileblade-logo-mask")
    );
    assert!(svg.contains("translate(410.39,243.20) scale(0.4)"));
    assert!(svg.contains("fill=\"#e0af68\""));
    assert_eq!(svg.matches("fill=\"#7aa2f7\"").count(), 3);
    assert!(svg.ends_with("</svg>\n"));
    let custom = image::banner_svg("Weather", Some("RAIN & SUN")).unwrap();
    assert!(custom.contains("translate(578.39,243.20)"));
    let logo = image::host_logo_svg();
    assert!(logo.contains("viewBox=\"0 0 960 240\""));
    assert!(logo.contains("id=\"fileblade-logo-mask\""));
    assert!(!logo.contains("#e0af68"));
    let label = image::banner_svg("We \"ather\" <&>", Some("WEATHER EXTENSION")).unwrap();
    assert!(
        label.contains("aria-label=\"FileBlade we &quot;ather&quot; &lt;&amp;&gt; extension\"")
    );
}

#[test]
fn unsupported_subtitles_are_refused() {
    for text in ["ÜBER EXTENSION", "", "   ", &"A".repeat(24)] {
        assert!(
            image::subtitle_path(text).is_err(),
            "{text:?} must be refused"
        );
    }
}

#[test]
fn the_verb_writes_the_banner_and_reports_its_refusals() {
    let temporary = tempfile::tempdir().unwrap();
    let manifest = temporary.path().join("manifest.json");
    fs::write(&manifest, "{\"name\":\"FileBlade Agent Skills\"}").unwrap();
    let out = temporary.path().join("assets");
    let printed = stdout_of(
        fileblade()
            .args(["extension", "image", "--manifest"])
            .arg(&manifest)
            .arg("--out")
            .arg(&out),
    );
    assert_eq!(
        printed.trim_end(),
        out.join("fileblade-extension-logo.svg").to_str().unwrap()
    );
    let svg = fs::read_to_string(out.join("fileblade-extension-logo.svg")).unwrap();
    assert!(svg.contains("translate(290.39,243.20)"));

    let missing = fileblade()
        .args(["extension", "image", "--manifest"])
        .arg(temporary.path().join("none.json"))
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("cannot read"));

    let stdout = stdout_of(fileblade().args(["extension", "image", "--name", "Git", "--stdout"]));
    assert!(stdout.starts_with("<svg"));
    assert!(stdout.contains("translate(506.39,243.20)"));

    let refused = fileblade()
        .args(["extension", "image", "--name", "Über", "--stdout"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("unsupported characters"));
}
