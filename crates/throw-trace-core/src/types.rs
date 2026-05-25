use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// Checks type compatibility for thrown vs declared types.
pub trait TypeResolver {
    /// Check if `thrown_type` is assignable to `declared_type`.
    fn is_assignable_to(
        &mut self,
        file_path: &Path,
        thrown_type: &str,
        declared_type: &str,
    ) -> bool;

    /// Resolve the type of an expression at the given span.
    fn resolve_type(&mut self, file_path: &Path, span: Span) -> Option<String>;
}

/// Default resolver that uses simple string equality.
pub struct NoOpTypeResolver;

impl TypeResolver for NoOpTypeResolver {
    fn is_assignable_to(
        &mut self,
        _file_path: &Path,
        thrown_type: &str,
        declared_type: &str,
    ) -> bool {
        thrown_type == declared_type
    }

    fn resolve_type(&mut self, _file_path: &Path, _span: Span) -> Option<String> {
        None
    }
}

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
    Rethrow(CompactString),
    Unknown,
}

impl ErrorType {
    pub fn type_name(&self) -> Option<&str> {
        match self {
            Self::Named(name) => Some(name.as_str()),
            Self::Rethrow(_) | Self::Unknown => None,
        }
    }

    pub fn is_rethrow(&self) -> bool {
        matches!(self, Self::Rethrow(_))
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
    pub callee_span: Span,
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
    pub name_span: Span,
    pub declared_throws: Vec<DeclaredThrow>,
    pub direct_throws: Vec<ThrowSite>,
    pub calls: Vec<CallSite>,
    pub try_catch_blocks: Vec<TryCatchBlock>,
    pub is_async: bool,
    pub class_name: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropagatedThrow {
    pub error_type: ErrorType,
    pub origin: ThrowSite,
    pub origin_function: FunctionId,
    pub path: Vec<FunctionId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub function: FunctionId,
    pub missing_throws: Vec<PropagatedThrow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TypeId {
    pub file_path: PathBuf,
    pub name: CompactString,
    pub span: Span,
}

impl TypeId {
    pub fn new(file_path: PathBuf, name: impl Into<CompactString>, span: Span) -> Self {
        Self { file_path, name: name.into(), span }
    }
}

impl fmt::Display for TypeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file_path.display(), self.name)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    Implements,
    Extends,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRelation {
    pub child: TypeId,
    pub parent: TypeId,
    pub kind: RelationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MethodSignature {
    pub type_id: TypeId,
    pub method_name: CompactString,
    pub method_span: Span,
    pub declared_throws: Vec<DeclaredThrow>,
    pub is_abstract: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspViolation {
    pub implementation: FunctionId,
    pub parent_method: MethodSignature,
    pub illegal_throws: Vec<ErrorType>,
}
