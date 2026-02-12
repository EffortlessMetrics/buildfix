//! BDD harness (cucumber-rs).
//!
//! This crate exists to keep scenario tests isolated from the production crates.

pub fn noop() {}

#[cfg(test)]
mod tests {
    use super::noop;

    #[test]
    fn noop_is_callable() {
        noop();
    }
}
