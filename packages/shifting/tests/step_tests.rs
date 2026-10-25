use shifting::{step_id, Either, Step, StepExt, StepIdentity};

#[derive(Debug, PartialEq, Eq)]
struct StepError(&'static str);

#[derive(Debug, PartialEq, Eq)]
enum CompositeError {
    Step(StepError),
    Parsing(&'static str),
}

impl From<StepError> for CompositeError {
    fn from(err: StepError) -> Self {
        Self::Step(err)
    }
}

#[test]
fn test_step_basic_apply() {
    let double_step = |n: i32| -> Result<i32, StepError> { Ok(n * 2) };
    assert_eq!(double_step.apply(21), Ok(42));
}

#[test]
fn test_blanket_fn_implementation_for_step() {
    let closure_step = |n: i32| -> Result<i32, StepError> { Ok(n * 3) };
    assert_eq!(closure_step.apply(10), Ok(30));

    let chained_closures = (|n: i32| -> Result<i32, StepError> { Ok(n + 5) })
        .and_then(|n: i32| -> Result<String, StepError> { Ok(format!("val={n}")) });
    assert_eq!(chained_closures.apply(15), Ok("val=20".to_string()));
}

#[test]
fn test_step_and_then_sequential_composition() {
    let parse_step = |s: &str| -> Result<i32, StepError> {
        match s.parse::<i32>() {
            Ok(val) => Ok(val),
            Err(_) => Err(StepError("parse error")),
        }
    };

    let double_step = |n: i32| -> Result<i32, StepError> { Ok(n * 2) };
    let to_string_step = |n: i32| -> Result<String, StepError> { Ok(format!("val={n}")) };

    let pipeline = parse_step.and_then(double_step).and_then(to_string_step);

    assert_eq!(pipeline.apply("25"), Ok("val=50".to_string()));
}

#[test]
fn test_step_and_then_error_short_circuiting() {
    let fail_first = |n: i32| -> Result<i32, StepError> {
        if n >= 0 {
            Err(StepError("step1 failed"))
        } else {
            Err(StepError("negative"))
        }
    };
    let fail_second = |n: i32| -> Result<i32, StepError> {
        if n >= 0 {
            Err(StepError("step2 failed"))
        } else {
            Err(StepError("negative"))
        }
    };
    let ok_step = |n: i32| -> Result<i32, StepError> { Ok(n + 1) };

    let early_fail_pipeline = fail_first.and_then(ok_step);
    assert_eq!(
        early_fail_pipeline.apply(10),
        Err(Either::Left(StepError("step1 failed")))
    );

    let late_fail_pipeline = ok_step.and_then(fail_second);
    assert_eq!(
        late_fail_pipeline.apply(10),
        Err(Either::Right(StepError("step2 failed")))
    );
}

#[test]
fn test_step_heterogeneous_error_composition() {
    let step_with_step_error = |n: i32| -> Result<i32, StepError> {
        if n > 0 {
            Ok(n * 10)
        } else {
            Err(StepError("must be positive"))
        }
    };

    let step_with_composite_error = |n: i32| -> Result<String, CompositeError> {
        if n < 100 {
            Ok(format!("result: {n}"))
        } else {
            Err(CompositeError::Parsing("too large"))
        }
    };

    // Composition preserves heterogeneous errors without requiring From or unifying enums
    let pipeline = step_with_step_error.and_then(step_with_composite_error);

    assert_eq!(pipeline.apply(5), Ok("result: 50".to_string()));
    assert_eq!(
        pipeline.apply(-1),
        Err(Either::Left(StepError("must be positive")))
    );
    assert_eq!(
        pipeline.apply(20),
        Err(Either::Right(CompositeError::Parsing("too large")))
    );
}

#[test]
fn test_category_theory_identity_laws() {
    let step = |n: i32| -> Result<i32, StepError> {
        if n >= 0 {
            Ok(n * 2)
        } else {
            Err(StepError("negative"))
        }
    };

    // Left identity: StepIdentity::id().and_then(step) == step
    let left_id_pipeline = StepIdentity::id().and_then(step);
    let left_eval_ok = left_id_pipeline
        .apply(21)
        .map_err(|e| e.either(|infallible| match infallible {}, |err| err));
    assert_eq!(left_eval_ok, step.apply(21));

    let left_eval_err = left_id_pipeline
        .apply(-5)
        .map_err(|e| e.either(|infallible| match infallible {}, |err| err));
    assert_eq!(left_eval_err, step.apply(-5));

    // Right identity: step.and_then(step_id()) == step
    let right_id_pipeline = step.and_then(step_id::<i32>());
    let right_eval_ok = right_id_pipeline
        .apply(21)
        .map_err(|e| e.either(|err| err, |infallible| match infallible {}));
    assert_eq!(right_eval_ok, step.apply(21));

    let right_eval_err = right_id_pipeline
        .apply(-5)
        .map_err(|e| e.either(|err| err, |infallible| match infallible {}));
    assert_eq!(right_eval_err, step.apply(-5));
}

#[test]
fn test_category_theory_associativity_law() {
    let f = |n: i32| -> Result<i32, StepError> { Ok(n + 1) };
    let g = |n: i32| -> Result<i32, StepError> { Ok(n * 2) };
    let h = |n: i32| -> Result<String, StepError> { Ok(format!("v={n}")) };

    // (f . g) . h
    let left_assoc = f.and_then(g).and_then(h);
    // f . (g . h)
    let right_assoc = f.and_then(g.and_then(h));

    assert_eq!(
        left_assoc.apply(10).expect("left assoc ok"),
        right_assoc.apply(10).expect("right assoc ok")
    );

    // Error path associativity verification across heterogeneous error tree
    let err_step = |_n: i32| -> Result<i32, StepError> { Err(StepError("fail_f")) };
    let left_err_pipeline = err_step.and_then(g).and_then(h);
    let right_err_pipeline = err_step.and_then(g.and_then(h));

    assert_eq!(
        left_err_pipeline.apply(10),
        Err(Either::Left(Either::Left(StepError("fail_f"))))
    );
    assert_eq!(
        right_err_pipeline.apply(10),
        Err(Either::Left(StepError("fail_f")))
    );
}
