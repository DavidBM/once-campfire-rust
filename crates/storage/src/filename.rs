//! `ActiveStorage::Filename`, with Ruby's `File.extname`/`File.basename` semantics.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filename(String);

impl Filename {
    pub fn new(filename: impl Into<String>) -> Self {
        Self(filename.into())
    }

    /// Raw bytes as they arrived; invalid UTF-8 is replaced the way `sanitized`'s `encode` does.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(String::from_utf8_lossy(bytes).into_owned())
    }

    /// The stored (unsanitized) value, as written to `active_storage_blobs.filename`.
    pub fn raw(&self) -> &str {
        &self.0
    }

    /// `File.basename(filename, extension_with_delimiter)`.
    pub fn base(&self) -> &str {
        let base = basename(&self.0);
        let ext = self.extension_with_delimiter();
        if !ext.is_empty() && base.len() > ext.len() && base.ends_with(ext) { &base[..base.len() - ext.len()] } else { base }
    }

    pub fn extension_with_delimiter(&self) -> &str {
        extname(&self.0)
    }

    /// `extension_without_delimiter`, aliased as `extension`.
    pub fn extension(&self) -> &str {
        self.extension_with_delimiter().get(1..).unwrap_or("")
    }

    /// `strip`, then replace RTL override, path separators and shell/HTML metacharacters with "-".
    pub fn sanitized(&self) -> String {
        ruby_compat::strip(&self.0).chars().map(|c| if "\u{202E}%$|:;/<>?*\"\t\r\n\\".contains(c) { '-' } else { c }).collect()
    }
}

impl std::fmt::Display for Filename {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.sanitized())
    }
}

/// `File.basename(path)`: the last component, ignoring trailing slashes.
pub fn basename(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        return if path.is_empty() { "" } else { "/" };
    }
    match trimmed.rfind('/') {
        Some(i) => &trimmed[i + 1..],
        None => trimmed,
    }
}

/// `File.extname(path)` on Unix: leading dots don't start an extension, and a trailing dot is
/// an extension of its own (`"foo."` → `"."`).
pub fn extname(path: &str) -> &str {
    let base = basename(path);
    let without_leading_dots = base.trim_start_matches('.');
    match without_leading_dots.rfind('.') {
        Some(i) => &without_leading_dots[i..],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_ruby_file_semantics() {
        let cases = [
            ("foo.", ".", "foo"),
            ("a.b.", ".", "a.b"),
            (".bashrc", "", ".bashrc"),
            ("..", "", ".."),
            ("...", "", "..."),
            (".a.b", ".b", ".a"),
            ("a..b", ".b", "a."),
            ("a/b.c/d", "", "d"),
            ("a.b/", ".b", "a"),
            ("x.tar.gz", ".gz", "x.tar"),
            ("a/.b", "", ".b"),
            (".", "", "."),
            ("", "", ""),
            ("é.png", ".png", "é"),
            ("a. b", ". b", "a"),
        ];
        for (name, ext, base) in cases {
            let filename = Filename::new(name);
            assert_eq!(filename.extension_with_delimiter(), ext, "extname({name:?})");
            assert_eq!(filename.base(), base, "base({name:?})");
        }
    }
}
