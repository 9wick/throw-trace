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
        Self { file_path, name: name.into(), span }
    }

    pub fn anonymous(file_path: PathBuf, line: u32, span: Span) -> Self {
        Self { file_path, name: format!("anonymous_L{line}").into(), span }
    }
}

impl fmt::Display for FunctionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file_path.display(), self.name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorType {
    Named(CompactString),
    Unknown,
}

impl ErrorType {
    pub fn type_name(&self) -> Option<&str> {
        match self {
            Self::Named(name) => Some(name.as_str()),
            Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThrowSite {
    pub location: Span,
    pub error_type: ErrorType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredThrow {
    pub error_type: CompactString,
    pub description: Option<CompactString>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallSite {
    pub callee_name: CompactString,
    pub location: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TryCatchBlock {
    pub try_span: Span,
    pub catch_span: Option<Span>,
    pub caught_types: Vec<CompactString>,
}

impl TryCatchBlock {
    pub fn contains(&self, offset: u32) -> bool {
        offset >= self.try_span.start && offset < self.try_span.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub id: FunctionId,
    pub declared_throws: Vec<DeclaredThrow>,
    pub direct_throws: Vec<ThrowSite>,
    pub calls: Vec<CallSite>,
    pub try_catch_blocks: Vec<TryCatchBlock>,
    pub is_async: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropagatedThrow {
    pub error_type: ErrorType,
    pub origin: ThrowSite,
    pub path: Vec<FunctionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub function: FunctionId,
    pub missing_throws: Vec<PropagatedThrow>,
}
