use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use throw_trace_core::ErrorType;

/// Analyze a throw expression and extract the error type.
/// Input should be a throw statement like "throw new Error('msg')".
pub fn analyze_throw_expr(source: &str) -> ErrorType {
    analyze_throw_expr_with_catch_params(source, &[])
}

/// Analyze a throw expression with knowledge of catch parameter names.
/// If throwing an identifier that matches a catch param, returns Rethrow.
pub fn analyze_throw_expr_with_catch_params(source: &str, catch_params: &[String]) -> ErrorType {
    let allocator = Allocator::default();
    let source_type = SourceType::ts();
    let parser_return = Parser::new(&allocator, source, source_type).parse();

    if !parser_return.errors.is_empty() {
        return ErrorType::Unknown;
    }

    let program = &parser_return.program;
    if program.body.is_empty() {
        return ErrorType::Unknown;
    }

    if let Statement::ThrowStatement(throw_stmt) = &program.body[0] {
        return analyze_expression(&throw_stmt.argument, catch_params);
    }

    ErrorType::Unknown
}

fn analyze_expression(expr: &Expression, catch_params: &[String]) -> ErrorType {
    match expr {
        Expression::NewExpression(new_expr) => {
            if let Expression::Identifier(id) = &new_expr.callee {
                return ErrorType::Named(id.name.as_str().into());
            }
            ErrorType::Unknown
        }
        Expression::CallExpression(call_expr) => {
            if let Expression::Identifier(id) = &call_expr.callee {
                if id.name.as_str().ends_with("Error") {
                    return ErrorType::Named(id.name.as_str().into());
                }
            }
            ErrorType::Unknown
        }
        Expression::ConditionalExpression(cond) => {
            let consequent = analyze_expression(&cond.consequent, catch_params);
            if matches!(consequent, ErrorType::Named(_)) {
                return consequent;
            }
            analyze_expression(&cond.alternate, catch_params)
        }
        Expression::Identifier(id) => {
            let name = id.name.as_str();
            if catch_params.iter().any(|p| p == name) {
                ErrorType::Rethrow(name.into())
            } else {
                ErrorType::Unknown
            }
        }
        _ => ErrorType::Unknown,
    }
}
