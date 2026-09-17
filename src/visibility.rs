use crate::filesystem::read_regular_prefix;
use crate::paths::xdg_home;
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

const CACHE_SIGNATURE: &[u8] = b"Signature: 8a477f597d28d172789f06886806bc55";
const CACHE_ENTRIES: usize = 4096;

pub(crate) struct Visibility {
    root: PathBuf,
    show_hidden: bool,
    private_roots: Vec<PathBuf>,
    caches: HashMap<PathBuf, bool>,
}

impl Visibility {
    pub(crate) fn new(root: &Path, show_hidden: bool) -> Self {
        let mut private_roots = Vec::new();
        for (variable, fallback) in [
            ("XDG_CONFIG_HOME", "~/.config"),
            ("XDG_STATE_HOME", "~/.local/state"),
        ] {
            let home = xdg_home(variable, fallback);
            private_roots.extend([
                home.join("omarchy/fileblade"),
                home.join("omarchy/filetree"),
            ]);
        }
        private_roots.push(xdg_home("XDG_CACHE_HOME", "~/.cache").join("fileblade"));
        private_roots.push(xdg_home("XDG_STATE_HOME", "~/.local/state").join("fileblade"));
        Self {
            root: root.to_path_buf(),
            show_hidden,
            private_roots,
            caches: HashMap::new(),
        }
    }

    pub(crate) fn entry(&self, path: &Path, is_dir: bool) -> bool {
        self.show_hidden
            || (!path
                .file_name()
                .is_some_and(|name| name.as_encoded_bytes().starts_with(b"."))
                && !self.private(path)
                && !(is_dir && cache_directory(path)))
    }

    pub(crate) fn path(&mut self, path: &Path) -> bool {
        if self.show_hidden {
            return true;
        }
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        if relative.components().any(|part| {
            matches!(part, Component::Normal(name) if name.as_encoded_bytes().starts_with(b"."))
        }) || self.private(path) {
            return false;
        }
        for ancestor in path.ancestors() {
            let cached = match self.caches.get(ancestor) {
                Some(cached) => *cached,
                None => {
                    let cached = cache_directory(ancestor);
                    if self.caches.len() < CACHE_ENTRIES {
                        self.caches.insert(ancestor.to_path_buf(), cached);
                    }
                    cached
                }
            };
            if cached {
                return false;
            }
        }
        true
    }

    fn private(&self, path: &Path) -> bool {
        self.private_roots.iter().any(|root| path.starts_with(root))
    }
}

fn cache_directory(path: &Path) -> bool {
    read_regular_prefix(&path.join("CACHEDIR.TAG"), CACHE_SIGNATURE.len())
        .is_ok_and(|bytes| bytes == CACHE_SIGNATURE)
}
