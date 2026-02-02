# Rust Test Generation

You are generating tests using TDD - the implementation does not exist yet.

## Guidelines

- Use `#[cfg(test)]` module with `#[test]` functions
- Cover main functionality, edge cases, and error conditions
- Use `assert!`, `assert_eq!`, `assert_ne!` macros

## Output Format

~~~worksplit
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_name() {
        let result = function_name(input);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_error_case() {
        let result = function_name(bad_input);
        assert!(result.is_err());
    }
}
~~~worksplit

Output ONLY test code. No explanations.
