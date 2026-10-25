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

    /// Executes the pipeline iteratively until reaching [`LoopFlow::Break`].
    pub fn run<State, Terminal>(&self, initial_state: State) -> Result<Terminal, P::Error>
    where
        P: Step<State, Out = LoopFlow<State, Terminal>>,
    {
        match self.pipeline.apply(initial_state)? {
            LoopFlow::Continue(next_state) => self.run(next_state),
            LoopFlow::Break(terminal) => Ok(terminal),
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
