use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionId {
    pub file_path: PathBuf,
    pub name: CompactString,
    pub span: Span,
}

impl FunctionId {
    pub fn new(file_path: PathBuf, name: impl Into<CompactString>, span: Span) -> Self {
        Self {
            file_path,
            name: name.into(),
            span,
        }
    }

    pub fn anonymous(file_path: PathBuf, line: u32, span: Span) -> Self {
        Self {
            file_path,
            name: format!("anonymous_L{line}").into(),
            span,
        }
    }
}

impl fmt::Display for FunctionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file_path.display(), self.name)
    }
}
