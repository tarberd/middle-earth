use std::collections::HashSet;
use std::error::Error;

use shifting::{
    branch_id, either_id, into_left, into_right, step_id, Branch, BranchIdentity, Either,
    EitherIdentity, IntoLeft, IntoRight, LoopFlow, Step, StepIdentity,
};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct TestErrA(&'static str);

impl core::fmt::Display for TestErrA {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ErrA: {}", self.0)
    }
}

impl Error for TestErrA {}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
struct TestErrB(&'static str);

impl core::fmt::Display for TestErrB {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ErrB: {}", self.0)
    }
}

impl Error for TestErrB {}

#[test]
fn test_either_left_variants_and_projections() {
    let left_val: Either<i32, String> = Either::Left(42);

    assert_eq!(left_val.as_ref(), Either::Left(&42));
    assert_eq!(left_val.left(), Some(42));

    let left_str: Either<String, Vec<u8>> = Either::Left("hello".to_string());
    assert_eq!(left_str.as_deref(), Either::Left("hello"));

    let left_for_right: Either<i32, String> = Either::Left(42);
    assert_eq!(left_for_right.right(), None);
}

#[test]
fn test_either_right_variants_and_projections() {
    let right_val: Either<i32, String> = Either::Right("failure".to_string());

    assert_eq!(right_val.as_ref(), Either::Right(&"failure".to_string()));
    assert_eq!(right_val.right(), Some("failure".to_string()));

    let right_str: Either<Vec<u8>, String> = Either::Right("failure".to_string());
    assert_eq!(right_str.as_deref(), Either::Right("failure"));

    let right_for_left: Either<i32, String> = Either::Right("failure".to_string());
    assert_eq!(right_for_left.left(), None);
}

#[test]
fn test_either_mapping_and_folding() {
    let left: Either<i32, &str> = Either::Left(10);
    let mapped_left = left.map_left(|val| val * 2);
    assert_eq!(mapped_left, Either::Left(20));

    let right: Either<i32, &str> = Either::Right("error");
    let mapped_right = right.map_right(|msg| msg.len());
    assert_eq!(mapped_right, Either::Right(5));

    let val1: Either<i32, String> = Either::Left(100);
    let folded1 = val1.either(|l| l.to_string(), |r| r);
    assert_eq!(folded1, "100");

    let val2: Either<i32, String> = Either::Right("custom".to_string());
    let folded2 = val2.either(|l| l.to_string(), |r| r);
    assert_eq!(folded2, "custom");
}

#[test]
fn test_either_into_inner() {
    let uniform_left: Either<String, String> = Either::Left("left-uniform".to_string());
    assert_eq!(uniform_left.into_inner(), "left-uniform");

    let uniform_right: Either<String, String> = Either::Right("right-uniform".to_string());
    assert_eq!(uniform_right.into_inner(), "right-uniform");
}

#[test]
fn test_either_display_and_error_impls() {
    let err_a: Either<TestErrA, TestErrB> = Either::Left(TestErrA("disk full"));
    assert_eq!(format!("{err_a}"), "ErrA: disk full");
    assert!(err_a.source().is_none());

    let err_b: Either<TestErrA, TestErrB> = Either::Right(TestErrB("permission denied"));
    assert_eq!(format!("{err_b}"), "ErrB: permission denied");
    assert!(err_b.source().is_none());

    // Verify it acts as a first-class dyn std::error::Error trait object
    let trait_object: &dyn Error = &err_a;
    assert_eq!(format!("{trait_object}"), "ErrA: disk full");
}

#[test]
fn test_either_and_loop_flow_conversions() {
    let either_left: Either<i32, String> = Either::Left(10);
    let flow_from_left: LoopFlow<i32, String> = either_left.into();
    assert_eq!(flow_from_left, LoopFlow::Continue(10));

    let either_right: Either<i32, String> = Either::Right("done".to_string());
    let flow_from_right: LoopFlow<i32, String> = either_right.into();
    assert_eq!(flow_from_right, LoopFlow::Break("done".to_string()));

    let flow_continue: LoopFlow<i32, String> = LoopFlow::Continue(42);
    let either_from_cont: Either<i32, String> = flow_continue.into();
    assert_eq!(either_from_cont, Either::Left(42));

    let flow_break: LoopFlow<i32, String> = LoopFlow::Break("halt".to_string());
    let either_from_break: Either<i32, String> = flow_break.into();
    assert_eq!(either_from_break, Either::Right("halt".to_string()));
}

