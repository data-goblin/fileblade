use fileblade::core_modules::frontmatter::{block, frontmatter_field, value};
use fileblade::core_modules::glob::{
    MAX_GLOB_MATCHES, bounded_glob, code_points, fnmatch_bytes, fnmatch_case,
};
use fileblade::core_modules::watch::WatchPlan;
use fileblade::project::{AGENT_SEARCH, SKILLS_SEARCH, marker_root};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

#[test]
fn a_frontmatter_block_tolerates_trailing_spaces_and_carriage_returns() {
    assert_eq!(block("--- \t\nname: one\n---\nbody"), "name: one");
    assert_eq!(block("---\r\nname: one\r\n---\r\nbody"), "name: one\r");
    assert_eq!(block("name: one\n---\n"), "");
    assert_eq!(block("---\nname: one\n"), "");
}

#[test]
fn single_line_scalars_strip_quotes_and_comments() {
    assert_eq!(value("---\nname: \"quoted\"\n---\n", "name"), "quoted");
    assert_eq!(value("---\nname: 'quoted'\n---\n", "name"), "quoted");
    assert_eq!(value("---\nname: plain # note\n---\n", "name"), "plain");
    assert_eq!(value("---\nname: # note\n---\n", "name"), "");
    assert_eq!(value("---\nname:\tvalue\n---\n", "name"), "value");
    assert_eq!(value("---\nnamespace: other\n---\n", "name"), "");
    assert_eq!(value("---\nname: one\n---\n", "missing"), "");
}

#[test]
fn block_scalars_fold_their_indented_lines() {
    for marker in [">", "|", ">-", "|-", ">+", "|+"] {
        let text =
            format!("---\ndescription: {marker}\n  first line\n  second line\nother: x\n---\n");
        assert_eq!(
            value(&text, "description"),
            "first line second line",
            "{marker}"
        );
    }
}

#[test]
fn the_simple_frontmatter_field_reader_stops_at_the_terminator() {
    assert_eq!(
        frontmatter_field("---\nname: one\n---\nbody", "name"),
        "one"
    );
    assert_eq!(frontmatter_field("---\n---\nname: one\n", "name"), "");
    assert_eq!(frontmatter_field("name: one\n", "name"), "");
    assert_eq!(
        frontmatter_field("---\nname: \"quoted\"\r\n---\n", "name"),
        "quoted"
    );
}

#[test]
fn fnmatch_semantics_match_python_fnmatchcase() {
    assert!(fnmatch_case("AGENTS.md", "*.md"));
    assert!(!fnmatch_case("AGENTS.MD", "*.md"));
    assert!(fnmatch_case("a1", "a[0-9]"));
    assert!(!fnmatch_case("ab", "a[0-9]"));
    assert!(fnmatch_case("ab", "a[!0-9]"));
    assert!(fnmatch_case("abc", "a?c"));
    assert!(fnmatch_case("anything", "*"));
    assert!(fnmatch_case("a[b", "a[b"));
}

#[test]
fn the_bounded_glob_walks_double_stars_and_stays_inside_its_root() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    fs::create_dir_all(root.join("rules/deep/deeper")).unwrap();
    fs::write(root.join("rules/one.md"), "one").unwrap();
    fs::write(root.join("rules/deep/two.md"), "two").unwrap();
    fs::write(root.join("rules/deep/deeper/three.md"), "three").unwrap();
    fs::write(root.join("rules/skip.txt"), "skip").unwrap();

    let mut plan = WatchPlan::new();
    let found = bounded_glob(&mut plan, &root, "rules/**/*.md");
    let names: Vec<String> = found
        .iter()
        .map(|path| {
            path.strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(
        names,
        vec![
            "rules/deep/deeper/three.md".to_string(),
            "rules/deep/two.md".to_string(),
            "rules/one.md".to_string(),
        ]
    );
    assert!(!plan.paths().is_empty());

    for refused in [
        "",
        "/absolute/*.md",
        "~/x.md",
        "../escape/*.md",
        "a**b/*.md",
    ] {
        assert!(
            bounded_glob(&mut WatchPlan::new(), &root, refused).is_empty(),
            "{refused}"
        );
    }
}

#[test]
fn the_bounded_glob_refuses_to_leave_its_root_through_a_symlink() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let inside = root.join("inside");
    let outside = root.join("outside");
    fs::create_dir_all(&inside).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret.md"), "secret").unwrap();
    std::os::unix::fs::symlink(&outside, inside.join("escape")).unwrap();
    let mut plan = WatchPlan::new();
    let found = bounded_glob(&mut plan, &inside, "escape/*.md");
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_bounded_glob_stops_at_its_match_limit() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    for index in 0..(MAX_GLOB_MATCHES + 16) {
        fs::write(root.join(format!("rule-{index}.md")), "body").unwrap();
    }
    let mut plan = WatchPlan::new();
    assert_eq!(
        bounded_glob(&mut plan, &root, "*.md").len(),
        MAX_GLOB_MATCHES
    );
}

