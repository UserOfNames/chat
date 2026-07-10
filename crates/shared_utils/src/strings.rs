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
    fn into_fast_trim(mut self) -> Self {
        self.fast_trim();
        self
    }
}

impl Sealed for String {}

impl StringExt for String {
    fn fast_trim(&mut self) {
        let start_offset = self.len() - self.trim_start().len();
        let end_offset = self.trim_end().len();

        // self.len() - self.trim_start().len() == self.len()
        // self.trim_start().len() == 0
        // This implies the `String` is all whitespace.
        if start_offset == self.len() {
            self.clear();
            return;
        }

        // Eliminates trailing whitespace.
        self.truncate(end_offset);

        // Eliminates leading whitespace. Under the hood, this optimizes down to a pointer copy, so
        // it's as fast as can be.
        self.drain(..start_offset);
    }
}
