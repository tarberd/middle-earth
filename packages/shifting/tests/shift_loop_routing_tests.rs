use shifting::either::{Either, LoopFlow};
use shifting::script::{ScriptExit, ScriptOutput};
use shifting::shift_loop::{
    CheckExit, Exhausted, Idle, LoopEnd, LoopReport, LoopSummary, Outcome, PrecheckExit, Rejection,
    SelectExit, Settled, Task, WorkExit,
};

/// Every way a script can end other than the codes the contract gives a meaning.
const FAILURES: [ScriptExit; 5] = [
    ScriptExit::Code(1),
    ScriptExit::Code(4),
    ScriptExit::Signal(9),
    ScriptExit::InvalidStdout { code: 0 },
    ScriptExit::InvalidStdout { code: 2 },
];

#[test]
fn test_select_exit_zero_selects_its_trimmed_stdout() {
    let output = ScriptOutput::new(ScriptExit::Code(0), "  task-07 \n\n", "");

    assert_eq!(
        SelectExit::from(output),
        SelectExit::Task(Task("task-07".to_string()))
    );
}

#[test]
fn test_select_exit_two_reports_no_work_left() {
    let output = ScriptOutput::new(ScriptExit::Code(2), "", "plan is empty\n");

    assert_eq!(SelectExit::from(output), SelectExit::NoWorkLeft);
}

