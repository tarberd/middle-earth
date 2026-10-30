use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use clap::Parser;
use shifting::branch::Branch;
use shifting::cli::{self, Cli};
use shifting::default_loop::{DefaultShiftError, DefaultShiftLoop, Idle, PlanBranch, ShiftSummary};
use shifting::either::{Either, LoopFlow};
use shifting::script::{ScriptError, ScriptExit};
use shifting::step::Step;

fn create_executable_script(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o755)
        .open(&path)
        .expect("failed to open script file");
    file.write_all(content.as_bytes())
        .expect("failed to write script content");
    file.sync_all().expect("failed to sync script content");
}

fn create_mock_shifting_dir(dir: &Path) {
    create_executable_script(dir, "plan.sh", "#!/bin/sh\nexit 2\n");
    create_executable_script(dir, "work.sh", "#!/bin/sh\ncat\n");
    create_executable_script(dir, "verify.sh", "#!/bin/sh\nexit 0\n");
    create_executable_script(dir, "commit.sh", "#!/bin/sh\nexit 0\n");
    create_executable_script(dir, "rollback.sh", "#!/bin/sh\nexit 0\n");
}

#[test]
fn test_multi_cycle_execution_full_flow() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();

    let counter_file = base.join("task_counter.txt");
    std::fs::write(&counter_file, "1").expect("failed to write initial counter");

    let work_log = base.join("work.log");
    let commit_log = base.join("commit.log");
    let rollback_log = base.join("rollback.log");

    let counter_path_str = counter_file.to_str().expect("valid counter path");
    let work_log_str = work_log.to_str().expect("valid work log path");
    let commit_log_str = commit_log.to_str().expect("valid commit log path");
    let rollback_log_str = rollback_log.to_str().expect("valid rollback log path");

    // plan.sh:
    // counter 1 -> outputs "task-1", sets counter to 2, exits 0
    // counter 2 -> outputs "task-2", sets counter to 3, exits 0
    // counter 3 -> exits 2 (clean break)
    let plan_script = format!(
        "#!/bin/sh\n\
         VAL=$(cat \"{counter_path_str}\")\n\
         if [ \"$VAL\" = \"1\" ]; then\n\
             echo \"2\" > \"{counter_path_str}\"\n\
             echo \"task-1\"\n\
             exit 0\n\
         elif [ \"$VAL\" = \"2\" ]; then\n\
             echo \"3\" > \"{counter_path_str}\"\n\
             echo \"task-2\"\n\
             exit 0\n\
         else\n\
             exit 2\n\
         fi\n"
    );
    create_executable_script(base, "plan.sh", &plan_script);

    // work.sh: reads stdin, logs work, prints result to stdout
    let work_script = format!(
        "#!/bin/sh\n\
         TASK=$(cat)\n\
         echo \"done: $TASK\" >> \"{work_log_str}\"\n\
         echo \"result-$TASK\"\n"
    );
    create_executable_script(base, "work.sh", &work_script);

    // verify.sh:
    // result-task-1 -> fails with exit 1 and stderr message
    // result-task-2 -> succeeds with exit 0 and stdout message
    let verify_script = "#!/bin/sh\n\
         INPUT=$(cat)\n\
         case \"$INPUT\" in\n\
             *task-1*) echo \"verify error on task-1\" >&2; exit 1 ;;\n\
             *task-2*) echo \"verified task-2\"; exit 0 ;;\n\
             *) echo \"unknown\" >&2; exit 1 ;;\n\
         esac\n";
    create_executable_script(base, "verify.sh", verify_script);

    // commit.sh: logs committed output
    let commit_script = format!(
        "#!/bin/sh\n\
         INPUT=$(cat)\n\
         echo \"committed: $INPUT\" >> \"{commit_log_str}\"\n\
         exit 0\n"
    );
    create_executable_script(base, "commit.sh", &commit_script);

    // rollback.sh: logs rolled back error
    let rollback_script = format!(
        "#!/bin/sh\n\
         INPUT=$(cat)\n\
         echo \"rolled_back: $INPUT\" >> \"{rollback_log_str}\"\n\
         exit 0\n"
    );
    create_executable_script(base, "rollback.sh", &rollback_script);

    let shift_loop = DefaultShiftLoop::discover(base).expect("discovery should succeed");
    let summary = shift_loop.run().expect("shift loop should complete Ok");
    assert_eq!(summary.iterations, 2);

    // Verify work log executed task-1 and task-2
    let work_content = std::fs::read_to_string(&work_log).expect("read work log");
    assert_eq!(work_content, "done: task-1\ndone: task-2\n");

    // Verify rollback log captured verify failure on task-1
    let rollback_content = std::fs::read_to_string(&rollback_log).expect("read rollback log");
    assert_eq!(
        rollback_content.trim(),
        "rolled_back: verify error on task-1"
    );

    // Verify commit log captured verify pass on task-2
    let commit_content = std::fs::read_to_string(&commit_log).expect("read commit log");
    assert_eq!(commit_content.trim(), "committed: verified task-2");

    // Verify counter finished at 3
    let final_counter = std::fs::read_to_string(&counter_file).expect("read counter");
    assert_eq!(final_counter.trim(), "3");
}

