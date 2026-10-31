use shifting::either::{Either, LoopFlow};
use shifting::repository::Commit;
use shifting::script::{ScriptExit, ScriptOutput};
use shifting::shift_loop::{
    AlreadyDone, Approved, Attempt, CheckExit, CountOverflow, Declined, Exhausted, Idle, LoopEnd,
    LoopReport, LoopSummary, Outcome, PrecheckExit, Rejected, Rejection, SelectError, SelectExit,
    Selected, SettleFailure, Settled, Settlement, Task, Verified, WorkDone, WorkExit,
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
fn test_select_exit_zero_with_a_line_break_in_its_trimmed_stdout_stops_the_run() {
    let routed = [
        "  line one\nline two \n",
        "line one\r\nline two",
        "line one\rline two",
    ]
    .map(|stdout| SelectExit::from(ScriptOutput::new(ScriptExit::Code(0), stdout, "")));

    assert_eq!(
        routed,
        [
            "line one\nline two",
            "line one\r\nline two",
            "line one\rline two"
        ]
        .map(|stdout| SelectExit::Stop(SelectError::SeveralLines(stdout.to_string())))
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
            .map(|exit| SelectExit::Stop(SelectError::Failed {
                exit,
                report: "selector broke\n".to_string(),
            }))
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
    .try_fold(LoopSummary::default(), LoopSummary::record);

    assert_eq!(
        summary,
        Ok(LoopSummary {
            commits: 2,
            already_done: 1,
            work_failed: 2,
            verify_failed: 1,
            judge_failed: 1,
            declines: 1,
        })
    );
    assert_eq!(summary.map(|summary| summary.iterations()), Ok(8));
}

/// Every outcome, each counted by its own count.
const OUTCOMES: [Outcome; 6] = [
    Outcome::Verified,
    Outcome::AlreadyDone,
    Outcome::Rejected(Rejection::WorkFailed),
    Outcome::Rejected(Rejection::VerifyFailed),
    Outcome::Rejected(Rejection::JudgeFailed),
    Outcome::Declined,
];

/// A summary whose every count is at its maximum.
const FULL: LoopSummary = LoopSummary {
    commits: u64::MAX,
    already_done: u64::MAX,
    work_failed: u64::MAX,
    verify_failed: u64::MAX,
    judge_failed: u64::MAX,
    declines: u64::MAX,
};

#[test]
fn test_counting_an_outcome_past_its_maximum_is_an_overflow() {
    assert_eq!(
        OUTCOMES.map(|outcome| FULL.record(outcome)),
        OUTCOMES.map(|outcome| Err(CountOverflow::Outcome(outcome)))
    );
}

#[test]
fn test_iterations_total_every_count_without_overflowing() {
    assert_eq!(FULL.iterations(), 110_680_464_442_257_309_690);
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
            Ok(LoopFlow::Continue(Idle {
                iteration: 5,
                summary: LoopSummary {
                    commits: 3,
                    ..BEFORE_FOURTH.summary
                },
            })),
            Ok(LoopFlow::Continue(Idle {
                iteration: 5,
                summary: LoopSummary {
                    already_done: 1,
                    ..BEFORE_FOURTH.summary
                },
            })),
            Ok(LoopFlow::Continue(Idle {
                iteration: 5,
                summary: LoopSummary {
                    judge_failed: 1,
                    ..BEFORE_FOURTH.summary
                },
            })),
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
        Ok(LoopFlow::Break(LoopReport {
            end: LoopEnd::Declined {
                task: Task("task-07".to_string()),
            },
            summary: LoopSummary {
                declines: 1,
                ..BEFORE_FOURTH.summary
            },
        }))
    );
}

#[test]
fn test_continuing_past_the_last_iteration_number_is_an_overflow() {
    let last = Idle {
        iteration: u64::MAX,
        summary: LoopSummary::default(),
    };

    assert_eq!(
        last.advance(Either::Left(Settled {
            task: Task("task-07".to_string()),
            outcome: Outcome::Verified,
        })),
        Err(CountOverflow::Iteration)
    );
}

#[test]
fn test_ending_an_iteration_whose_count_is_full_is_an_overflow() {
    let full = Idle {
        iteration: 4,
        summary: FULL,
    };

    assert_eq!(
        OUTCOMES.map(|outcome| {
            full.advance(Either::Left(Settled {
                task: Task("task-07".to_string()),
                outcome,
            }))
        }),
        OUTCOMES.map(|outcome| Err(CountOverflow::Outcome(outcome)))
    );
}

#[test]
fn test_select_reporting_no_work_left_ends_the_loop_exhausted() {
    assert_eq!(
        BEFORE_FOURTH.advance(Either::Right(Exhausted)),
        Ok(LoopFlow::Break(LoopReport {
            end: LoopEnd::Exhausted,
            summary: BEFORE_FOURTH.summary,
        }))
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

/// The fourth iteration's attempt at `task-07`, from the base commit `c0ffee`.
fn attempt() -> Attempt {
    Attempt {
        iteration: 4,
        task: Task("task-07".to_string()),
        base: Commit("c0ffee".to_string()),
    }
}

#[test]
fn test_a_selected_task_starts_the_iterations_attempt_from_its_base_commit() {
    assert_eq!(
        BEFORE_FOURTH.after_select(
            Commit("c0ffee".to_string()),
            SelectExit::Task(Task("task-07".to_string()))
        ),
        Ok(Either::Left(Selected { attempt: attempt() }))
    );
}

#[test]
fn test_select_with_no_work_left_exhausts_the_loop() {
    assert_eq!(
        BEFORE_FOURTH.after_select(Commit("c0ffee".to_string()), SelectExit::NoWorkLeft),
        Ok(Either::Right(Exhausted))
    );
}

#[test]
fn test_a_select_that_stops_the_run_is_an_error() {
    let errors = [
        SelectError::Failed {
            exit: ScriptExit::Code(1),
            report: "selector broke\n".to_string(),
        },
        SelectError::SeveralLines("line one\nline two".to_string()),
    ];

    assert_eq!(
        errors.clone().map(|error| {
            BEFORE_FOURTH.after_select(Commit("c0ffee".to_string()), SelectExit::Stop(error))
        }),
        errors.map(Err)
    );
}

#[test]
fn test_a_task_precheck_finds_done_goes_to_commit_with_precheck_stdout() {
    assert_eq!(
        Selected { attempt: attempt() }.after_precheck(PrecheckExit::AlreadyDone(
            "tests already pass\n".to_string()
        )),
        Either::Left(AlreadyDone {
            attempt: attempt(),
            output: "tests already pass\n".to_string(),
        })
    );
}

#[test]
fn test_a_task_that_needs_work_stays_selected() {
    assert_eq!(
        Selected { attempt: attempt() }.after_precheck(PrecheckExit::NeedsWork),
        Either::Right(Selected { attempt: attempt() })
    );
}

#[test]
fn test_work_done_passes_its_summary_on_to_verify() {
    assert_eq!(
        Selected { attempt: attempt() }
            .after_work(WorkExit::Done("implemented task-07\n".to_string())),
        Either::Left(WorkDone {
            attempt: attempt(),
            summary: "implemented task-07\n".to_string(),
        })
    );
}

#[test]
fn test_declined_work_goes_to_rollback_with_its_reason() {
    assert_eq!(
        Selected { attempt: attempt() }
            .after_work(WorkExit::Declined("task-07 is wrong\n".to_string())),
        Either::Right(Either::Left(Declined {
            attempt: attempt(),
            reason: "task-07 is wrong\n".to_string(),
        }))
    );
}

#[test]
fn test_failed_work_goes_to_rollback_as_work_failed() {
    assert_eq!(
        Selected { attempt: attempt() }.after_work(WorkExit::Failed("agent crashed\n".to_string())),
        Either::Right(Either::Right(Rejected {
            attempt: attempt(),
            outcome: Rejection::WorkFailed,
            report: "agent crashed\n".to_string(),
        }))
    );
}

/// Work on the attempt's task finished with this summary.
fn work_done() -> WorkDone {
    WorkDone {
        attempt: attempt(),
        summary: "implemented task-07\n".to_string(),
    }
}

/// Verify passed the attempt's work, whose summary judge receives.
fn verified() -> Verified {
    Verified {
        attempt: attempt(),
        summary: "implemented task-07\n".to_string(),
    }
}

#[test]
fn test_passing_verify_hands_the_work_summary_to_judge() {
    assert_eq!(
        work_done().after_verify(CheckExit::Passed("verify output\n".to_string())),
        Either::Left(verified())
    );
}

#[test]
fn test_failing_verify_goes_to_rollback_as_verify_failed() {
    assert_eq!(
        work_done().after_verify(CheckExit::Failed("tests failed\n".to_string())),
        Either::Right(Rejected {
            attempt: attempt(),
            outcome: Rejection::VerifyFailed,
            report: "tests failed\n".to_string(),
        })
    );
}

#[test]
fn test_passing_judge_goes_to_commit_with_its_verdict() {
    assert_eq!(
        verified().after_judge(CheckExit::Passed("approved\n".to_string())),
        Either::Left(Approved {
            attempt: attempt(),
            verdict: "approved\n".to_string(),
        })
    );
}

#[test]
fn test_failing_judge_goes_to_rollback_as_judge_failed() {
    assert_eq!(
        verified().after_judge(CheckExit::Failed("judge rejects\n".to_string())),
        Either::Right(Rejected {
            attempt: attempt(),
            outcome: Rejection::JudgeFailed,
            report: "judge rejects\n".to_string(),
        })
    );
}

#[test]
fn test_each_settled_state_gives_commit_or_rollback_its_outcome_and_stdin() {
    let settlements = [
        Settlement::from(Approved {
            attempt: attempt(),
            verdict: "approved\n".to_string(),
        }),
        Settlement::from(AlreadyDone {
            attempt: attempt(),
            output: "tests already pass\n".to_string(),
        }),
        Settlement::from(Rejected {
            attempt: attempt(),
            outcome: Rejection::VerifyFailed,
            report: "tests failed\n".to_string(),
        }),
        Settlement::from(Declined {
            attempt: attempt(),
            reason: "task-07 is wrong\n".to_string(),
        }),
    ];

    assert_eq!(
        settlements,
        [
            (Outcome::Verified, "approved\n"),
            (Outcome::AlreadyDone, "tests already pass\n"),
            (Outcome::Rejected(Rejection::VerifyFailed), "tests failed\n"),
            (Outcome::Declined, "task-07 is wrong\n"),
        ]
        .map(|(outcome, input)| Settlement {
            attempt: attempt(),
            outcome,
            input: input.to_string(),
        })
    );
}

#[test]
fn test_a_settlement_whose_script_succeeds_settles_the_iteration() {
    let settlement = Settlement {
        attempt: attempt(),
        outcome: Outcome::Rejected(Rejection::JudgeFailed),
        input: "judge rejects\n".to_string(),
    };

    assert_eq!(
        settlement.settle(ScriptOutput::new(ScriptExit::Code(0), "reset\n", "")),
        Ok(Settled {
            task: Task("task-07".to_string()),
            outcome: Outcome::Rejected(Rejection::JudgeFailed),
        })
    );
}

#[test]
fn test_a_settlement_whose_script_fails_is_a_failure_for_its_task() {
    let settlement = Settlement {
        attempt: attempt(),
        outcome: Outcome::Verified,
        input: "approved\n".to_string(),
    };
    let failures = FAILURES
        .into_iter()
        .chain([ScriptExit::Code(2), ScriptExit::Code(3)]);

    assert_eq!(
        failures
            .clone()
            .map(|exit| {
                settlement
                    .clone()
                    .settle(ScriptOutput::new(exit, "", "index.lock exists\n"))
            })
            .collect::<Vec<_>>(),
        failures
            .map(|exit| {
                Err(SettleFailure {
                    task: Task("task-07".to_string()),
                    exit,
                    report: "index.lock exists\n".to_string(),
                })
            })
            .collect::<Vec<_>>()
    );
}
