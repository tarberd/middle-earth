use std::io::Write;
use std::path::{Path, PathBuf};

use shifting::branch::Branch;
use shifting::either::Either;
use shifting::step::{Step, StepExt};
use shifting::{ScriptBranch, ScriptOutput, ScriptStep};

fn create_executable_script(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    let path_str = path.to_str().expect("valid utf-8 path");
    let mut child = std::process::Command::new("sh")
        .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "_", path_str])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("failed to spawn script creator");
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(content.as_bytes())
            .expect("failed to write script content");
    }
    let status = child.wait().expect("failed to wait on script creator");
    assert!(status.success(), "script creator failed with {status}");
    path
}

#[test]
fn test_script_step_exit_zero_transforms_stdin_to_stdout() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "echo_upper.sh",
        "#!/bin/sh\ntr '[:lower:]' '[:upper:]'\n",
    );

    let step = ScriptStep::new(&script_path);
    let output = step
        .apply("hello world".to_string())
        .expect("step execution failed");

    assert_eq!(output, "HELLO WORLD");
}

#[test]
fn test_script_output_trait_derivations() {
    let out1 = ScriptOutput::new(0, "ok", "warn");
    let out2 = ScriptOutput {
        code: 0,
        stdout: "ok".to_string(),
        stderr: "warn".to_string(),
    };

    assert_eq!(out1, out2);
    assert_eq!(out1.clone(), out2);
}

#[test]
fn test_script_step_non_zero_exit_returns_typed_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "fail.sh",
        "#!/bin/sh\necho 'compilation failed' >&2\nexit 42\n",
    );

    let step = ScriptStep::new(&script_path);
    let result = step.apply("source code".to_string());

    assert!(
        matches!(
            result,
            Err(shifting::ScriptError::NonZeroExit { code: 42, .. })
        ),
        "expected NonZeroExit with code 42, got {result:?}"
    );
    if let Err(shifting::ScriptError::NonZeroExit { code, stderr }) = result {
        assert_eq!(code, 42);
        assert_eq!(stderr, "compilation failed\n");
    }
}

#[test]
fn test_script_step_terminated_by_signal_returns_signaled_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "kill_self.sh", "#!/bin/sh\nkill -9 $$\n");

    let step = ScriptStep::new(&script_path);
    let result = step.apply("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Signaled)),
        "expected Signaled error, got {result:?}"
    );
}

#[test]
fn test_script_step_missing_executable_returns_io_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_path = temp_dir.path().join("non_existent_script.sh");

    let step = ScriptStep::new(&missing_path);
    let result = step.apply("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Io(ref err)) if err.kind() == std::io::ErrorKind::NotFound),
        "expected NotFound Io error, got {result:?}"
    );
}

#[test]
fn test_script_branch_exit_zero_returns_left() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "verify_ok.sh",
        "#!/bin/sh\nread -r payload\necho \"verified: $payload\"\nexit 0\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("git diff".to_string());

    assert!(
        matches!(result, Ok(Either::Left(ref stdout)) if stdout == "verified: git diff\n"),
        "expected Either::Left(\"verified: git diff\\n\"), got {result:?}"
    );
}

#[test]
fn test_script_branch_non_zero_exit_returns_right_with_output() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "verify_fail.sh",
        "#!/bin/sh\necho 'test summary'\necho 'test failed details' >&2\nexit 1\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("run-tests".to_string());

    assert!(
        matches!(result, Ok(Either::Right(ref out)) if out.code == 1),
        "expected Either::Right with code 1, got {result:?}"
    );
    if let Ok(Either::Right(out)) = result {
        assert_eq!(out.code, 1);
        assert_eq!(out.stdout, "test summary\n");
        assert_eq!(out.stderr, "test failed details\n");
    }
}

#[test]
fn test_script_branch_terminated_by_signal_returns_signaled_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "kill_self_branch.sh",
        "#!/bin/sh\nkill -9 $$\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Signaled)),
        "expected Signaled error, got {result:?}"
    );
}

#[test]
fn test_script_branch_missing_executable_returns_io_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let missing_path = temp_dir.path().join("missing_branch_script.sh");

    let branch = ScriptBranch::new(&missing_path);
    let result = branch.evaluate("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Io(ref err)) if err.kind() == std::io::ErrorKind::NotFound),
        "expected NotFound Io error, got {result:?}"
    );
}

