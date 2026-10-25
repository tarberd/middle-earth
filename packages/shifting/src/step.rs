use core::convert::Infallible;

use crate::adt::Either;
use crate::branch::Branch;

/// An unconditional state transition morphism transforming an input state into an output state.
pub trait Step<In> {
    type Out;
    type Error;

    /// Applies the transition morphism to the given input state.
    fn apply(&self, input: In) -> Result<Self::Out, Self::Error>;

    /// Returns the identity morphism for this step.
    fn id() -> StepIdentity<In>
    where
        Self: Sized,
    {
        StepIdentity::new()
    }
}

/// Blanket implementation of [`Step`] for any closure matching `Fn(In) -> Result<Out, Error>`.
impl<F, In, Out, Error> Step<In> for F
where
    F: Fn(In) -> Result<Out, Error>,
{
    type Out = Out;
    type Error = Error;

    fn apply(&self, input: In) -> Result<Self::Out, Self::Error> {
        self(input)
    }
}

/// Canonical identity morphism ($id_A: A \to A$).
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct StepIdentity<T> {
    marker: core::marker::PhantomData<fn(T) -> T>,
}

pub type Identity<T> = StepIdentity<T>;

impl<T> Clone for Identity<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Identity<T> {}

impl<T> Default for Identity<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> StepIdentity<T> {
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

impl<T> Step<T> for Identity<T> {
    type Out = T;
    type Error = Infallible;

    fn apply(&self, input: T) -> Result<Self::Out, Self::Error> {
        Ok(input)
    }
}

/// Returns the step identity morphism.
#[must_use]
pub const fn step_id<T>() -> StepIdentity<T> {
    StepIdentity::new()
}

/// Sequential composition of two steps ($g \circ f$).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AndThen<First, Second> {
    pub(crate) first: First,
    pub(crate) second: Second,
}

impl<First, Second> AndThen<First, Second> {
    #[must_use]
    pub const fn new(first: First, second: Second) -> Self {
        Self { first, second }
    }
}

impl<In, First, Second> Step<In> for AndThen<First, Second>
where
    First: Step<In>,
    Second: Step<First::Out>,
{
    type Out = Second::Out;
    type Error = Either<First::Error, Second::Error>;

    fn apply(&self, input: In) -> Result<Self::Out, Self::Error> {
        let intermediate = self.first.apply(input).map_err(Either::Left)?;
        self.second.apply(intermediate).map_err(Either::Right)
    }
}

/// Extension combinators for chaining and transforming [`Step`] transitions.
pub trait StepExt<In>: Step<In> {
    /// Chains another step sequentially after `self` ($g \circ f$), preserving heterogeneous errors via [`Either`].
    fn and_then<Next>(self, next: Next) -> AndThen<Self, Next>
    where
        Self: Sized,
        Next: Step<Self::Out>,
    {
        AndThen::new(self, next)
    }

    /// Composes a branch coproduct morphism after `self` ($b \circ f$), preserving heterogeneous errors via [`Either`].
    fn branch<B>(self, branch: B) -> crate::branch::StepBranch<Self, B>
    where
        Self: Sized,
        B: Branch<Self::Out>,
    {
        crate::branch::StepBranch::new(self, branch)
    }
}

impl<T, In> StepExt<In> for T where T: Step<In> {}
