use crate::adt::Either;
use crate::step::Step;

/// A conditional decision coproduct morphism mapping an input state to a sum type (`Either<L, R>`).
pub trait Branch<In> {
    type Left;
    type Right;
    type Error;

    /// Evaluates the branch condition against the input state.
    fn evaluate(&self, input: In) -> Result<Either<Self::Left, Self::Right>, Self::Error>;

    /// Returns the identity morphism for this branch.
    fn id<L, R>() -> crate::adt::BranchIdentity<L, R>
    where
        Self: Sized,
    {
        crate::adt::BranchIdentity::new()
    }
}

/// Blanket implementation of [`Branch`] for any closure matching `Fn(In) -> Result<Either<Left, Right>, Error>`.
impl<F, In, Left, Right, Error> Branch<In> for F
where
    F: Fn(In) -> Result<Either<Left, Right>, Error>,
{
    type Left = Left;
    type Right = Right;
    type Error = Error;

    fn evaluate(&self, input: In) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        self(input)
    }
}

/// A composite morphism chaining a [`Step`] into a [`Branch`] ($b \circ f$).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StepBranch<S, B> {
    pub(crate) step: S,
    pub(crate) branch: B,
}

impl<S, B> StepBranch<S, B> {
    #[must_use]
    pub const fn new(step: S, branch: B) -> Self {
        Self { step, branch }
    }
}

impl<In, S, B> Branch<In> for StepBranch<S, B>
where
    S: Step<In>,
    B: Branch<S::Out>,
{
    type Left = B::Left;
    type Right = B::Right;
    type Error = Either<S::Error, B::Error>;

    fn evaluate(&self, input: In) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        let intermediate = self.step.apply(input).map_err(Either::Left)?;
        self.branch.evaluate(intermediate).map_err(Either::Right)
    }
}

/// Cascades branch decision morphisms along both arms of a coproduct without intermediate steps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BranchBoth<B, LeftBranch, RightBranch> {
    pub(crate) branch: B,
    pub(crate) left_branch: LeftBranch,
    pub(crate) right_branch: RightBranch,
}

impl<B, LeftBranch, RightBranch> BranchBoth<B, LeftBranch, RightBranch> {
    #[must_use]
    pub const fn new(branch: B, left_branch: LeftBranch, right_branch: RightBranch) -> Self {
        Self {
            branch,
            left_branch,
            right_branch,
        }
    }
}

impl<In, B, LeftBranch, RightBranch> Branch<In> for BranchBoth<B, LeftBranch, RightBranch>
where
    B: Branch<In>,
    LeftBranch: Branch<B::Left>,
    RightBranch: Branch<B::Right>,
{
    type Left = Either<LeftBranch::Left, LeftBranch::Right>;
    type Right = Either<RightBranch::Left, RightBranch::Right>;
    type Error = Either<B::Error, Either<LeftBranch::Error, RightBranch::Error>>;

    fn evaluate(&self, input: In) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        let branch_result = self.branch.evaluate(input).map_err(Either::Left)?;
        match branch_result {
            Either::Left(l) => self
                .left_branch
                .evaluate(l)
                .map(Either::Left)
                .map_err(|e| Either::Right(Either::Left(e))),
            Either::Right(r) => self
                .right_branch
                .evaluate(r)
                .map(Either::Right)
                .map_err(|e| Either::Right(Either::Right(e))),
        }
    }
}

/// Cascades a branch decision morphism along the left arm of a coproduct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BranchLeft<B, LeftBranch> {
    pub(crate) branch: B,
    pub(crate) left_branch: LeftBranch,
}

impl<B, LeftBranch> BranchLeft<B, LeftBranch> {
    #[must_use]
    pub const fn new(branch: B, left_branch: LeftBranch) -> Self {
        Self {
            branch,
            left_branch,
        }
    }
}

impl<In, B, LeftBranch> Branch<In> for BranchLeft<B, LeftBranch>
where
    B: Branch<In>,
    LeftBranch: Branch<B::Left>,
{
    type Left = Either<LeftBranch::Left, LeftBranch::Right>;
    type Right = B::Right;
    type Error = Either<B::Error, LeftBranch::Error>;

    fn evaluate(&self, input: In) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        let branch_result = self.branch.evaluate(input).map_err(Either::Left)?;
        match branch_result {
            Either::Left(l) => self
                .left_branch
                .evaluate(l)
                .map(Either::Left)
                .map_err(Either::Right),
            Either::Right(r) => Ok(Either::Right(r)),
        }
    }
}

/// Cascades a branch decision morphism along the right arm of a coproduct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BranchRight<B, RightBranch> {
    pub(crate) branch: B,
    pub(crate) right_branch: RightBranch,
}

impl<B, RightBranch> BranchRight<B, RightBranch> {
    #[must_use]
    pub const fn new(branch: B, right_branch: RightBranch) -> Self {
        Self {
            branch,
            right_branch,
        }
    }
}

impl<In, B, RightBranch> Branch<In> for BranchRight<B, RightBranch>
where
    B: Branch<In>,
    RightBranch: Branch<B::Right>,
{
    type Left = B::Left;
    type Right = Either<RightBranch::Left, RightBranch::Right>;
    type Error = Either<B::Error, RightBranch::Error>;

    fn evaluate(&self, input: In) -> Result<Either<Self::Left, Self::Right>, Self::Error> {
        let branch_result = self.branch.evaluate(input).map_err(Either::Left)?;
        match branch_result {
            Either::Left(l) => Ok(Either::Left(l)),
            Either::Right(r) => self
                .right_branch
                .evaluate(r)
                .map(Either::Right)
                .map_err(Either::Right),
        }
    }
}

/// Extension combinators for chaining and transforming [`Branch`] transitions.
pub trait BranchExt<In>: Branch<In> {
    /// Cascades branch decision morphisms along both arms of a coproduct without intermediate steps.
    fn branch_both<LeftBranch, RightBranch>(
        self,
        left_branch: LeftBranch,
        right_branch: RightBranch,
    ) -> BranchBoth<Self, LeftBranch, RightBranch>
    where
        Self: Sized,
        LeftBranch: Branch<Self::Left>,
        RightBranch: Branch<Self::Right>,
    {
        BranchBoth::new(self, left_branch, right_branch)
    }

    /// Cascades a branch decision morphism along the left arm of a coproduct.
    fn branch_left<LeftBranch>(self, left_branch: LeftBranch) -> BranchLeft<Self, LeftBranch>
    where
        Self: Sized,
        LeftBranch: Branch<Self::Left>,
    {
        BranchLeft::new(self, left_branch)
    }

    /// Cascades a branch decision morphism along the right arm of a coproduct.
    fn branch_right<RightBranch>(self, right_branch: RightBranch) -> BranchRight<Self, RightBranch>
    where
        Self: Sized,
        RightBranch: Branch<Self::Right>,
    {
        BranchRight::new(self, right_branch)
    }
}

impl<T, In> BranchExt<In> for T where T: Branch<In> {}