#[test]
fn test_script_step_and_branch_composition() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let work_script = create_executable_script(
        temp_dir.path(),
        "work.sh",
        "#!/bin/sh\nread -r task\necho \"done: $task\"\n",
    );
    let verify_pass_script = create_executable_script(
        temp_dir.path(),
        "verify_pass.sh",
        "#!/bin/sh\nread -r status\necho \"passed: $status\"\nexit 0\n",
    );
    let verify_fail_script = create_executable_script(
        temp_dir.path(),
        "verify_fail_comp.sh",
        "#!/bin/sh\necho 'tests failed' >&2\nexit 2\n",
    );

    let step = ScriptStep::new(&work_script);
    let branch_pass = ScriptBranch::new(&verify_pass_script);
    let branch_fail = ScriptBranch::new(&verify_fail_script);

    let pipeline_pass = step.clone().branch(branch_pass);
    let pass_res = pipeline_pass.evaluate("ticket-03".to_string());
    assert!(
        matches!(pass_res, Ok(Either::Left(ref stdout)) if stdout == "passed: done: ticket-03\n"),
        "expected Left(\"passed: done: ticket-03\\n\"), got {pass_res:?}"
    );

    let pipeline_fail = step.branch(branch_fail);
    let fail_res = pipeline_fail.evaluate("ticket-03".to_string());
    assert!(
        matches!(fail_res, Ok(Either::Right(ref out)) if out.code == 2 && out.stderr == "tests failed\n"),
        "expected Right with code 2 and stderr, got {fail_res:?}"
    );
}

#[test]
fn test_script_step_invalid_utf8_output_returns_utf8_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "invalid_utf8.sh",
        "#!/bin/sh\nprintf '\\xff\\xff'\n",
    );

    let step = ScriptStep::new(&script_path);
    let result = step.apply("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Utf8(_))),
        "expected ScriptError::Utf8, got {result:?}"
    );
    if let Err(shifting::ScriptError::Utf8(err)) = result {
        assert_eq!(err.as_bytes(), &[0xff, 0xff]);
    }
}

#[test]
fn test_script_branch_invalid_utf8_output_returns_utf8_error() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "invalid_utf8_branch.sh",
        "#!/bin/sh\nprintf '\\xff\\xff'\n",
    );

    let branch = ScriptBranch::new(&script_path);
    let result = branch.evaluate("input".to_string());

    assert!(
        matches!(result, Err(shifting::ScriptError::Utf8(_))),
        "expected ScriptError::Utf8, got {result:?}"
    );
}

#[test]
fn test_script_step_large_payload_streaming_no_deadlock() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "stream_echo.sh", "#!/bin/sh\ncat\n");

    let step = ScriptStep::new(&script_path);
    // 128KB payload exceeds default 64KB OS pipe buffer
    let payload = "x".repeat(128 * 1024);
    let output = step.apply(payload.clone()).expect("step execution failed");

    assert_eq!(output.len(), 128 * 1024);
    assert_eq!(output, payload);
}

#[test]
fn test_script_branch_large_payload_streaming_no_deadlock() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "stream_branch.sh", "#!/bin/sh\ncat\n");

    let branch = ScriptBranch::new(&script_path);
    let payload = "y".repeat(128 * 1024);
    let result = branch
        .evaluate(payload.clone())
        .expect("branch execution failed");

    assert_eq!(result, Either::Left(payload));
}

#[test]
fn test_script_error_display_and_source() {
    use std::error::Error;

    let io_err = shifting::ScriptError::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "not found",
    ));
    assert_eq!(format!("{io_err}"), "I/O error: not found");
    assert!(io_err.source().is_some());

    let non_zero = shifting::ScriptError::NonZeroExit {
        code: 1,
        stderr: "fail".to_string(),
    };
    assert_eq!(format!("{non_zero}"), "process exited with status 1: fail");
    assert!(non_zero.source().is_none());

    let signaled = shifting::ScriptError::Signaled;
    assert_eq!(format!("{signaled}"), "process terminated by signal");
    assert!(signaled.source().is_none());

    let utf8_raw_err = String::from_utf8(vec![0xff]).expect_err("expected utf8 error");
    let utf8_err = shifting::ScriptError::Utf8(utf8_raw_err);
    assert!(format!("{utf8_err}").starts_with("UTF-8 error:"));
    assert!(utf8_err.source().is_some());
}

#[test]
fn test_script_step_broken_pipe_early_exit_completes_ok() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path =
        create_executable_script(temp_dir.path(), "early_exit.sh", "#!/bin/sh\nexit 0\n");

    let step = ScriptStep::new(&script_path);
    // 512KB payload ensures pipe buffer is exceeded and BrokenPipe is encountered
    let payload = "x".repeat(512 * 1024);
    let output = step
        .apply(payload)
        .expect("step execution failed on broken pipe");

    assert_eq!(output, "");
}

#[test]
fn test_script_branch_broken_pipe_early_exit_completes_ok() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let script_path = create_executable_script(
        temp_dir.path(),
        "head_exit.sh",
        "#!/bin/sh\nhead -c 1 > /dev/null\necho 'early branch ok'\nexit 0\n",
    );

    let branch = ScriptBranch::new(&script_path);
    // 512KB payload ensures pipe buffer is exceeded and BrokenPipe is encountered
    let payload = "y".repeat(512 * 1024);
    let result = branch
        .evaluate(payload)
        .expect("branch execution failed on broken pipe");

    assert_eq!(result, Either::Left("early branch ok\n".to_string()));
}
