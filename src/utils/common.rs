use std::{
    fmt::{self, Debug, Display, Formatter},
    str::FromStr,
};

use either::Either;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// An `untagged` version of `Either`.
///
/// The reason this structure is needed is that `either::Either` is
/// by default an `Externally Tagged` enum, and it is possible to
/// implement `untagged` via `#[serde(with = "either::serde_untagged_optional")]`
/// as well. But this approach can cause problems with deserialization,
/// resulting in having to manually add the `#[serde(default)]` tag,
/// and this leads to less readable as well as less flexible code.
/// So it would be better if we manually implement this `UntaggedEither` here,
/// while providing a two-way conversion to `either::Either`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UntaggedEither<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> From<UntaggedEither<L, R>> for Either<L, R> {
    fn from(value: UntaggedEither<L, R>) -> Self {
        match value {
            UntaggedEither::Left(l) => Self::Left(l),
            UntaggedEither::Right(r) => Self::Right(r),
        }
    }
}

impl<L, R> From<Either<L, R>> for UntaggedEither<L, R> {
    fn from(value: Either<L, R>) -> Self {
        match value {
            Either::Left(l) => UntaggedEither::Left(l),
            Either::Right(r) => UntaggedEither::Right(r),
        }
    }
}

/// A generic newtype which serialized using `Display` and deserialized using `FromStr`.
#[derive(Default, Clone, DeserializeFromStr, SerializeDisplay)]
pub struct SerdeDisplayFromStr<T>(pub T);

impl<T: Debug> Debug for SerdeDisplayFromStr<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<T: Display> Display for SerdeDisplayFromStr<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<T: FromStr> FromStr for SerdeDisplayFromStr<T> {
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<T>().map(Self)
    }
}

/// The reason it's needed here is that using `Deserialize` generated
/// by default by `serde` will cause deserialization to fail if
/// both untyped formats (such as `urlencoded`) and `untagged enum`
/// are used. The Wrap type here forces the deserialization process to
/// be delegated to `FromStr`.
pub type WrappedBool = SerdeDisplayFromStr<bool>;

/// Returns truncated string accounting for multi-byte characters.
pub(crate) fn safe_truncate(s: &str, len: usize) -> &str {
    if len == 0 {
        return "";
    }

    if s.len() <= len {
        return s;
    }

    if s.is_char_boundary(len) {
        return &s[0..len];
    }

    // Only 3 cases possible: 1, 2, or 3 bytes need to be removed for a new,
    // valid UTF-8 string to appear when truncated, just enumerate them,
    // Underflow is not possible since position 0 is always a valid boundary.

    if let Some((slice, _rest)) = s.split_at_checked(len - 1) {
        return slice;
    }

    if let Some((slice, _rest)) = s.split_at_checked(len - 2) {
        return slice;
    }

    if let Some((slice, _rest)) = s.split_at_checked(len - 3) {
        return slice;
    }

    unreachable!("all branches covered");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_truncation() {
        assert_eq!(safe_truncate("", 0), "");
        assert_eq!(safe_truncate("", 1), "");
        assert_eq!(safe_truncate("", 9), "");

        assert_eq!(safe_truncate("a", 0), "");
        assert_eq!(safe_truncate("a", 1), "a");
        assert_eq!(safe_truncate("a", 9), "a");

        assert_eq!(safe_truncate("lorem\nipsum", 0), "");
        assert_eq!(safe_truncate("lorem\nipsum", 5), "lorem");
        assert_eq!(safe_truncate("lorem\nipsum", usize::MAX), "lorem\nipsum");

        assert_eq!(safe_truncate("café", 1), "c");
        assert_eq!(safe_truncate("café", 2), "ca");
        assert_eq!(safe_truncate("café", 3), "caf");
        assert_eq!(safe_truncate("café", 4), "caf");
        assert_eq!(safe_truncate("café", 5), "café");

        // 2-byte char
        assert_eq!(safe_truncate("é", 0), "");
        assert_eq!(safe_truncate("é", 1), "");
        assert_eq!(safe_truncate("é", 2), "é");

        // 3-byte char
        assert_eq!(safe_truncate("⊕", 0), "");
        assert_eq!(safe_truncate("⊕", 1), "");
        assert_eq!(safe_truncate("⊕", 2), "");
        assert_eq!(safe_truncate("⊕", 3), "⊕");

        // 4-byte char
        assert_eq!(safe_truncate("🦊", 0), "");
        assert_eq!(safe_truncate("🦊", 1), "");
        assert_eq!(safe_truncate("🦊", 2), "");
        assert_eq!(safe_truncate("🦊", 3), "");
        assert_eq!(safe_truncate("🦊", 4), "🦊");
    }
}