#[test]
fn project_marker_searches_are_parameterised_per_module() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let repository = root.join("work/repo");
    let nested = repository.join("a/b/c");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(repository.join(".claude")).unwrap();

    assert_eq!(marker_root(&nested, SKILLS_SEARCH), None);
    assert_eq!(
        marker_root(&nested, AGENT_SEARCH),
        Some((repository.clone(), ".claude".to_string()))
    );

    fs::create_dir_all(repository.join(".git")).unwrap();
    assert_eq!(
        marker_root(&nested, SKILLS_SEARCH),
        Some((repository.clone(), ".git".to_string()))
    );
    assert_eq!(
        marker_root(&nested, AGENT_SEARCH),
        Some((repository, ".git".to_string()))
    );
}

#[test]
fn the_bounded_glob_follows_symlinked_matches_like_the_python_scandir_walk() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let source = root.join("source");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("nested/AGENTS.md"), "body").unwrap();
    let tree = root.join("tree");
    fs::create_dir_all(&tree).unwrap();
    std::os::unix::fs::symlink(source.join("nested"), tree.join("linked")).unwrap();
    std::os::unix::fs::symlink(source.join("nested/AGENTS.md"), tree.join("AGENTS.md")).unwrap();

    let mut plan = WatchPlan::new();
    let matches = bounded_glob(&mut plan, &root, "tree/**/AGENTS.md");
    assert!(matches.contains(&tree.join("AGENTS.md")));
    assert!(matches.contains(&tree.join("linked/AGENTS.md")));
}

#[test]
fn marker_root_walks_the_logical_ancestor_chain_through_a_symlink() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let repository = root.join("repo");
    fs::create_dir_all(repository.join("a/b")).unwrap();
    fs::create_dir_all(repository.join(".git")).unwrap();
    let link = root.join("link");
    std::os::unix::fs::symlink(&repository, &link).unwrap();

    assert_eq!(
        marker_root(&link.join("a/b"), SKILLS_SEARCH),
        Some((link, ".git".to_string()))
    );
}

#[test]
fn the_bounded_glob_carries_names_that_are_not_valid_utf8() {
    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let repository = root.join(OsStr::from_bytes(b"repo-\xff"));
    let rules = repository.join("rules");
    fs::create_dir_all(&rules).unwrap();
    let odd = rules.join(OsStr::from_bytes(b"\xfe.md"));
    fs::write(&odd, "body").unwrap();
    fs::write(rules.join("plain.md"), "body").unwrap();

    let mut plan = WatchPlan::new();
    let found = bounded_glob(&mut plan, &repository, "rules/*.md");
    assert!(found.contains(&odd), "{found:?}");
    assert!(found.contains(&rules.join("plain.md")), "{found:?}");
}

#[test]
fn name_matching_treats_undecodable_bytes_as_single_code_points() {
    assert!(fnmatch_bytes(b"\xff.md", "*.md"));
    assert!(fnmatch_bytes(b"\xff.md", "?.md"));
    assert!(!fnmatch_bytes(b"\xff\xfe.md", "?.md"));
    assert!(fnmatch_bytes("é.md".as_bytes(), "?.md"));
    assert_eq!(code_points(b"a\xffb"), vec![0x61, 0xdcff, 0x62]);
    assert_eq!(code_points("é".as_bytes()), vec![0xe9]);
}

#[test]
fn marker_root_refuses_an_empty_anchor_and_keeps_raw_path_bytes() {
    assert_eq!(marker_root(Path::new(""), AGENT_SEARCH), None);
    assert_eq!(marker_root(Path::new(""), SKILLS_SEARCH), None);

    let scratch = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let repository = root.join(OsStr::from_bytes(b"repo-\xff"));
    let nested = repository.join("a/b");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(repository.join(".git")).unwrap();
    assert_eq!(
        marker_root(&nested, SKILLS_SEARCH),
        Some((repository, ".git".to_string()))
    );
}
