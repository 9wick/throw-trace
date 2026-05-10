use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
use oxc_parser::Parser;
use oxc_span::SourceType;
use throw_trace_core::ErrorType;

/// Analyze a throw expression and extract the error type.
/// Input should be a throw statement like "throw new Error('msg')".
pub fn analyze_throw_expr(source: &str) -> ErrorType {
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
        return analyze_expression(&throw_stmt.argument);
    }

    ErrorType::Unknown
}

fn analyze_expression(expr: &Expression) -> ErrorType {
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
            let consequent = analyze_expression(&cond.consequent);
            if matches!(consequent, ErrorType::Named(_)) {
                return consequent;
            }
            analyze_expression(&cond.alternate)
        }
        _ => ErrorType::Unknown,
    }
}
