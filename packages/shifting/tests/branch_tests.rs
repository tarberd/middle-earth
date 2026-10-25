use core::convert::Infallible;

use shifting::either;
use shifting::{Branch, BranchExt, Either, LoopFlow, Step, StepExt};

#[derive(Debug, PartialEq, Eq)]
struct ParseError(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct EvalError(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct LeftError(&'static str);

#[derive(Debug, PartialEq, Eq)]
struct RightError(u32);

#[test]
fn test_branch_basic_evaluate() {
    let check_even = |n: i32| -> Result<Either<i32, String>, EvalError> {
        if n % 2 == 0 {
            Ok(Either::Left(n))
        } else {
            Ok(Either::Right(format!("odd: {n}")))
        }
    };

    assert_eq!(check_even.evaluate(4), Ok(Either::Left(4)));
    assert_eq!(
        check_even.evaluate(7),
        Ok(Either::Right("odd: 7".to_string()))
    );

    let check_sign = |n: i32| -> Result<Either<i32, String>, EvalError> {
        if n > 0 {
            Ok(Either::Left(n))
        } else {
            Ok(Either::Right("non-positive".to_string()))
        }
    };
    assert_eq!(check_sign.evaluate(5), Ok(Either::Left(5)));
    assert_eq!(
        check_sign.evaluate(-2),
        Ok(Either::Right("non-positive".to_string()))
    );
}

#[test]
fn test_step_branch_composition_with_bare_closures() {
    let parse_step = |s: &str| -> Result<i32, ParseError> {
        match s.parse::<i32>() {
            Ok(v) => Ok(v),
            Err(_) => Err(ParseError("parse failed")),
        }
    };

    let is_positive = |n: i32| -> Result<Either<u32, i32>, EvalError> {
        match u32::try_from(n) {
            Ok(pos) => Ok(Either::Left(pos)),
            Err(_) => Ok(Either::Right(n)),
        }
    };

    // Step-to-Branch chaining: .branch() on StepExt with bare closures
    let pipeline = parse_step.branch(is_positive);

    assert_eq!(pipeline.evaluate("42"), Ok(Either::Left(42)));
    assert_eq!(pipeline.evaluate("-15"), Ok(Either::Right(-15)));
    assert_eq!(
        pipeline.evaluate("not-a-number"),
        Err(Either::Left(ParseError("parse failed")))
    );
}

#[test]
fn test_direct_cascading_branch_composition() {
    let outer_branch = |n: i32| -> Result<Either<i32, i32>, Infallible> {
        if n % 2 == 0 {
            Ok(Either::Left(n))
        } else {
            Ok(Either::Right(n))
        }
    };

    let even_branch = |n: i32| -> Result<Either<u32, i32>, Infallible> {
        if n >= 0 {
            Ok(Either::Left(n.unsigned_abs()))
        } else {
            Ok(Either::Right(n))
        }
    };

    let odd_branch = |n: i32| -> Result<Either<String, String>, Infallible> {
        if n % 3 == 0 {
            Ok(Either::Left(format!("div3: {n}")))
        } else {
            Ok(Either::Right(format!("other: {n}")))
        }
    };

    // 1. branch_both: cascades both arms directly
    let both_cascaded = outer_branch.branch_both(even_branch, odd_branch);
    assert_eq!(
        both_cascaded.evaluate(4),
        Ok(Either::Left(Either::Left(4u32)))
    );
    assert_eq!(
        both_cascaded.evaluate(-4),
        Ok(Either::Left(Either::Right(-4)))
    );
    assert_eq!(
        both_cascaded.evaluate(9),
        Ok(Either::Right(Either::Left("div3: 9".to_string())))
    );
    assert_eq!(
        both_cascaded.evaluate(7),
        Ok(Either::Right(Either::Right("other: 7".to_string())))
    );

    // 2. branch_left: cascades left arm only
    let left_cascaded = outer_branch.branch_left(even_branch);
    assert_eq!(
        left_cascaded.evaluate(4),
        Ok(Either::Left(Either::Left(4u32)))
    );
    assert_eq!(left_cascaded.evaluate(7), Ok(Either::Right(7)));

    // 3. branch_right: cascades right arm only
    let right_cascaded = outer_branch.branch_right(odd_branch);
    assert_eq!(right_cascaded.evaluate(4), Ok(Either::Left(4)));
    assert_eq!(
        right_cascaded.evaluate(9),
        Ok(Either::Right(Either::Left("div3: 9".to_string())))
    );
    assert_eq!(
        right_cascaded.evaluate(7),
        Ok(Either::Right(Either::Right("other: 7".to_string())))
    );

    // 4. branch_both with either::into_left and either::into_right
    let into_cascaded = outer_branch.branch_both(either::into_left::<i32, String>(), either::into_right::<String, i32>());
    assert_eq!(into_cascaded.evaluate(4), Ok(Either::Left(Either::Left(4))));
    assert_eq!(into_cascaded.evaluate(7), Ok(Either::Right(Either::Right(7))));

    // Universal coproduct pass-through verification: [iota_L, iota_R] = id_{L+R}
    let coproduct_id_passthrough = outer_branch.branch_both(either::into_left::<i32, i32>(), either::into_right::<i32, i32>());
    let collapsed_4 = coproduct_id_passthrough
        .evaluate(4)
        .expect("eval 4 ok")
        .either(
            |left_coproduct| left_coproduct.either(Either::Left, Either::Right),
            |right_coproduct| right_coproduct.either(Either::Left, Either::Right),
        );
    assert_eq!(collapsed_4, outer_branch.evaluate(4).expect("outer 4 ok"));

    let collapsed_7 = coproduct_id_passthrough
        .evaluate(7)
        .expect("eval 7 ok")
        .either(
            |left_coproduct| left_coproduct.either(Either::Left, Either::Right),
            |right_coproduct| right_coproduct.either(Either::Left, Either::Right),
        );
    assert_eq!(collapsed_7, outer_branch.evaluate(7).expect("outer 7 ok"));

    // 5. step.branch with either::id() (coproduct identity)
    let step_with_either = |n: i32| -> Result<Either<i32, String>, Infallible> {
        if n >= 0 {
            Ok(Either::Left(n))
        } else {
            Ok(Either::Right("neg".to_string()))
        }
    };
    let lifted_branch = step_with_either.branch(either::id::<i32, String>());
    assert_eq!(lifted_branch.evaluate(10), Ok(Either::Left(10)));
    assert_eq!(lifted_branch.evaluate(-1), Ok(Either::Right("neg".to_string())));
}

#[test]
fn test_branch_cascading_error_combinations() {
    let root_branch = |n: i32| -> Result<Either<i32, i32>, EvalError> {
        if n == 0 {
            Err(EvalError("zero is root invalid"))
        } else if n > 0 {
            Ok(Either::Left(n))
        } else {
            Ok(Either::Right(n))
        }
    };

    let left_branch = |n: i32| -> Result<Either<String, String>, LeftError> {
        if n > 100 {
            Err(LeftError("left overflow"))
        } else {
            Ok(Either::Left(format!("left: {n}")))
        }
    };

    let right_branch = |n: i32| -> Result<Either<String, String>, RightError> {
        if n < -100 {
            Err(RightError(404))
        } else {
            Ok(Either::Right(format!("right: {n}")))
        }
    };

    // Case 1: branch_both errors
    let both = root_branch.branch_both(left_branch, right_branch);

    // Root fails: Either::Left(EvalError)
    assert_eq!(
        both.evaluate(0),
        Err(Either::Left(EvalError("zero is root invalid")))
    );
    // Left fails: Either::Right(Either::Left(LeftError))
    assert_eq!(
        both.evaluate(150),
        Err(Either::Right(Either::Left(LeftError("left overflow"))))
    );
    // Right fails: Either::Right(Either::Right(RightError))
    assert_eq!(
        both.evaluate(-150),
        Err(Either::Right(Either::Right(RightError(404))))
    );
    // Both succeed on left
    assert_eq!(
        both.evaluate(10),
        Ok(Either::Left(Either::Left("left: 10".to_string())))
    );
    // Both succeed on right
    assert_eq!(
        both.evaluate(-10),
        Ok(Either::Right(Either::Right("right: -10".to_string())))
    );

    // Case 2: branch_left errors
    let left_only = root_branch.branch_left(left_branch);
    assert_eq!(
        left_only.evaluate(0),
        Err(Either::Left(EvalError("zero is root invalid")))
    );
    assert_eq!(
        left_only.evaluate(150),
        Err(Either::Right(LeftError("left overflow")))
    );
    assert_eq!(
        left_only.evaluate(10),
        Ok(Either::Left(Either::Left("left: 10".to_string())))
    );
    assert_eq!(left_only.evaluate(-10), Ok(Either::Right(-10)));

    // Case 3: branch_right errors
    let right_only = root_branch.branch_right(right_branch);
    assert_eq!(
        right_only.evaluate(0),
        Err(Either::Left(EvalError("zero is root invalid")))
    );
    assert_eq!(
        right_only.evaluate(-150),
        Err(Either::Right(RightError(404)))
    );
    assert_eq!(right_only.evaluate(10), Ok(Either::Left(10)));
    assert_eq!(
        right_only.evaluate(-10),
        Ok(Either::Right(Either::Right("right: -10".to_string())))
    );
}

#[test]
fn test_full_agentic_pipeline_graph_with_bare_closures() {
    #[derive(Debug, PartialEq, Eq)]
    struct Idle {
        pub task_id: u32,
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Planned {
        pub task_id: u32,
    }
    #[derive(Debug, PartialEq, Eq)]
    struct WorkDone {
        pub task_id: u32,
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Verified {
        pub task_id: u32,
    }
    #[derive(Debug, PartialEq, Eq)]
    struct Rejected {
        pub task_id: u32,
    }
    #[derive(Debug, PartialEq, Eq)]
    enum Outcome {
        Committed(u32),
        RolledBack(u32),
    }

    #[derive(Debug, PartialEq, Eq)]
    struct PlanError;
    #[derive(Debug, PartialEq, Eq)]
    struct WorkError;
    #[derive(Debug, PartialEq, Eq)]
    struct VerifyError;
    #[derive(Debug, PartialEq, Eq)]
    struct TerminalError;

    let plan = |s: Idle| -> Result<Planned, PlanError> { Ok(Planned { task_id: s.task_id }) };

    let work = |s: Planned| -> Result<WorkDone, WorkError> { Ok(WorkDone { task_id: s.task_id }) };

    let verify = |s: WorkDone| -> Result<Either<Verified, Rejected>, VerifyError> {
        if s.task_id > 0 {
            Ok(Either::Left(Verified { task_id: s.task_id }))
        } else {
            Ok(Either::Right(Rejected { task_id: s.task_id }))
        }
    };

    let commit =
        |s: Verified| -> Result<Outcome, TerminalError> { Ok(Outcome::Committed(s.task_id)) };

    let rollback =
        |s: Rejected| -> Result<Outcome, TerminalError> { Ok(Outcome::RolledBack(s.task_id)) };

    // Sequential steps chained into pure decision branch
    let pipeline = plan.and_then(work).branch(verify);

    // Verify successful path
    let ok_decision = pipeline.evaluate(Idle { task_id: 101 }).expect("eval ok");
    let flow: LoopFlow<Verified, Rejected> = ok_decision.into();
    let final_outcome = match flow {
        LoopFlow::Continue(v) => commit.apply(v).expect("commit ok"),
        LoopFlow::Break(r) => rollback.apply(r).expect("rollback ok"),
    };
    assert_eq!(final_outcome, Outcome::Committed(101));

    // Verify rejection path
    let rejected_decision = pipeline
        .evaluate(Idle { task_id: 0 })
        .expect("eval rejected");
    let outcome = rejected_decision.either(
        |v| commit.apply(v).expect("commit ok"),
        |r| rollback.apply(r).expect("rollback ok"),
    );
    assert_eq!(outcome, Outcome::RolledBack(0));
}