#[test]
fn test_loop_flow_projections_and_collapses() {
    let cont: LoopFlow<i32, String> = LoopFlow::Continue(42);
    assert_eq!(cont.as_ref(), LoopFlow::Continue(&42));
    assert_eq!(cont.clone().continue_value(), Some(42));
    assert_eq!(cont.break_value(), None);

    let cont_str: LoopFlow<String, Vec<u8>> = LoopFlow::Continue("running".to_string());
    assert_eq!(cont_str.as_deref(), LoopFlow::Continue("running"));

    let brk: LoopFlow<i32, String> = LoopFlow::Break("terminated".to_string());
    assert_eq!(brk.as_ref(), LoopFlow::Break(&"terminated".to_string()));
    assert_eq!(brk.clone().continue_value(), None);
    assert_eq!(brk.break_value(), Some("terminated".to_string()));

    let brk_vec: LoopFlow<Vec<u8>, String> = LoopFlow::Break("terminated".to_string());
    assert_eq!(brk_vec.as_deref(), LoopFlow::Break("terminated"));
}

#[test]
fn test_canonical_injections_and_identities() {
    let inject_l: IntoLeft<i32, String> = IntoLeft::new();
    let left_step_res = Step::apply(&inject_l, 42);
    assert_eq!(left_step_res, Ok(Either::Left(42)));
    let left_branch_res = Branch::evaluate(&into_left::<i32, String>(), 42);
    assert_eq!(left_branch_res, Ok(Either::Left(42)));
    let left_either_res = Branch::evaluate(&Either::<i32, String>::into_left(), 42);
    assert_eq!(left_either_res, Ok(Either::Left(42)));

    let inject_r: IntoRight<i32, String> = IntoRight::new();
    let right_step_res = Step::apply(&inject_r, "hello".to_string());
    assert_eq!(right_step_res, Ok(Either::Right("hello".to_string())));
    let right_branch_res = Branch::evaluate(&into_right::<i32, String>(), "hello".to_string());
    assert_eq!(right_branch_res, Ok(Either::Right("hello".to_string())));
    let right_either_res = Branch::evaluate(&Either::<i32, String>::into_right(), "hello".to_string());
    assert_eq!(right_either_res, Ok(Either::Right("hello".to_string())));

    let either_identity: EitherIdentity<i32, String> = EitherIdentity::new();
    let step_res = Step::apply(&either_identity, Either::Left(99));
    assert_eq!(step_res, Ok(Either::Left(99)));
    let branch_res = Branch::evaluate(&either_identity, Either::Right("world".to_string()));
    assert_eq!(branch_res, Ok(Either::Right("world".to_string())));

    let either_id_sugar = Either::<i32, String>::id();
    assert_eq!(Step::apply(&either_id_sugar, Either::Left(1)), Ok(Either::Left(1)));
    assert_eq!(either_id::<i32, String>(), either_identity);

    let branch_id_sugar: BranchIdentity<i32, String> = branch_id();
    assert_eq!(
        Branch::evaluate(&branch_id_sugar, Either::Right("sugar".to_string())),
        Ok(Either::Right("sugar".to_string()))
    );
}

#[test]
fn test_trait_derivations_and_hashing() {
    let e1: Either<i32, &str> = Either::Left(1);
    let e2: Either<i32, &str> = Either::Right("b");
    let set = HashSet::from([e1, e2]);
    assert_eq!(set.len(), 2);

    let f1: LoopFlow<i32, &str> = LoopFlow::Continue(1);
    let f2: LoopFlow<i32, &str> = LoopFlow::Break("halt");
    let flow_set = HashSet::from([f1, f2]);
    assert_eq!(flow_set.len(), 2);

    let id = StepIdentity::<i32>::new();
    let id_clone = id;
    assert_eq!(id, id_clone);
    assert_eq!(id, step_id::<i32>());
    assert_eq!(id, StepIdentity::<i32>::id());

    let either_id_val = EitherIdentity::<i32, String>::new();
    let either_id_clone = either_id_val;
    assert_eq!(either_id_val, either_id_clone);
    assert_eq!(either_id_val, EitherIdentity::<i32, String>::id());

    let inj_l = IntoLeft::<i32, String>::new();
    let inj_r = IntoRight::<i32, String>::new();
    assert_eq!(inj_l, inj_l);
    assert_eq!(inj_r, inj_r);
}
