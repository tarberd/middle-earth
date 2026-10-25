use core::convert::Infallible;
use core::fmt;
use core::ops::Deref;

use crate::branch::Branch;
use crate::step::Step;

/// A zero-dependency coproduct type representing one of two possibilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<L, R> Either<L, R> {
    /// Converts from `Either<L, R>` to `Option<L>`, discarding any `Right` value.
    #[must_use]
    pub fn left(self) -> Option<L> {
        match self {
            Self::Left(l) => Some(l),
            Self::Right(_) => None,
        }
    }

    /// Converts from `Either<L, R>` to `Option<R>`, discarding any `Left` value.
    #[must_use]
    pub fn right(self) -> Option<R> {
        match self {
            Self::Left(_) => None,
            Self::Right(r) => Some(r),
        }
    }

    /// Converts from `&Either<L, R>` to `Either<&L, &R>`.
    #[must_use]
    pub const fn as_ref(&self) -> Either<&L, &R> {
        match self {
            Self::Left(l) => Either::Left(l),
            Self::Right(r) => Either::Right(r),
        }
    }

    /// Converts from `&Either<L, R>` to `Either<&L::Target, &R::Target>`.
    #[must_use]
    pub fn as_deref(&self) -> Either<&L::Target, &R::Target>
    where
        L: Deref,
        R: Deref,
    {
        match self {
            Self::Left(l) => Either::Left(l.deref()),
            Self::Right(r) => Either::Right(r.deref()),
        }
    }

    /// Maps an `Either<L, R>` to `Either<T, R>` by applying a function to a contained `Left` value.
    pub fn map_left<F, T>(self, f: F) -> Either<T, R>
    where
        F: FnOnce(L) -> T,
    {
        match self {
            Self::Left(l) => Either::Left(f(l)),
            Self::Right(r) => Either::Right(r),
        }
    }

    /// Maps an `Either<L, R>` to `Either<L, T>` by applying a function to a contained `Right` value.
    pub fn map_right<F, T>(self, f: F) -> Either<L, T>
    where
        F: FnOnce(R) -> T,
    {
        match self {
            Self::Left(l) => Either::Left(l),
            Self::Right(r) => Either::Right(f(r)),
        }
    }

    /// Applies one of two functions depending on the variant, producing a unified output.
    pub fn either<T, FL, FR>(self, on_left: FL, on_right: FR) -> T
    where
        FL: FnOnce(L) -> T,
        FR: FnOnce(R) -> T,
    {
        match self {
            Self::Left(l) => on_left(l),
            Self::Right(r) => on_right(r),
        }
    }

    /// Returns the identity morphism for this coproduct.
    #[must_use]
    pub const fn id() -> EitherIdentity<L, R> {
        EitherIdentity::new()
    }

    /// Returns the canonical left injection/value constructor morphism.
    #[must_use]
    pub const fn into_left() -> IntoLeft<L, R> {
        IntoLeft::new()
    }

    /// Returns the canonical right injection/value constructor morphism.
    #[must_use]
    pub const fn into_right() -> IntoRight<L, R> {
        IntoRight::new()
    }
}

impl<T> Either<T, T> {
    /// Extracts the inner value when both sides share the same type.
    pub fn into_inner(self) -> T {
        match self {
            Self::Left(v) | Self::Right(v) => v,
        }
    }
}

impl<L, R> fmt::Display for Either<L, R>
where
    L: fmt::Display,
    R: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left(l) => fmt::Display::fmt(l, f),
            Self::Right(r) => fmt::Display::fmt(r, f),
        }
    }
}

impl<L, R> std::error::Error for Either<L, R>
where
    L: std::error::Error,
    R: std::error::Error,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Left(l) => l.source(),
            Self::Right(r) => r.source(),
        }
    }
}

/// A control-flow sum type driving iterative state machine execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoopFlow<State, Terminal> {
    Continue(State),
    Break(Terminal),
}

impl<State, Terminal> LoopFlow<State, Terminal> {
    /// Converts from `LoopFlow<State, Terminal>` to `Option<State>`, discarding any `Break` value.
    #[must_use]
    pub fn continue_value(self) -> Option<State> {
        match self {
            Self::Continue(s) => Some(s),
            Self::Break(_) => None,
        }
    }

    /// Converts from `LoopFlow<State, Terminal>` to `Option<Terminal>`, discarding any `Continue` value.
    #[must_use]
    pub fn break_value(self) -> Option<Terminal> {
        match self {
            Self::Continue(_) => None,
            Self::Break(t) => Some(t),
        }
    }

    /// Converts from `&LoopFlow<State, Terminal>` to `LoopFlow<&State, &Terminal>`.
    #[must_use]
    pub const fn as_ref(&self) -> LoopFlow<&State, &Terminal> {
        match self {
            Self::Continue(s) => LoopFlow::Continue(s),
            Self::Break(t) => LoopFlow::Break(t),
        }
    }