#[test]
fn test_plan_exit_2_categorical_branching_routes_to_break() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();

    create_executable_script(base, "plan_exit_2.sh", "#!/bin/sh\nexit 2\n");
    create_executable_script(base, "plan_exit_0.sh", "#!/bin/sh\necho 'work payload'\n");
    create_executable_script(
        base,
        "plan_exit_1.sh",
        "#!/bin/sh\necho 'failed to plan' >&2\nexit 1\n",
    );

    let plan_exit_2 = base.join("plan_exit_2.sh");
    let plan_exit_0 = base.join("plan_exit_0.sh");
    let plan_exit_1 = base.join("plan_exit_1.sh");

    let branch_exit_2 = PlanBranch::new(&plan_exit_2);
    let res_exit_2 = branch_exit_2
        .evaluate(Idle::default())
        .expect("exit 2 evaluate ok");
    assert_eq!(res_exit_2, Either::Right(LoopFlow::Break(())));

    let branch_exit_0 = PlanBranch::new(&plan_exit_0);
    let res_exit_0 = branch_exit_0
        .evaluate(Idle::default())
        .expect("exit 0 evaluate ok");
    assert_eq!(res_exit_0, Either::Left("work payload\n".to_string()));

    let branch_exit_1 = PlanBranch::new(&plan_exit_1);
    let res_exit_1 = branch_exit_1.evaluate(Idle::default());
    assert!(
        matches!(res_exit_1, Err(ScriptError::Failed { exit: ScriptExit::Code(1), ref report }) if report.contains("failed to plan")),
        "exit 1 must return Failed typed error"
    );
}

#[test]
fn test_plan_exit_2_immediate_clean_break() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();

    let side_effect_file = base.join("side_effect.txt");
    let side_effect_str = side_effect_file.to_str().expect("valid path");

    create_executable_script(base, "plan.sh", "#!/bin/sh\nexit 2\n");
    create_executable_script(
        base,
        "work.sh",
        &format!("#!/bin/sh\necho 'work' >> \"{side_effect_str}\"\n"),
    );
    create_executable_script(
        base,
        "verify.sh",
        &format!("#!/bin/sh\necho 'verify' >> \"{side_effect_str}\"\n"),
    );
    create_executable_script(
        base,
        "commit.sh",
        &format!("#!/bin/sh\necho 'commit' >> \"{side_effect_str}\"\n"),
    );
    create_executable_script(
        base,
        "rollback.sh",
        &format!("#!/bin/sh\necho 'rollback' >> \"{side_effect_str}\"\n"),
    );

    let shift_loop = DefaultShiftLoop::discover(base).expect("discover ok");
    let summary = shift_loop.run().expect("shift loop ok");
    assert_eq!(summary.iterations, 0);

    // No side effects should exist because work.sh was never invoked
    assert!(!side_effect_file.exists());
}

