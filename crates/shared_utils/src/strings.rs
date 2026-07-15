mod private {
    pub trait Sealed {}
}

use private::Sealed;

/// Extra methods on `String`.
pub trait StringExt: Sealed + Sized {
    /// Trim a [`String`] in-place. Unlike the typical `string.trim().to_owned()` pattern, this
    /// method will never reallocate.
    fn fast_trim(&mut self);

    /// Consumes the [`String`], trims it in-place without reallocating, and returns it. Useful for
    /// method chaining.
    #[must_use]
    fn into_fast_trim(self) -> Self;
}

impl Sealed for String {}

impl StringExt for String {
    fn fast_trim(&mut self) {
        let start_offset = self.len() - self.trim_start().len();

        // self.len() - self.trim_start().len() == self.len()
        // self.trim_start().len() == 0
        // This implies the `String` is all whitespace.
        if start_offset == self.len() {
            self.clear();
            return;
        }

        let end_offset = self.trim_end().len();

        // Eliminates trailing whitespace.
        self.truncate(end_offset);

        // Eliminates leading whitespace. Under the hood, this optimizes down to a pointer copy, so
        // it's as fast as can be.
        self.drain(..start_offset);
    }

    fn into_fast_trim(mut self) -> Self {
        self.fast_trim();
        self
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_fast_trim_handles_empty_string() {
        let mut string = String::new();

        string.fast_trim();

        assert!(string.is_empty());
    }

    #[test]
    fn test_fast_trim_handles_all_whitespace() {
        let mut string = String::from("   \t\n  \n \t  ");

        string.fast_trim();

        assert!(string.is_empty());
    }

    #[test]
    fn test_fast_trim_handles_left_whitespace() {
        let mut string = String::from("   \t\n  Hello World!");
        let trimmed_string = string.trim().to_owned();

        string.fast_trim();

        assert_eq!(trimmed_string, string);
    }

    #[test]
    fn test_fast_trim_handles_right_whitespace() {
        let mut string = String::from("Hello World!   \t\n   \n");
        let trimmed_string = string.trim().to_owned();

        string.fast_trim();

        assert_eq!(trimmed_string, string);
    }

    #[test]
    fn test_fast_trim_handles_left_and_right_whitespace() {
        let mut string = String::from("\t\n   \t   \n \nHello World!   \t\n   \n");
        let trimmed_string = string.trim().to_owned();

        string.fast_trim();

        assert_eq!(trimmed_string, string);
    }

    #[test]
    fn test_fast_trim_handles_unusual_whitespace() {
        let ideographic = String::from_utf8(vec![0xE3, 0x80, 0x80]).expect(
            "Conversion of ideographic whitespace failed, byte pattern was written incorrectly. This is a problem with the test design, not the module."
        );
        let zero_width = String::from_utf8(vec![0xE2, 0x80, 0x8B]).expect(
            "Conversion of zero width whitespace failed, byte pattern was written incorrectly. This is a problem with the test design, not the module."
        );
        let ogham = String::from_utf8(vec![0xE1, 0x9A, 0x80]).expect(
            "Conversion of ogham whitespace failed, byte pattern was written incorrectly. This is a problem with the test design, not the module."
        );

        let mut string = format!("{ideographic}  \t \n   \t  {zero_width} Hello World!\n {ogham}  ");
        let trimmed_string = string.trim().to_owned();

        string.fast_trim();

        assert_eq!(trimmed_string, string);
    }
}