    /// Converts from `&LoopFlow<State, Terminal>` to `LoopFlow<&State::Target, &Terminal::Target>`.
    #[must_use]
    pub fn as_deref(&self) -> LoopFlow<&State::Target, &Terminal::Target>
    where
        State: Deref,
        Terminal: Deref,
    {
        match self {
            Self::Continue(s) => LoopFlow::Continue(s.deref()),
            Self::Break(t) => LoopFlow::Break(t.deref()),
        }
    }
}

impl<L, R> From<Either<L, R>> for LoopFlow<L, R> {
    fn from(either: Either<L, R>) -> Self {
        match either {
            Either::Left(l) => Self::Continue(l),
            Either::Right(r) => Self::Break(r),
        }
    }
}

impl<S, T> From<LoopFlow<S, T>> for Either<S, T> {
    fn from(flow: LoopFlow<S, T>) -> Self {
        match flow {
            LoopFlow::Continue(s) => Self::Left(s),
            LoopFlow::Break(t) => Self::Right(t),
        }
    }
}

type EitherIdentityMarker<L, R> = core::marker::PhantomData<(fn(L) -> L, fn(R) -> R)>;

/// Coproduct identity morphism ($id_{L+R}: L+R \to L+R$).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct EitherIdentity<L, R> {
    marker: EitherIdentityMarker<L, R>,
}

pub type BranchIdentity<L, R> = EitherIdentity<L, R>;

impl<L, R> Clone for EitherIdentity<L, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L, R> Copy for EitherIdentity<L, R> {}

impl<L, R> Default for EitherIdentity<L, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L, R> EitherIdentity<L, R> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marker: core::marker::PhantomData,
        }
    }

    #[must_use]
    pub const fn id() -> Self {
        Self::new()
    }
}

impl<L, R> Step<Either<L, R>> for EitherIdentity<L, R> {
    type Out = Either<L, R>;
    type Error = Infallible;

    fn apply(&self, input: Either<L, R>) -> Result<Self::Out, Self::Error> {
        Ok(input)
    }
}

impl<L, R> Branch<Either<L, R>> for EitherIdentity<L, R> {
    type Left = L;
    type Right = R;
    type Error = Infallible;

    fn evaluate(
        &self,
        input: Either<L, R>,
    ) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        Ok(input)
    }
}

/// Returns the coproduct identity morphism.
#[must_use]
pub const fn either_id<L, R>() -> EitherIdentity<L, R> {
    EitherIdentity::new()
}

/// Returns the branch identity morphism.
#[must_use]
pub const fn branch_id<L, R>() -> BranchIdentity<L, R> {
    BranchIdentity::new()
}

/// Canonical coproduct left injection/value constructor morphism ($\iota_L: L \to L + R$).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct IntoLeft<L, R> {
    marker: core::marker::PhantomData<fn(L) -> Either<L, R>>,
}

pub type InjectLeft<L, R> = IntoLeft<L, R>;

impl<L, R> Clone for IntoLeft<L, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L, R> Copy for IntoLeft<L, R> {}

impl<L, R> Default for IntoLeft<L, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L, R> IntoLeft<L, R> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marker: core::marker::PhantomData,
        }
    }
}

impl<L, R> Step<L> for IntoLeft<L, R> {
    type Out = Either<L, R>;
    type Error = Infallible;

    fn apply(&self, input: L) -> Result<Self::Out, Self::Error> {
        Ok(Either::Left(input))
    }
}

impl<L, R> Branch<L> for IntoLeft<L, R> {
    type Left = L;
    type Right = R;
    type Error = Infallible;

    fn evaluate(&self, input: L) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        Ok(Either::Left(input))
    }
}

/// Returns the canonical left injection/value constructor morphism.
#[must_use]
pub const fn into_left<L, R>() -> IntoLeft<L, R> {
    IntoLeft::new()
}

/// Canonical coproduct right injection/value constructor morphism ($\iota_R: R \to L + R$).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct IntoRight<L, R> {
    marker: core::marker::PhantomData<fn(R) -> Either<L, R>>,
}

pub type InjectRight<L, R> = IntoRight<L, R>;

impl<L, R> Clone for IntoRight<L, R> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<L, R> Copy for IntoRight<L, R> {}

impl<L, R> Default for IntoRight<L, R> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L, R> IntoRight<L, R> {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            marker: core::marker::PhantomData,
        }
    }
}

impl<L, R> Step<R> for IntoRight<L, R> {
    type Out = Either<L, R>;
    type Error = Infallible;

    fn apply(&self, input: R) -> Result<Self::Out, Self::Error> {
        Ok(Either::Right(input))
    }
}

impl<L, R> Branch<R> for IntoRight<L, R> {
    type Left = L;
    type Right = R;
    type Error = Infallible;

    fn evaluate(&self, input: R) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        Ok(Either::Right(input))
    }
}

/// Returns the canonical right injection/value constructor morphism.
#[must_use]
pub const fn into_right<L, R>() -> IntoRight<L, R> {
    IntoRight::new()
}