#[test]
fn test_max_iterations_bounding() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();

    let counter_file = base.join("iterations.txt");
    let counter_str = counter_file.to_str().expect("valid path");
    std::fs::write(&counter_file, "0").expect("write counter");

    // plan.sh increments counter and always outputs work (exit 0)
    let plan_script = format!(
        "#!/bin/sh\n\
         COUNT=$(cat \"{counter_str}\")\n\
         COUNT=$((COUNT + 1))\n\
         echo \"$COUNT\" > \"{counter_str}\"\n\
         echo \"iteration-$COUNT\"\n\
         exit 0\n"
    );
    create_executable_script(base, "plan.sh", &plan_script);
    create_executable_script(base, "work.sh", "#!/bin/sh\ncat\n");
    create_executable_script(base, "verify.sh", "#!/bin/sh\nexit 0\n");
    create_executable_script(base, "commit.sh", "#!/bin/sh\nexit 0\n");
    create_executable_script(base, "rollback.sh", "#!/bin/sh\nexit 0\n");

    let shift_loop = DefaultShiftLoop::discover(base)
        .expect("discover ok")
        .with_max_iterations(Some(3));

    let summary = shift_loop.run().expect("shift loop ok");
    assert_eq!(summary.iterations, 3, "must report exactly 3 iterations");

    let count = std::fs::read_to_string(&counter_file).expect("read counter");
    assert_eq!(count.trim(), "3", "must have run exactly 3 iterations");
}

#[test]
fn test_max_iterations_zero() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();
    create_mock_shifting_dir(base);

    let shift_loop = DefaultShiftLoop::discover(base)
        .expect("discover ok")
        .with_max_iterations(Some(0));

    let summary = shift_loop.run().expect("shift loop ok");
    assert_eq!(summary.iterations, 0);
}

#[test]
fn test_script_discovery_validation() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();

    // 1. Missing directory
    let non_existent = base.join("non_existent_dir");
    let err_dir = DefaultShiftLoop::discover(&non_existent);
    assert!(
        matches!(err_dir, Err(DefaultShiftError::DirectoryNotFound(p)) if p == non_existent),
        "must return DirectoryNotFound"
    );

    // 2. Directory exists but missing scripts
    let empty_dir = base.join("empty_dir");
    std::fs::create_dir(&empty_dir).expect("create empty dir");
    let err_missing = DefaultShiftLoop::discover(&empty_dir);
    assert!(
        matches!(err_missing, Err(DefaultShiftError::MissingScript(p)) if p == empty_dir.join("plan.sh")),
        "must return MissingScript for plan.sh"
    );

    // 3. Partial scripts present
    create_executable_script(&empty_dir, "plan.sh", "#!/bin/sh\nexit 2\n");
    let err_missing_work = DefaultShiftLoop::discover(&empty_dir);
    assert!(
        matches!(err_missing_work, Err(DefaultShiftError::MissingScript(p)) if p == empty_dir.join("work.sh")),
        "must return MissingScript for work.sh"
    );

    // 4. All scripts present
    create_executable_script(&empty_dir, "work.sh", "#!/bin/sh\ncat\n");
    create_executable_script(&empty_dir, "verify.sh", "#!/bin/sh\nexit 0\n");
    create_executable_script(&empty_dir, "commit.sh", "#!/bin/sh\nexit 0\n");
    create_executable_script(&empty_dir, "rollback.sh", "#!/bin/sh\nexit 0\n");
    let ok_discover = DefaultShiftLoop::discover(&empty_dir);
    assert!(ok_discover.is_ok());
}