#[test]
fn test_select_any_other_end_fails_with_its_stderr() {
    let routed = FAILURES
        .into_iter()
        .map(|exit| SelectExit::from(ScriptOutput::new(exit, "task-07\n", "selector broke\n")))
        .collect::<Vec<_>>();

    assert_eq!(
        routed,
        FAILURES
            .into_iter()
            .map(|exit| SelectExit::Failed {
                exit,
                report: "selector broke\n".to_string(),
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_precheck_exit_zero_means_already_done_with_its_stdout() {
    let output = ScriptOutput::new(ScriptExit::Code(0), "tests already pass\n", "");

    assert_eq!(
        PrecheckExit::from(output),
        PrecheckExit::AlreadyDone("tests already pass\n".to_string())
    );
}

#[test]
fn test_precheck_any_other_end_means_the_task_needs_work() {
    let routed = FAILURES
        .into_iter()
        .chain([ScriptExit::Code(2), ScriptExit::Code(3)])
        .map(|exit| PrecheckExit::from(ScriptOutput::new(exit, "done\n", "not yet\n")))
        .collect::<Vec<_>>();

    assert_eq!(routed, vec![PrecheckExit::NeedsWork; 7]);
}

#[test]
fn test_work_exit_zero_passes_its_stdout_as_the_work_summary() {
    let output = ScriptOutput::new(ScriptExit::Code(0), "implemented task-07\n", "thinking\n");

    assert_eq!(
        WorkExit::from(output),
        WorkExit::Done("implemented task-07\n".to_string())
    );
}

#[test]
fn test_work_exit_three_declines_with_its_stdout_as_the_reason() {
    let output = ScriptOutput::new(ScriptExit::Code(3), "task-07 is wrong\n", "giving up\n");

    assert_eq!(
        WorkExit::from(output),
        WorkExit::Declined("task-07 is wrong\n".to_string())
    );
}

#[test]
fn test_work_any_other_end_fails_with_its_failure_report() {
    let failures = FAILURES
        .into_iter()
        .chain([ScriptExit::Code(2), ScriptExit::InvalidStdout { code: 3 }]);
    let routed = failures
        .clone()
        .map(|exit| WorkExit::from(ScriptOutput::new(exit, "partial\n", "agent crashed\n")))
        .chain(failures.map(|exit| WorkExit::from(ScriptOutput::new(exit, "partial\n", ""))))
        .collect::<Vec<_>>();

    assert_eq!(
        routed,
        [
            vec![WorkExit::Failed("agent crashed\n".to_string()); 7],
            vec![WorkExit::Failed("partial\n".to_string()); 7],
        ]
        .concat()
    );
}

#[test]
fn test_verify_or_judge_exit_zero_passes_with_its_stdout() {
    let output = ScriptOutput::new(ScriptExit::Code(0), "approved\n", "");

    assert_eq!(
        CheckExit::from(output),
        CheckExit::Passed("approved\n".to_string())
    );
}

#[test]
fn test_verify_or_judge_any_other_end_fails_with_its_failure_report() {
    let failures = FAILURES
        .into_iter()
        .chain([ScriptExit::Code(2), ScriptExit::Code(3)]);
    let routed = failures
        .clone()
        .map(|exit| CheckExit::from(ScriptOutput::new(exit, "summary\n", "tests failed\n")))
        .chain(failures.map(|exit| CheckExit::from(ScriptOutput::new(exit, "summary\n", ""))))
        .collect::<Vec<_>>();

    assert_eq!(
        routed,
        [
            vec![CheckExit::Failed("tests failed\n".to_string()); 7],
            vec![CheckExit::Failed("summary\n".to_string()); 7],
        ]
        .concat()
    );
}

#[test]
fn test_loop_summary_counts_each_outcome() {
    let summary = [
        Outcome::Verified,
        Outcome::Rejected(Rejection::VerifyFailed),
        Outcome::AlreadyDone,
        Outcome::Verified,
        Outcome::Rejected(Rejection::WorkFailed),
        Outcome::Rejected(Rejection::JudgeFailed),
        Outcome::Rejected(Rejection::WorkFailed),
        Outcome::Declined,
    ]
    .into_iter()
    .fold(LoopSummary::default(), LoopSummary::record);

    assert_eq!(
        summary,
        LoopSummary {
            commits: 2,
            already_done: 1,
            work_failed: 2,
            verify_failed: 1,
            judge_failed: 1,
            declines: 1,
        }
    );
    assert_eq!(summary.iterations(), 8);
}

#[test]
fn test_empty_loop_summary_counts_nothing() {
    assert_eq!(LoopSummary::default().iterations(), 0);
}

/// An idle loop before its fourth iteration, after two commits and a work failure.
const BEFORE_FOURTH: Idle = Idle {
    iteration: 4,
    summary: LoopSummary {
        commits: 2,
        already_done: 0,
        work_failed: 1,
        verify_failed: 0,
        judge_failed: 0,
        declines: 0,
    },
};

#[test]
fn test_a_committed_or_rejected_iteration_continues_to_the_next_one() {
    let flows = [
        Outcome::Verified,
        Outcome::AlreadyDone,
        Outcome::Rejected(Rejection::JudgeFailed),
    ]
    .map(|outcome| {
        BEFORE_FOURTH.advance(Either::Left(Settled {
            task: Task("task-07".to_string()),
            outcome,
        }))
    });

    assert_eq!(
        flows,
        [
            LoopFlow::Continue(Idle {
                iteration: 5,
                summary: LoopSummary {
                    commits: 3,
                    ..BEFORE_FOURTH.summary
                },
            }),
            LoopFlow::Continue(Idle {
                iteration: 5,
                summary: LoopSummary {
                    already_done: 1,
                    ..BEFORE_FOURTH.summary
                },
            }),
            LoopFlow::Continue(Idle {
                iteration: 5,
                summary: LoopSummary {
                    judge_failed: 1,
                    ..BEFORE_FOURTH.summary
                },
            }),
        ]
    );
}

#[test]
fn test_a_declined_iteration_ends_the_loop_declined_with_its_task() {
    let flow = BEFORE_FOURTH.advance(Either::Left(Settled {
        task: Task("task-07".to_string()),
        outcome: Outcome::Declined,
    }));

    assert_eq!(
        flow,
        LoopFlow::Break(LoopReport {
            end: LoopEnd::Declined {
                task: Task("task-07".to_string()),
            },
            summary: LoopSummary {
                declines: 1,
                ..BEFORE_FOURTH.summary
            },
        })
    );
}

#[test]
fn test_select_reporting_no_work_left_ends_the_loop_exhausted() {
    assert_eq!(
        BEFORE_FOURTH.advance(Either::Right(Exhausted)),
        LoopFlow::Break(LoopReport {
            end: LoopEnd::Exhausted,
            summary: BEFORE_FOURTH.summary,
        })
    );
}

#[test]
fn test_an_iteration_within_the_bound_runs() {
    assert_eq!(
        [None, Some(4), Some(9)]
            .map(|max_iterations| BEFORE_FOURTH.within(max_iterations))
            .to_vec(),
        vec![Either::Left(BEFORE_FOURTH); 3]
    );
}

#[test]
fn test_an_iteration_past_the_bound_ends_the_loop_at_max_iterations() {
    let first = Idle {
        iteration: 1,
        summary: LoopSummary::default(),
    };

    assert_eq!(
        [BEFORE_FOURTH.within(Some(3)), first.within(Some(0))],
        [
            Either::Right(LoopReport {
                end: LoopEnd::MaxIterations,
                summary: BEFORE_FOURTH.summary,
            }),
            Either::Right(LoopReport {
                end: LoopEnd::MaxIterations,
                summary: LoopSummary::default(),
            }),
        ]
    );
}
