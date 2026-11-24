use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeMode {
    Terminal,
    Desktop,
}

pub enum ResumeTarget {
    Cli {
        args: Vec<OsString>,
        env: Option<(&'static str, PathBuf)>,
    },
    Url(String),
}

impl ResumeTarget {
    pub fn cli(args: impl IntoIterator<Item = impl Into<OsString>>) -> Self {
        Self::cli_with_env(args, None)
    }

    pub fn cli_with_env(
        args: impl IntoIterator<Item = impl Into<OsString>>,
        env: Option<(&'static str, PathBuf)>,
    ) -> Self {
        Self::Cli {
            args: args.into_iter().map(Into::into).collect(),
            env,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResumeLaunch {
    Cli {
        program: PathBuf,
        args: Vec<OsString>,
        cwd: PathBuf,
        env: Option<(&'static str, PathBuf)>,
    },
    Url(String),
}
