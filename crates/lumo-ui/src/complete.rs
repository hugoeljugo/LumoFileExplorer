//! Address bar autocompletion: recent locations plus matching subfolders.

use std::path::{Path, PathBuf};

use lumo_core::{EntryKind, list_entries, natural_cmp};

/// Remembers the last folder it listed, so typing within one folder doesn't
/// re-read it on every keystroke.
#[derive(Debug, Default)]
pub struct Completer {
    cached: Option<(PathBuf, Vec<String>)>,
}

impl Completer {
    /// Suggestions for `input`: recent locations starting with it, then
    /// subfolders of the typed parent folder matching the last segment
    /// (case-insensitive). Relative input resolves against `base`.
    pub fn suggest(
        &mut self,
        input: &str,
        base: &Path,
        recent: &[PathBuf],
        limit: usize,
    ) -> Vec<String> {
        let input = input.trim().trim_matches('"');
        let needle = input.to_lowercase();
        let mut out: Vec<String> = recent
            .iter()
            .map(|p| p.display().to_string())
            .filter(|p| p.to_lowercase().starts_with(&needle) && p.len() > input.len())
            .collect();

        if !input.is_empty() {
            let (typed_dir, prefix) = split_input(input);
            let dir = base.join(typed_dir);
            let prefix = prefix.to_lowercase();
            for name in self.subfolders(&dir) {
                if name.to_lowercase().starts_with(&prefix) {
                    let suggestion = format!("{typed_dir}{name}");
                    if !out.contains(&suggestion) {
                        out.push(suggestion);
                    }
                }
            }
        }
        out.truncate(limit);
        out
    }

    fn subfolders(&mut self, dir: &Path) -> &[String] {
        if self.cached.as_ref().is_none_or(|(cached, _)| cached != dir) {
            // Reading on the UI thread: fine for local disks; slow network
            // folders may stutter here until search moves to a worker (Phase 5).
            let mut names: Vec<String> = list_entries(dir)
                .map(|entries| {
                    entries
                        .into_iter()
                        .filter(|e| {
                            e.kind == EntryKind::Directory
                                || (e.kind == EntryKind::Symlink && dir.join(&e.name).is_dir())
                        })
                        .map(|e| e.name.to_string_lossy().into_owned())
                        .collect()
                })
                .unwrap_or_default();
            names.sort_by(|a, b| natural_cmp(a, b));
            self.cached = Some((dir.to_path_buf(), names));
        }
        self.cached.as_ref().map_or(&[], |(_, names)| names)
    }
}

/// Splits typed text into the folder to list and the partial name after the
/// last separator: `C:\Us` → (`C:\`, `Us`); `src\` → (`src\`, ``).
fn split_input(input: &str) -> (&str, &str) {
    match input.rfind(['/', '\\']) {
        Some(i) => (&input[..=i], &input[i + 1..]),
        None => ("", input),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{File, create_dir};

    #[test]
    fn splits_at_last_separator() {
        assert_eq!(split_input("C:\\Users\\Hu"), ("C:\\Users\\", "Hu"));
        assert_eq!(split_input("a/b/"), ("a/b/", ""));
        assert_eq!(split_input("abc"), ("", "abc"));
    }

    #[test]
    fn suggests_matching_subfolders_case_insensitively() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["Documents", "Downloads", "desktop", "Music"] {
            create_dir(dir.path().join(d)).unwrap();
        }
        File::create(dir.path().join("Dfile.txt")).unwrap();
        let typed = format!("{}{}d", dir.path().display(), std::path::MAIN_SEPARATOR);

        let got = Completer::default().suggest(&typed, Path::new("."), &[], 10);

        let root = format!("{}{}", dir.path().display(), std::path::MAIN_SEPARATOR);
        assert_eq!(
            got,
            [
                format!("{root}desktop"),
                format!("{root}Documents"),
                format!("{root}Downloads")
            ]
        );
    }

    #[test]
    fn relative_input_resolves_against_base() {
        let dir = tempfile::tempdir().unwrap();
        create_dir(dir.path().join("src")).unwrap();

        let got = Completer::default().suggest("s", dir.path(), &[], 10);

        assert_eq!(got, ["src"]);
    }

    #[test]
    fn recent_locations_come_first_and_respect_limit() {
        let dir = tempfile::tempdir().unwrap();
        for d in ["aa", "ab", "ac"] {
            create_dir(dir.path().join(d)).unwrap();
        }
        let recent = vec![dir.path().join("zz-recent")];
        let typed = dir.path().display().to_string();

        let got = Completer::default().suggest(&typed, Path::new("."), &recent, 2);

        assert_eq!(got.len(), 2);
        assert_eq!(got[0], recent[0].display().to_string());
    }

    #[test]
    fn empty_input_lists_recent_locations() {
        let recent = vec![PathBuf::from("/x"), PathBuf::from("/y")];
        let got = Completer::default().suggest("", Path::new("."), &recent, 10);
        assert_eq!(got, ["/x", "/y"]);
    }
}
