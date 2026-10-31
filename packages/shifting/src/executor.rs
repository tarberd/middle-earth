use core::iter;
use core::ops::ControlFlow;

use crate::either::LoopFlow;
use crate::step::Step;

/// A generic state machine execution engine driving iterative transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Executor<P> {
    pub pipeline: P,
}

impl<P> Executor<P> {
    /// Creates a new `Executor` wrapping the given pipeline.
    #[must_use]
    pub const fn new(pipeline: P) -> Self {
        Self { pipeline }
    }

    /// Executes the pipeline iteratively until reaching [`LoopFlow::Break`], in constant stack
    /// however many iterations it takes.
    pub fn run<State, Terminal>(&self, initial_state: State) -> Result<Terminal, P::Error>
    where
        P: Step<State, Out = LoopFlow<State, Terminal>>,
    {
        match iter::repeat(()).try_fold(initial_state, |state, ()| self.pass(state)) {
            ControlFlow::Break(ended) => ended,
            // The passes never run out, so the fold only returns at a break. A state it handed
            // back would still be running, so it runs on.
            ControlFlow::Continue(state) => self.run(state),
        }
    }

    /// One pass of the pipeline: the state the next pass starts from, or how the run ended.
    fn pass<State, Terminal>(&self, state: State) -> ControlFlow<Result<Terminal, P::Error>, State>
    where
        P: Step<State, Out = LoopFlow<State, Terminal>>,
    {
        match self.pipeline.apply(state) {
            Ok(LoopFlow::Continue(next_state)) => ControlFlow::Continue(next_state),
            Ok(LoopFlow::Break(terminal)) => ControlFlow::Break(Ok(terminal)),
            Err(error) => ControlFlow::Break(Err(error)),
        }
    }
}

impl<State, Terminal, P> Step<State> for Executor<P>
where
    P: Step<State, Out = LoopFlow<State, Terminal>>,
{
    type Out = Terminal;
    type Error = P::Error;

    fn apply(&self, input: State) -> Result<Self::Out, Self::Error> {
        self.run(input)
    }
}
