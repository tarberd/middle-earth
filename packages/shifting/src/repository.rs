//! The git repository a shift loop runs in.

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::string::FromUtf8Error;

/// A commit id, as git names it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Commit(pub String);

impl fmt::Display for Commit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A git work tree, named by its root directory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Repository {
    pub root: PathBuf,
}

/// Errors from querying git.
#[derive(thiserror::Error, Debug)]
pub enum GitError {
    #[error("failed to run git: {0}")]
    Spawn(#[source] io::Error),
    #[error("git {} failed ({status}): {stderr}", .args.join(" "))]
    Failed {
        args: &'static [&'static str],
        status: ExitStatus,
        stderr: String,
    },
    #[error("git {} wrote stdout that is not valid UTF-8", .args.join(" "))]
    InvalidStdout {
        args: &'static [&'static str],
        #[source]
        source: FromUtf8Error,
    },
}

impl Repository {
    /// The repository whose work tree contains `dir`, its root resolved through git.
    pub fn containing(dir: &Path) -> Result<Self, GitError> {
        git(dir, &["rev-parse", "--show-toplevel"]).map(|stdout| Self {
            root: PathBuf::from(OsString::from_vec(without_final_newline(stdout))),
        })
    }

    /// The commit HEAD points at.
    pub fn head(&self) -> Result<Commit, GitError> {
        const ARGS: &[&str] = &["rev-parse", "HEAD"];
        git(&self.root, ARGS).and_then(|stdout| {
            String::from_utf8(without_final_newline(stdout))
                .map(Commit)
                .map_err(|source| GitError::InvalidStdout { args: ARGS, source })
        })
    }
}

/// Runs git with `args` in `dir` and returns its stdout.
fn git(dir: &Path, args: &'static [&'static str]) -> Result<Vec<u8>, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(GitError::Spawn)?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(GitError::Failed {
            args,
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// A line of git output without the newline git ends it with.
fn without_final_newline(stdout: Vec<u8>) -> Vec<u8> {
    match stdout.strip_suffix(b"\n") {
        Some(line) => line.to_vec(),
        None => stdout,
    }
}
