//! TypeScript adapter for throw-trace (oxc-based).

mod extract;
mod jsdoc;
mod parser;
mod throw_analyzer;
mod try_catch;
pub mod tsserver;

pub use extract::{extract_all, extract_functions, ExtractionResult};
pub use jsdoc::extract_throws_from_jsdoc;
pub use parser::parse_source;
pub use throw_analyzer::{analyze_throw_expr, analyze_throw_expr_with_catch_params};
pub use try_catch::extract_try_catch_blocks;
pub use tsserver::{byte_offset_to_line_col, TsServer, TsServerError, TsServerTypeResolver};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use throw_trace_core::ErrorType;

    #[test]
    fn parse_source_returns_program() {
        let source = "function foo() { return 1; }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_handles_typescript() {
        let source = "function foo(x: number): string { return x.toString(); }";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_source_reports_syntax_error() {
        let source = "function foo( { }";
        let result = parse_source(source);
        assert!(result.is_err());
    }

    #[test]
    fn extract_functions_finds_function_declaration() {
        let source = "function foo() { }\nfunction bar() { }";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 2);
        assert_eq!(sigs[0].id.name.as_str(), "foo");
        assert_eq!(sigs[1].id.name.as_str(), "bar");
    }

    #[test]
    fn extract_functions_finds_arrow_function() {
        let source = "const add = (a: number, b: number) => a + b;";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].id.name.as_str(), "add");
    }

    #[test]
    fn extract_functions_finds_function_expression() {
        let source = "const throwsInExpr = function() { throw new E1(); };";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1, "function expression must produce a signature");
        assert_eq!(sigs[0].id.name.as_str(), "throwsInExpr");
        assert_eq!(sigs[0].direct_throws.len(), 1);
    }

    #[test]
    fn extract_functions_finds_exported_function_expression() {
        let source = "export const throwsInExpr = function() { throw new E1(); };";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1, "exported function expression must produce a signature");
        assert_eq!(sigs[0].id.name.as_str(), "throwsInExpr");
        assert_eq!(sigs[0].direct_throws.len(), 1);
    }

    #[test]
    fn extract_functions_finds_class_property_arrow() {
        let source = r"
class Handler {
    onClick = () => {
        throw new PropError();
    };
}
";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        let on_click = sigs
            .iter()
            .find(|s| s.id.name.as_str() == "onClick")
            .expect("class property arrow must produce a signature");
        assert_eq!(on_click.direct_throws.len(), 1);
        assert_eq!(on_click.class_name.as_deref(), Some("Handler"));
    }

    #[test]
    fn class_property_arrow_throw_not_attributed_to_sibling_method() {
        let source = r"
class Handler {
    onClick = () => {
        throw new PropError();
    };

    safe() {
        return 1;
    }
}
";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        let safe = sigs.iter().find(|s| s.id.name.as_str() == "safe").unwrap();
        assert!(safe.direct_throws.is_empty());
    }

    #[test]
    fn extract_functions_finds_async_function() {
        let source = "async function fetchData() { }";
        let file_path = PathBuf::from("test.ts");
        let result = extract_functions(source, &file_path);
        assert!(result.is_ok());
        let sigs = result.unwrap();
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].is_async);
    }

    #[test]
    fn extract_throws_single() {
        let comment = "/**\n * @throws {ValidationError} When input is invalid\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].type_name, "ValidationError");
        assert_eq!(throws[0].description.as_deref(), Some("When input is invalid"));
    }

    #[test]
    fn extract_throws_multiple() {
        let comment =
            "/**\n * @throws {ValidationError}\n * @throws {NetworkError} Connection failed\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].type_name, "ValidationError");
        assert_eq!(throws[1].type_name, "NetworkError");
    }

    #[test]
    fn extract_throws_no_braces() {
        let comment = "/** @throws Error when something fails */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 1);
        assert_eq!(throws[0].type_name, "Error");
    }

    #[test]
    fn extract_throws_empty() {
        let comment = "/** This is a description */";
        let throws = extract_throws_from_jsdoc(comment);
        assert!(throws.is_empty());
    }

    #[test]
    fn extract_throws_union_type() {
        let comment = "/**\n * @throws {ValidationError | NetworkError} When something fails\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 2);
        assert_eq!(throws[0].type_name, "ValidationError");
        assert_eq!(throws[0].description.as_deref(), Some("When something fails"));
        assert_eq!(throws[1].type_name, "NetworkError");
        assert_eq!(throws[1].description.as_deref(), Some("When something fails"));
    }

    #[test]
    fn extract_throws_union_type_three() {
        let comment = "/**\n * @throws {A | B | C}\n */";
        let throws = extract_throws_from_jsdoc(comment);
        assert_eq!(throws.len(), 3);
        assert_eq!(throws[0].type_name, "A");
        assert_eq!(throws[1].type_name, "B");
        assert_eq!(throws[2].type_name, "C");
    }

    #[test]
    fn analyze_throw_new_error() {
        let source = "throw new ValidationError('msg')";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Named("ValidationError".into()));
    }

    #[test]
    fn analyze_throw_new_error_simple() {
        let source = "throw new Error('msg')";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Named("Error".into()));
    }

    #[test]
    fn analyze_throw_literal() {
        let source = "throw 'error'";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Unknown);
    }

    #[test]
    fn analyze_throw_variable_is_unknown() {
        // throw variable (not catch param) should be Unknown
        let source = "throw err";
        let result = analyze_throw_expr(source);
        assert_eq!(result, ErrorType::Unknown);
    }

    #[test]
    fn analyze_throw_catch_param_is_rethrow() {
        // throw catch param should be Rethrow
        let source = "throw e";
        let catch_params = vec!["e".to_string()];
        let result = analyze_throw_expr_with_catch_params(source, &catch_params);
        assert_eq!(result, ErrorType::Rethrow("e".into()));
    }

    #[test]
    fn analyze_throw_non_catch_param_is_unknown() {
        // throw variable that is NOT a catch param should be Unknown
        let source = "throw err";
        let catch_params = vec!["e".to_string()];
        let result = analyze_throw_expr_with_catch_params(source, &catch_params);
        assert_eq!(result, ErrorType::Unknown);
    }

    #[test]
    fn extract_try_catch_simple() {
        let source = r"
try {
    validate();
} catch (e) {
    console.log(e);
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].catch_span.is_some());
    }

    #[test]
    fn extract_try_catch_with_instanceof() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof ValidationError) {
        return;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].caught_types.len(), 1);
        assert_eq!(blocks[0].caught_types[0].as_str(), "ValidationError");
    }

    #[test]
    fn instanceof_else_if_chain_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof ErrorA) {
        return null;
    } else if (e instanceof ErrorB) {
        return null;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        let caught: Vec<&str> =
            blocks[0].caught_types.iter().map(compact_str::CompactString::as_str).collect();
        assert_eq!(caught, vec!["ErrorA", "ErrorB"], "else-if chain instanceof must be caught");
    }

    #[test]
    fn instanceof_else_if_without_termination_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof ErrorA) {
        return null;
    } else if (e instanceof ErrorB) {
        console.log(e);
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        let caught: Vec<&str> =
            blocks[0].caught_types.iter().map(compact_str::CompactString::as_str).collect();
        assert_eq!(caught, vec!["ErrorA"], "non-terminating else-if branch must not be caught");
    }

    #[test]
    fn instanceof_else_if_behind_non_instanceof_test_not_in_caught_types() {
        // 先頭の条件が catch param の instanceof でない場合、else-if への到達は
        // 例外型と無関係な条件に依存するため捕捉済みとはみなせない
        let source = r"
try {
    validate();
} catch (e) {
    if (cond) {
        console.log(e);
    } else if (e instanceof ErrorB) {
        return null;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "else-if behind unrelated condition must not be caught, got: {:?}",
            blocks[0].caught_types
        );
    }

    #[test]
    fn instanceof_without_return_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        console.log(e);
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "instanceof without terminal statement should not be in caught_types"
        );
    }

    #[test]
    fn instanceof_with_conditional_return_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        if (cond) return;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "instanceof with partial termination should not be in caught_types"
        );
    }

    #[test]
    fn instanceof_unreachable_rethrow_after_if_else_return_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        if (cond) return 1;
        else return 2;
        throw e;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].caught_types.len(),
            1,
            "throw e after if/else that both return is unreachable, SomeError should be caught"
        );
    }

    #[test]
    fn instanceof_with_conditional_rethrow_and_return_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        if (cond) throw e;
        return;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "conditional throw e is a reachable rethrow, return after it does not make it caught"
        );
    }

    #[test]
    fn instanceof_with_rethrow_catch_param_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        throw e;
    }
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "throw e in instanceof branch is a rethrow, not a catch"
        );
    }

    // instanceof の左辺が catch パラメータでない場合、
    // 捕捉した例外の型チェックではないので caught_types に入れてはならない
    #[test]
    fn instanceof_on_unrelated_variable_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (otherVar instanceof NetworkError) {
        return;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "instanceof on a variable other than the catch param must not count as caught, got: {:?}",
            blocks[0].caught_types
        );
    }

    // instanceof 分岐内の switch が全ケースで終端する場合は捕捉済みとみなす
    #[test]
    fn instanceof_with_exhaustive_switch_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        switch (e.code) {
            case 1:
                return null;
            case 2:
                throw new WrappedError(e);
            default:
                return null;
        }
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].caught_types,
            vec!["SomeError"],
            "switch terminating in every case (incl. default) should count as caught"
        );
    }

    // default がない switch はフォールスルーし得るので終端とみなさない
    #[test]
    fn instanceof_with_switch_without_default_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        switch (e.code) {
            case 1:
                return null;
        }
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "switch without default may fall through, must not count as caught, got: {:?}",
            blocks[0].caught_types
        );
    }

    // switch 内で catch パラメータを rethrow する場合は捕捉とみなさない
    #[test]
    fn instanceof_with_switch_rethrowing_catch_param_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        switch (e.code) {
            case 1:
                return null;
            default:
                throw e;
        }
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "rethrowing the catch param inside switch must not count as caught, got: {:?}",
            blocks[0].caught_types
        );
    }

    // パラメータなしの catch 節では例外を参照できないため、
    // instanceof による捕捉型は存在し得ない
    #[test]
    fn instanceof_in_paramless_catch_not_in_caught_types() {
        let source = r"
try {
    validate();
} catch {
    if (globalErr instanceof NetworkError) {
        return;
    }
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(
            blocks[0].caught_types.is_empty(),
            "paramless catch cannot type-check the caught error, got: {:?}",
            blocks[0].caught_types
        );
    }

    #[test]
    fn instanceof_with_throw_new_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        throw new WrappedError(e);
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0].caught_types.len(),
            1,
            "throw new replaces the error, so SomeError is caught"
        );
        assert_eq!(blocks[0].caught_types[0].as_str(), "SomeError");
    }

    #[test]
    fn instanceof_with_return_in_caught_types() {
        let source = r"
try {
    validate();
} catch (e) {
    if (e instanceof SomeError) {
        return null;
    }
    throw e;
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].caught_types.len(), 1);
        assert_eq!(blocks[0].caught_types[0].as_str(), "SomeError");
    }

    #[test]
    fn extract_try_catch_no_catch() {
        let source = r"
try {
    validate();
} finally {
    cleanup();
}
";
        let blocks = extract_try_catch_blocks(source);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].catch_span.is_none());
    }

    #[test]
    fn rethrow_in_catch_is_rethrow() {
        let source = r"
function foo() {
    try {
        bar();
    } catch (e) {
        throw e;
    }
}
";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].direct_throws.len(), 1);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Rethrow("e".into()));
    }

    #[test]
    fn throw_variable_outside_catch_is_unknown() {
        let source = r"
function foo() {
    const err = getError();
    throw err;
}
";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].direct_throws.len(), 1);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Unknown);
    }

    #[test]
    fn throw_typed_variable_uses_annotation() {
        let source = r"
function foo() {
    const err: SomeError = getError();
    throw err;
}
";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        assert_eq!(sigs[0].direct_throws.len(), 1);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Named("SomeError".into()));
    }

    #[test]
    fn throw_union_typed_variable_extracts_all() {
        let source = r"
function foo() {
    const err: ErrorA | ErrorB = getError();
    throw err;
}
";
        let file_path = PathBuf::from("test.ts");
        let sigs = extract_functions(source, &file_path).unwrap();
        assert_eq!(sigs.len(), 1);
        // Union type creates multiple throw sites
        assert_eq!(sigs[0].direct_throws.len(), 2);
        assert_eq!(sigs[0].direct_throws[0].error_type, ErrorType::Named("ErrorA".into()));
        assert_eq!(sigs[0].direct_throws[1].error_type, ErrorType::Named("ErrorB".into()));
    }

    #[test]
    fn extract_interface_with_method_throws() {
        let source = r"
interface UserRepository {
    /**
     * @throws {NotFoundError}
     */
    findById(id: string): User;
}
";
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.method_signatures.len(), 1);
        assert_eq!(result.method_signatures[0].method_name.as_str(), "findById");
        assert_eq!(result.method_signatures[0].declared_throws.len(), 1);
        assert_eq!(
            result.method_signatures[0].declared_throws[0].error_type.as_str(),
            "NotFoundError"
        );
    }

    #[test]
    fn extract_class_implements_interface() {
        let source = r"
interface Repository {}
class UserRepository implements Repository {}
";
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.type_relations.len(), 1);
        assert_eq!(result.type_relations[0].child.name.as_str(), "UserRepository");
        assert_eq!(result.type_relations[0].parent.name.as_str(), "Repository");
        assert_eq!(result.type_relations[0].kind, throw_trace_core::RelationKind::Implements);
    }

    #[test]
    fn extract_class_extends_class() {
        let source = r"
class BaseService {}
class UserService extends BaseService {}
";
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.type_relations.len(), 1);
        assert_eq!(result.type_relations[0].child.name.as_str(), "UserService");
        assert_eq!(result.type_relations[0].parent.name.as_str(), "BaseService");
        assert_eq!(result.type_relations[0].kind, throw_trace_core::RelationKind::Extends);
    }

    #[test]
    fn extract_abstract_class_method() {
        let source = r"
abstract class BaseRepository {
    /**
     * @throws {NotFoundError}
     */
    abstract findById(id: string): User;
}
";
        let file_path = PathBuf::from("test.ts");
        let result = extract_all(source, &file_path).unwrap();
        assert_eq!(result.method_signatures.len(), 1);
        assert_eq!(result.method_signatures[0].method_name.as_str(), "findById");
        assert!(result.method_signatures[0].is_abstract);
        assert_eq!(result.method_signatures[0].declared_throws.len(), 1);
    }
}
