use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use shifting::{Either, Executor, LoopFlow, Step, StepExt};

#[derive(Debug, PartialEq, Eq)]
struct CustomError {
    code: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct StateMachine {
    target: i32,
}

impl Step<i32> for StateMachine {
    type Out = LoopFlow<i32, String>;
    type Error = CustomError;

    fn apply(&self, input: i32) -> Result<Self::Out, Self::Error> {
        if input < 0 {
            Err(CustomError { code: 999 })
        } else if input >= self.target {
            Ok(LoopFlow::Break(format!("reached {input}")))
        } else {
            Ok(LoopFlow::Continue(input + 1))
        }
    }
}

#[test]
fn test_single_pass_immediate_break() {
    let pipeline = |n: i32| -> Result<LoopFlow<i32, String>, CustomError> {
        Ok(LoopFlow::Break(format!("terminal={n}")))
    };

    let executor = Executor::new(pipeline);
    let result = executor.run(42);
    assert_eq!(result, Ok("terminal=42".to_string()));

    let step_result = executor.apply(42);
    assert_eq!(step_result, Ok("terminal=42".to_string()));
}

#[test]
fn test_multi_pass_loop_convergence() {
    let step_fn = |n: i32| -> Result<LoopFlow<i32, i32>, CustomError> {
        if n >= 16 {
            Ok(LoopFlow::Break(n))
        } else {
            Ok(LoopFlow::Continue(n * 2))
        }
    };

    let executor = Executor::new(step_fn);
    let result = executor.run(1);
    assert_eq!(result, Ok(16));
}

#[test]
fn test_state_encoded_iteration_bounding() {
    #[derive(Debug, PartialEq, Eq)]
    enum BoundedResult {
        Completed(i32),
        MaxIterationsReached(i32),
    }

    // State encodes both domain value and remaining iteration fuel
    let bounding_step =
        |(val, remaining): (i32, u32)| -> Result<LoopFlow<(i32, u32), BoundedResult>, CustomError> {
            if val >= 100 {
                Ok(LoopFlow::Break(BoundedResult::Completed(val)))
            } else if remaining == 0 {
                Ok(LoopFlow::Break(BoundedResult::MaxIterationsReached(val)))
            } else {
                Ok(LoopFlow::Continue((val + 10, remaining - 1)))
            }
        };

    let executor = Executor::new(bounding_step);

    // Terminates cleanly due to iteration bound
    let bounded_run = executor.run((0, 3));
    assert_eq!(bounded_run, Ok(BoundedResult::MaxIterationsReached(30)));

    // Terminates cleanly due to convergence before bound
    let completed_run = executor.run((80, 5));
    assert_eq!(completed_run, Ok(BoundedResult::Completed(100)));
}

#[test]
fn test_error_short_circuiting_generic_error() {
    let error_step = |n: i32| -> Result<LoopFlow<i32, String>, CustomError> {
        if n == 2 {
            Err(CustomError { code: 42 })
        } else {
            Ok(LoopFlow::Continue(n + 1))
        }
    };

    let executor = Executor::new(error_step);
    let result = executor.run(0);
    assert_eq!(result, Err(CustomError { code: 42 }));
}

#[test]
fn test_executor_composition_as_step_via_and_then() {
    let initial_step = |input: &str| -> Result<i32, CustomError> {
        match input {
            "start" => Ok(0),
            _ => Err(CustomError { code: 1 }),
        }
    };

    let loop_step = |n: i32| -> Result<LoopFlow<i32, i32>, CustomError> {
        if n >= 5 {
            Ok(LoopFlow::Break(n * 10))
        } else {
            Ok(LoopFlow::Continue(n + 1))
        }
    };

    let finish_step = |n: i32| -> Result<String, CustomError> { Ok(format!("final={n}")) };

    let executor = Executor::new(loop_step);

    let full_pipeline = initial_step.and_then(executor).and_then(finish_step);

    let result = full_pipeline.apply("start");
    assert_eq!(result, Ok("final=50".to_string()));

    let err_result = full_pipeline.apply("bad");
    assert_eq!(
        err_result,
        Err(Either::Left(Either::Left(CustomError { code: 1 })))
    );
}

#[test]
fn test_step_ext_into_executor() {
    let loop_step = |n: i32| -> Result<LoopFlow<i32, i32>, CustomError> {
        if n >= 3 {
            Ok(LoopFlow::Break(n))
        } else {
            Ok(LoopFlow::Continue(n + 1))
        }
    };

    let executor = loop_step.into_executor();
    assert_eq!(executor.run(0), Ok(3));
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct NonCopyPipeline {
    label: String,
}

#[test]
fn test_trait_derivations_and_const_new() {
    const EXECUTOR: Executor<StateMachine> = Executor::new(StateMachine { target: 10 });

    let copied = EXECUTOR;
    assert_eq!(EXECUTOR, copied);

    let non_copy = Executor::new(NonCopyPipeline {
        label: "loop".to_string(),
    });
    let cloned = non_copy.clone();
    assert_eq!(non_copy, cloned);

    let formatted = format!("{EXECUTOR:?}");
    assert_eq!(
        formatted,
        "Executor { pipeline: StateMachine { target: 10 } }"
    );

    let mut hasher_a = DefaultHasher::new();
    EXECUTOR.hash(&mut hasher_a);
    let hash_a = hasher_a.finish();

    let mut hasher_b = DefaultHasher::new();
    copied.hash(&mut hasher_b);
    let hash_b = hasher_b.finish();

    assert_eq!(hash_a, hash_b);

    assert_eq!(EXECUTOR.run(8), Ok("reached 10".to_string()));
}
