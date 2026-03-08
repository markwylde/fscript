//! Shared test helpers for workspace integration and snapshot tests.

use std::{
    fs,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use camino::{Utf8Path, Utf8PathBuf};

static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Returns the repository root.
#[must_use]
pub fn repo_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Returns the examples directory.
#[must_use]
pub fn examples_dir() -> Utf8PathBuf {
    repo_root().join("examples")
}

/// Returns every `.fs` example file, including nested example projects.
#[must_use]
pub fn example_source_paths() -> Vec<Utf8PathBuf> {
    let mut paths = Vec::new();
    collect_example_paths(&examples_dir(), &mut paths);
    paths.sort();
    paths
}

fn collect_example_paths(root: &Utf8Path, paths: &mut Vec<Utf8PathBuf>) {
    let entries = fs::read_dir(root).expect("example directories should be readable");
    for entry in entries {
        let entry = entry.expect("example entries should be readable");
        let path = Utf8PathBuf::from_path_buf(entry.path()).expect("example paths should be utf-8");
        if path.is_dir() {
            collect_example_paths(&path, paths);
        } else if path.extension() == Some("fs") {
            paths.push(path);
        }
    }
}

/// Returns a single example path relative to the shared examples directory.
#[must_use]
pub fn example_path(relative: &str) -> Utf8PathBuf {
    examples_dir().join(relative)
}

/// Writes a single temporary `.fs` file and returns its path.
#[must_use]
pub fn write_temp_file(name: &str, contents: &str) -> Utf8PathBuf {
    let root = temp_file_path(&format!("fscript-{name}"));
    let parent = root
        .parent()
        .expect("temp file paths should have a parent directory");
    fs::create_dir_all(parent).expect("temp parents should be creatable");
    fs::write(&root, contents).expect("temp source should be writable");
    root
}

/// Writes a temporary project tree and returns the project root.
#[must_use]
pub fn write_temp_project(name: &str, files: &[(&str, &str)]) -> Utf8PathBuf {
    let root = temp_dir_path(&format!("fscript-project-{name}"));
    write_project_tree(&root, files);
    root
}

/// Canonicalizes a UTF-8 path for assertions and snapshots.
#[must_use]
pub fn canonicalize_utf8(path: &Utf8Path) -> Utf8PathBuf {
    Utf8PathBuf::from_path_buf(fs::canonicalize(path).expect("paths should be canonicalizable"))
        .expect("canonical paths should stay utf-8")
}

/// Normalizes absolute paths and platform separators for stable snapshots.
#[must_use]
pub fn normalize_snapshot(text: &str) -> String {
    let text = strip_ansi(text).replace('\\', "/");
    let repo = repo_root().as_str().replace('\\', "/");
    let temp = std::env::temp_dir()
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_owned();

    let text = text
        .replace(&repo, "<repo>")
        .replace(&temp, "<tmp>")
        .replace("/private<tmp>", "<tmp>");
    text.lines()
        .map(|line| {
            line.split('/')
                .map(normalize_temp_component)
                .collect::<Vec<_>>()
                .join("/")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_ansi(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            let _ = chars.next();
            for code in chars.by_ref() {
                if code.is_ascii_alphabetic() {
                    break;
                }
            }
            continue;
        }

        result.push(ch);
    }

    result
}

fn temp_file_path(prefix: &str) -> Utf8PathBuf {
    temp_dir_path(prefix).with_extension("fs")
}

fn temp_dir_path(prefix: &str) -> Utf8PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);

    Utf8PathBuf::from_path_buf(std::env::temp_dir().join(format!("{prefix}-{timestamp}-{counter}")))
        .expect("temp paths should be utf-8")
}

fn write_project_tree(root: &Utf8Path, files: &[(&str, &str)]) {
    if root.exists() {
        fs::remove_dir_all(root).expect("old temp project should be removable");
    }
    fs::create_dir_all(root).expect("temp project should be creatable");

    for (relative, contents) in files {
        let path = root.join(relative);
        let parent = path
            .parent()
            .expect("temp project file paths should have a parent directory");
        fs::create_dir_all(parent).expect("temp project parents should be creatable");
        fs::write(&path, contents).expect("temp project files should be writable");
    }
}

