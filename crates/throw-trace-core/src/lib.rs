//! Core engine for throw-trace: types, call graph, propagation analysis.

pub fn hello() -> &'static str {
    "throw-trace-core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_returns_crate_name() {
        assert_eq!(hello(), "throw-trace-core");
    }
}
