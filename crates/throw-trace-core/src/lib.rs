//! Core engine for throw-trace: types, call graph, propagation analysis.

mod types;

pub use types::{
    CallSite, DeclaredThrow, ErrorType, FunctionId, FunctionSignature, Span, ThrowSite,
    TryCatchBlock,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn function_id_display() {
        let id = FunctionId {
            file_path: PathBuf::from("src/service.ts"),
            name: "createUser".into(),
            span: Span { start: 10, end: 50 },
        };
        assert_eq!(format!("{id}"), "src/service.ts:createUser");
    }

    #[test]
    fn function_id_anonymous() {
        let id = FunctionId::anonymous(PathBuf::from("src/util.ts"), 42, Span { start: 100, end: 150 });
        assert_eq!(id.name.as_str(), "anonymous_L42");
    }

    #[test]
    fn error_type_named() {
        let err = ErrorType::Named("ValidationError".into());
        assert_eq!(err.type_name(), Some("ValidationError"));
    }

    #[test]
    fn error_type_unknown() {
        let err = ErrorType::Unknown;
        assert_eq!(err.type_name(), None);
    }

    #[test]
    fn throw_site_creation() {
        let site = ThrowSite {
            location: Span { start: 100, end: 120 },
            error_type: ErrorType::Named("MyError".into()),
        };
        assert_eq!(site.error_type.type_name(), Some("MyError"));
    }

    #[test]
    fn declared_throw_with_description() {
        let decl = DeclaredThrow {
            error_type: "ValidationError".into(),
            description: Some("When input is invalid".into()),
            span: Span { start: 5, end: 50 },
        };
        assert_eq!(decl.error_type.as_str(), "ValidationError");
        assert!(decl.description.is_some());
    }

    #[test]
    fn call_site_creation() {
        let call = CallSite {
            callee_name: "validate".into(),
            location: Span { start: 200, end: 220 },
        };
        assert_eq!(call.callee_name.as_str(), "validate");
    }

    #[test]
    fn try_catch_block_contains_span() {
        let block = TryCatchBlock {
            try_span: Span { start: 100, end: 200 },
            catch_span: Some(Span { start: 200, end: 300 }),
            caught_types: vec!["ValidationError".into()],
        };
        assert!(block.contains(150));
        assert!(!block.contains(50));
    }

    #[test]
    fn function_signature_creation() {
        let sig = FunctionSignature {
            id: FunctionId::new(
                PathBuf::from("src/test.ts"),
                "testFn",
                Span { start: 0, end: 100 },
            ),
            declared_throws: vec![],
            direct_throws: vec![],
            calls: vec![],
            try_catch_blocks: vec![],
            is_async: false,
        };
        assert_eq!(sig.id.name.as_str(), "testFn");
        assert!(!sig.is_async);
    }
}
