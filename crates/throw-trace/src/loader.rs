use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

pub struct FileLoader {
    exclude_patterns: GlobSet,
}

impl FileLoader {
    pub fn new(exclude_patterns: &[String]) -> Result<Self> {
        let mut builder = GlobSetBuilder::new();
        for pattern in exclude_patterns {
            builder.add(Glob::new(pattern).context("Invalid glob pattern")?);
        }
        Ok(Self { exclude_patterns: builder.build()? })
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
        self.exclude_patterns.is_match(path)
    }
}
