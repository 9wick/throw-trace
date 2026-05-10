//! TypeScript adapter for throw-trace (oxc-based).

pub fn hello() -> &'static str {
    "throw-trace-ts"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_returns_crate_name() {
        assert_eq!(hello(), "throw-trace-ts");
    }
}