fn normalize_temp_component(component: &str) -> String {
    if component == "fscript-bootstrap-compile" {
        return "fscript-compile".to_owned();
    }

    let (base, suffix) = match component.split_once(':') {
        Some((base, suffix)) => (base, format!(":{suffix}")),
        None => (component, String::new()),
    };

    let Some(core_start) = base.find(|ch: char| ch.is_ascii_alphanumeric()) else {
        return component.to_owned();
    };
    let core_end = base
        .rfind(|ch: char| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
        .expect("an alphanumeric core start guarantees a matching core end");
    let core_end = core_end + 1;
    let prefix = &base[..core_start];
    let core = &base[core_start..core_end];
    let postfix = &base[core_end..];

    let (stem, extension) = match core.rsplit_once('.') {
        Some((stem, extension)) => (stem, format!(".{extension}")),
        None => (core, String::new()),
    };

    let parts = stem.split('-').collect::<Vec<_>>();
    let has_generated_suffix = parts.len() >= 3
        && parts
            .iter()
            .rev()
            .take(2)
            .all(|part| part.chars().all(|ch| ch.is_ascii_digit()))
        && parts
            .first()
            .is_some_and(|part| part.starts_with("fscript"));

    if !has_generated_suffix {
        return component.to_owned();
    }

    format!(
        "{prefix}{}{}{postfix}{}",
        parts[..parts.len() - 2].join("-"),
        extension,
        suffix
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repo_and_example_helpers_point_into_the_workspace() {
        let repo = repo_root();
        let examples = examples_dir();

        assert!(repo.join("Cargo.toml").exists());
        assert_eq!(examples, repo.join("examples"));
        assert!(example_path("hello_world.fs").ends_with("examples/hello_world.fs"));
    }

    #[test]
    fn example_source_paths_are_sorted_and_include_nested_examples() {
        let paths = example_source_paths();

        assert!(!paths.is_empty());
        assert_eq!(paths, {
            let mut sorted = paths.clone();
            sorted.sort();
            sorted
        });
        assert!(
            paths
                .iter()
                .any(|path| path.ends_with("examples/hello_world.fs"))
        );
        assert!(
            paths
                .iter()
                .any(|path| path.ends_with("examples/http_hello_server/main.fs"))
        );
    }

    #[test]
    fn temp_file_and_project_helpers_write_expected_contents() {
        let file_path = write_temp_file("support-test", "value = 1");
        assert_eq!(
            fs::read_to_string(&file_path).expect("temp file should be readable"),
            "value = 1"
        );

        let project_name = "support-project";
        let first_root = write_temp_project(
            project_name,
            &[("main.fs", "first"), ("nested/lib.fs", "nested")],
        );
        assert_eq!(
            fs::read_to_string(first_root.join("main.fs")).expect("main file should be readable"),
            "first"
        );
        assert_eq!(
            fs::read_to_string(first_root.join("nested/lib.fs"))
                .expect("nested file should be readable"),
            "nested"
        );

        let second_root = write_temp_project(project_name, &[("main.fs", "second")]);
        assert_ne!(first_root, second_root);
        assert_eq!(
            fs::read_to_string(second_root.join("main.fs"))
                .expect("rewritten main file should be readable"),
            "second"
        );
    }

    #[test]
    fn write_project_tree_replaces_existing_content_when_root_already_exists() {
        let root = temp_dir_path("fscript-project-rewrite");
        write_project_tree(&root, &[("nested/old.fs", "old")]);
        assert!(root.join("nested/old.fs").exists());

        write_project_tree(&root, &[("main.fs", "new")]);

        assert_eq!(
            fs::read_to_string(root.join("main.fs")).expect("rewritten main file should exist"),
            "new"
        );
        assert!(!root.join("nested/old.fs").exists());
    }

    #[test]
    fn canonicalize_and_normalize_snapshot_stabilize_paths_and_ansi_sequences() {
        let generated = write_temp_file("normalize-snapshot", "value = 1");
        let canonical = canonicalize_utf8(&generated);
        assert!(canonical.is_absolute());

        let raw = format!(
            "\u{1b}[31m{}\u{1b}[0m\n{}\n{}\n{}\n{}:12\n---",
            repo_root().join("specs/IMPLEMENTATION_PLAN.md"),
            generated,
            std::env::temp_dir()
                .join("ordinary-file.txt")
                .to_string_lossy(),
            "/private<tmp>/fscript-fake-123-456.fs",
            generated
        );

        let normalized = normalize_snapshot(&raw);

        assert!(normalized.contains("<repo>/specs/IMPLEMENTATION_PLAN.md"));
        assert!(normalized.contains("<tmp>/fscript-normalize-snapshot.fs"));
        assert!(normalized.contains("<tmp>/ordinary-file.txt"));
        assert!(normalized.contains("<tmp>/fscript-fake.fs"));
        assert!(normalized.contains("<tmp>/fscript-normalize-snapshot.fs:12"));
        assert!(normalized.contains("---"));
        assert!(!normalized.contains('\u{1b}'));
    }

    #[test]
    fn normalize_temp_component_handles_bootstrap_generated_and_plain_segments() {
        assert_eq!(
            normalize_temp_component("fscript-bootstrap-compile"),
            "fscript-compile"
        );
        assert_eq!(
            normalize_temp_component("fscript-task-123-456.fs:7"),
            "fscript-task.fs:7"
        );
        assert_eq!(normalize_temp_component("---"), "---");
        assert_eq!(
            normalize_temp_component("ordinary-file.txt"),
            "ordinary-file.txt"
        );
    }
}
