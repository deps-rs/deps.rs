use either::Either;
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use std::{
    fmt::{self, Debug, Display, Formatter},
    str::FromStr,
};

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

impl<L, R> UntaggedEither<L, R> {
    pub fn into_either(self) -> Either<L, R> {
        self.into()
    }
}

/// A generic newtype which serialized using `Display` and deserialized using `FromStr`.
#[derive(Default, Clone, DeserializeFromStr, SerializeDisplay)]
pub struct SerdeDisplayFromStr<T>(pub T);

impl<T> From<T> for SerdeDisplayFromStr<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T: Debug> Debug for SerdeDisplayFromStr<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Display> Display for SerdeDisplayFromStr<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: FromStr> FromStr for SerdeDisplayFromStr<T> {
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(s.parse::<T>()?.into())
    }
}

/// The reason it's needed here is that using `Deserialize` generated
/// by default by `serde` will cause deserialization to fail if
/// both untyped formats (such as `urlencoded`) and `untagged enum`
/// are used. The Wrap type here forces the deserialization process to
/// be delegated to `FromStr`.
pub type WrappedBool = SerdeDisplayFromStr<bool>;
