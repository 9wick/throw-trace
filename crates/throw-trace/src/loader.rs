use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::{Component, Path, PathBuf};

pub struct FileLoader {
    exclude_patterns: GlobSet,
    base_dir: PathBuf,
}

impl FileLoader {
    pub fn new(exclude_patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for pattern in exclude_patterns {
            builder.add(Glob::new(pattern).context("Invalid glob pattern")?);
        }
        let base_dir = std::env::current_dir().context("cannot determine current directory")?;
        Ok(Self { exclude_patterns: builder.build()?, base_dir })
    }

    pub fn collect_ts_files(&self, paths: &[String]) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        for path_str in paths {
            let path = Path::new(path_str);

            if path.is_file() {
                if Self::is_ts_file(path) && !self.is_excluded(path) {
                    files.push(path.to_path_buf());
                }
            } else if path.is_dir() {
                for entry in WalkBuilder::new(path).build() {
                    let entry = entry?;
                    let entry_path = entry.path();
                    if entry_path.is_file()
                        && Self::is_ts_file(entry_path)
                        && !self.is_excluded(entry_path)
                    {
                        files.push(entry_path.to_path_buf());
                    }
                }
            } else {
                // 黙ってスキップすると CI がサイレント成功するためエラーにする
                anyhow::bail!("path does not exist: {path_str}");
            }
        }

        Ok(files)
    }

    fn is_ts_file(path: &Path) -> bool {
        matches!(path.extension().and_then(|e| e.to_str()), Some("ts" | "tsx" | "mts" | "cts"))
    }

    fn is_excluded(&self, path: &Path) -> bool {
        // walk 結果は引数の形式（`./src/a.ts` や絶対パス）を引き継ぐため、
        // ワークスペース相対に正規化してからパターン照合する
        let relative = path.strip_prefix(&self.base_dir).unwrap_or(path);
        let normalized: PathBuf =
            relative.components().filter(|c| !matches!(c, Component::CurDir)).collect();
        self.exclude_patterns.is_match(&normalized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_matches_plain_relative_path() {
        let loader = FileLoader::new(&["src/**".to_string()]).unwrap();
        assert!(loader.is_excluded(Path::new("src/a.ts")));
    }

    #[test]
    fn exclude_matches_dot_prefixed_path() {
        // `check .` の walk 結果は `./src/a.ts` 形式になる
        let loader = FileLoader::new(&["src/**".to_string()]).unwrap();
        assert!(loader.is_excluded(Path::new("./src/a.ts")));
    }

    #[test]
    fn exclude_matches_absolute_path_under_cwd() {
        let loader = FileLoader::new(&["src/**".to_string()]).unwrap();
        let abs = std::env::current_dir().unwrap().join("src/a.ts");
        assert!(loader.is_excluded(&abs));
    }

    #[test]
    fn exclude_does_not_match_unrelated_path() {
        let loader = FileLoader::new(&["src/**".to_string()]).unwrap();
        assert!(!loader.is_excluded(Path::new("./lib/a.ts")));
    }
}
