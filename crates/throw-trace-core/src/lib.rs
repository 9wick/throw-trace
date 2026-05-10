//! Core engine for throw-trace: types, call graph, propagation analysis.

mod types;

pub use types::{ErrorType, FunctionId, Span, ThrowSite};

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
}