#[test]
fn test_cli_argument_parsing() {
    // Default parsing
    let cli_default = Cli::try_parse_from(["shift"]).expect("parse default");
    assert_eq!(cli_default.dir, PathBuf::from(".shifting"));
    assert_eq!(cli_default.max_iterations, None);
    assert!(!cli_default.verbose);

    // Long arguments
    let cli_long = Cli::try_parse_from([
        "shift",
        "--dir",
        "/custom/scripts",
        "--max-iterations",
        "42",
        "--verbose",
    ])
    .expect("parse long");
    assert_eq!(cli_long.dir, PathBuf::from("/custom/scripts"));
    assert_eq!(cli_long.max_iterations, Some(42));
    assert!(cli_long.verbose);

    // Short arguments
    let cli_short =
        Cli::try_parse_from(["shift", "-d", "custom_dir", "-m", "10", "-v"]).expect("parse short");
    assert_eq!(cli_short.dir, PathBuf::from("custom_dir"));
    assert_eq!(cli_short.max_iterations, Some(10));
    assert!(cli_short.verbose);
}

#[test]
fn test_cli_run_flow_with_mock_scripts() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();
    create_mock_shifting_dir(base);

    let cli_args = Cli {
        dir: base.to_path_buf(),
        max_iterations: Some(5),
        verbose: true,
    };

    let mut stdout_buffer = Vec::new();
    let summary = cli::run(cli_args, &mut stdout_buffer).expect("cli run ok");
    assert_eq!(summary.iterations, 0);

    let stdout_str = String::from_utf8(stdout_buffer).expect("valid utf-8 output");
    assert_eq!(stdout_str, "Shift completed successfully (0 iterations).\n");
}

#[test]
fn test_cli_binary_execution() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();
    create_mock_shifting_dir(base);

    let binary_path = env!("CARGO_BIN_EXE_shift");
    let output = Command::new(binary_path)
        .args(["--dir", base.to_str().expect("valid path"), "--verbose"])
        .output()
        .expect("failed to execute shift binary");

    assert!(output.status.success(), "binary should exit with status 0");
    let stdout = String::from_utf8(output.stdout).expect("valid utf-8 stdout");
    assert!(
        stdout.contains("Shift completed successfully (0 iterations)."),
        "stdout should report completion: {stdout}"
    );

    let stderr = String::from_utf8(output.stderr).expect("valid utf-8 stderr");
    assert!(
        stderr.contains("[shift] iteration 0:"),
        "stderr should contain verbose log: {stderr}"
    );
}

#[test]
fn test_default_repo_shifting_directory_works_out_of_box() {
    let repo_shifting = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.shifting");
    assert!(
        repo_shifting.is_dir(),
        ".shifting directory must exist in repo root: {}",
        repo_shifting.display()
    );

    let shift_loop = DefaultShiftLoop::discover(&repo_shifting).expect(".shifting must be valid");
    let result = shift_loop.run();
    assert!(result.is_ok(), ".shifting must run out of the box");
}

#[test]
fn test_rollback_is_minimal_non_destructive_stub() {
    let rollback_file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.shifting/rollback.sh");
    assert!(rollback_file.is_file(), "rollback.sh must exist");

    let content = std::fs::read_to_string(&rollback_file).expect("read rollback.sh");
    assert!(
        content.contains("exit 0"),
        "rollback.sh must be a minimal stub exiting 0"
    );

    let perms = std::fs::metadata(&rollback_file)
        .expect("metadata")
        .permissions();
    assert!(perms.mode() & 0o111 != 0, "rollback.sh must be executable");
}

#[test]
fn test_step_impl_for_default_shift_loop() {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let base = temp_dir.path();
    create_mock_shifting_dir(base);

    let shift_loop = DefaultShiftLoop::new(base);
    let step_res = shift_loop.apply(Idle::new(0));
    assert!(step_res.is_ok());
    assert_eq!(
        step_res.expect("step ok"),
        LoopFlow::Break(ShiftSummary { iterations: 0 })
    );
}
