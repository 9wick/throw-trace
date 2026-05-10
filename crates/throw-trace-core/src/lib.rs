//! Core engine for throw-trace: types, call graph, propagation analysis.

mod types;

pub use types::{FunctionId, Span};

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
}
