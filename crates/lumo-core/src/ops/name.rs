//! File-name rules and Explorer-style names for conflicts.

use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NameError {
    #[error("a name can't be empty")]
    Empty,
    #[error("a name can't contain {0:?}")]
    InvalidChar(char),
    #[error("\"{0}\" is reserved by Windows")]
    Reserved(String),
    #[error("a name can't end with a dot or a space")]
    TrailingDotOrSpace,
    #[error("that name is too long")]
    TooLong,
}

/// Checks a single file or folder name (not a path). Windows' rules are
/// enforced on Windows; elsewhere only `/` and NUL are invalid.
pub fn validate_name(name: &str) -> Result<(), NameError> {
    if name.is_empty() || name == "." || name == ".." {
        return Err(NameError::Empty);
    }
    if name.len() > 255 {
        return Err(NameError::TooLong);
    }
    if let Some(c) = name.chars().find(|&c| c == '/' || c == '\0') {
        return Err(NameError::InvalidChar(c));
    }
    if cfg!(windows) {
        if let Some(c) = name
            .chars()
            .find(|&c| matches!(c, '<' | '>' | ':' | '"' | '\\' | '|' | '?' | '*') || c < ' ')
        {
            return Err(NameError::InvalidChar(c));
        }
        if name.ends_with(['.', ' ']) {
            return Err(NameError::TrailingDotOrSpace);
        }
        let stem = name.split('.').next().unwrap_or(name).trim_end();
        let reserved = ["CON", "PRN", "AUX", "NUL"]
            .iter()
            .any(|r| stem.eq_ignore_ascii_case(r))
            || ["COM", "LPT"].iter().any(|prefix| {
                stem.len() == 4
                    && stem[..3].eq_ignore_ascii_case(prefix)
                    && stem.as_bytes()[3].is_ascii_digit()
                    && stem.as_bytes()[3] != b'0'
            });
        if reserved {
            return Err(NameError::Reserved(stem.to_owned()));
        }
    }
    Ok(())
}

/// `desired` if free in `dir`, else "name (2).ext", "name (3).ext", …
pub fn unique_name(dir: &Path, desired: &str) -> String {
    first_free(dir, desired, |stem, ext, n| format!("{stem} ({n}){ext}"))
}

/// Name for a copy made in the same folder, like Explorer:
/// "report - Copy.pdf", then "report - Copy (2).pdf", …
pub fn copy_name(dir: &Path, original: &str) -> String {
    let (stem, ext) = split_extension(original);
    let base = format!("{stem} - Copy{ext}");
    first_free(dir, &base, |stem, ext, n| format!("{stem} ({n}){ext}"))
}

fn first_free(dir: &Path, desired: &str, numbered: impl Fn(&str, &str, u32) -> String) -> String {
    if !exists(dir, desired) {
        return desired.to_owned();
    }
    let (stem, ext) = split_extension(desired);
    (2..)
        .map(|n| numbered(stem, ext, n))
        .find(|candidate| !exists(dir, candidate))
        .unwrap_or_else(|| desired.to_owned())
}

fn exists(dir: &Path, name: &str) -> bool {
    std::fs::symlink_metadata(dir.join(name)).is_ok()
}

/// Splits "archive.tar.gz" into ("archive.tar", ".gz"); names starting with
/// a dot (".gitignore") or without one have no extension.
fn split_extension(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(i) => name.split_at(i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::write;

    #[test]
    fn accepts_ordinary_names() {
        for name in [
            "report.pdf",
            ".gitignore",
            "My Folder",
            "a b c (2).txt",
            "ñandú",
        ] {
            assert_eq!(validate_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn rejects_empty_and_slashes_everywhere() {
        assert_eq!(validate_name(""), Err(NameError::Empty));
        assert_eq!(validate_name(".."), Err(NameError::Empty));
        assert_eq!(validate_name("a/b"), Err(NameError::InvalidChar('/')));
    }

    #[test]
    #[cfg(windows)]
    fn rejects_windows_specific_names() {
        assert_eq!(validate_name("a:b"), Err(NameError::InvalidChar(':')));
        assert_eq!(validate_name("what?"), Err(NameError::InvalidChar('?')));
        assert_eq!(validate_name("name."), Err(NameError::TrailingDotOrSpace));
        assert_eq!(validate_name("con"), Err(NameError::Reserved("con".into())));
        assert_eq!(
            validate_name("COM1.txt"),
            Err(NameError::Reserved("COM1".into()))
        );
        assert_eq!(validate_name("COM0.txt"), Ok(()));
        assert_eq!(validate_name("console.txt"), Ok(()));
    }

    #[test]
    fn unique_and_copy_names() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unique_name(dir.path(), "a.txt"), "a.txt");
        write(dir.path().join("a.txt"), "").unwrap();
        write(dir.path().join("a (2).txt"), "").unwrap();
        assert_eq!(unique_name(dir.path(), "a.txt"), "a (3).txt");

        assert_eq!(copy_name(dir.path(), "a.txt"), "a - Copy.txt");
        write(dir.path().join("a - Copy.txt"), "").unwrap();
        assert_eq!(copy_name(dir.path(), "a.txt"), "a - Copy (2).txt");
        assert_eq!(copy_name(dir.path(), ".gitignore"), ".gitignore - Copy");
    }
}
