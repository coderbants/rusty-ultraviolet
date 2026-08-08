//! Cleanroom Rust port of upstream Go source file: `utils.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! Shared numeric helpers.
//! </public-docs>

/// abs returns the absolute value of x.
#[allow(dead_code)]
pub(crate) fn abs(x: i64) -> i64 {
    if x < 0 {
        -x
    } else {
        x
    }
}

/// clamp returns v clamped to the [low, high] range. If high < low, the
/// bounds are swapped first.
#[allow(dead_code)]
pub(crate) fn clamp(v: i64, low: i64, high: i64) -> i64 {
    let (low, high) = if high < low { (high, low) } else { (low, high) };
    v.max(low).min(high)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abs() {
        assert_eq!(abs(5), 5);
        assert_eq!(abs(-5), 5);
        assert_eq!(abs(0), 0);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(5, 0, 10), 5);
        assert_eq!(clamp(-1, 0, 10), 0);
        assert_eq!(clamp(11, 0, 10), 10);
        // Swapped bounds.
        assert_eq!(clamp(3, 10, 0), 3);
    }
}
