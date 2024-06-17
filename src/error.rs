use std::{
    process::exit,
    fmt::{Display, Formatter},
};
use robuild::*;
use crate::Rakefile;

// File path, row
#[derive(Eq, Hash, Debug, Clone, PartialEq)]
pub struct Info(pub String, pub usize);

impl From::<&Rakefile<'_>> for Info {
    #[inline]
    fn from(rake: &Rakefile) -> Self {
        Self(rake.file_path.display().to_string(), rake.row)
    }
}

impl From::<(&Rakefile<'_>, usize)> for Info {
    #[inline]
    fn from(rrow: (&Rakefile, usize)) -> Self {
        let mut info = Self::from(rrow.0);
        info.1 = rrow.1;
        info
    }
}

#[derive(Debug)]
pub enum RakeError {
    FailedToExecute(Info),

    InvalidIndentation(Info, usize),

    InvalidDependency(Info, String),

    /// Directory path
    NoRakefileInDir(String),

    /// Can be happen in case of $d[index] syntax.
    DepsIndexOutOfBounds(Info, usize),

    /// SS -> Special Symbol
    /// Can be happen in here:
    /// ```
    /// : foo.c
    ///     mkdir -p build
    ///     clang -o $t $d
    /// ```
    DepsSSwithoutDeps(Info),

    /// Target is mandatory
    NoTarget(Info),

    MultipleNames(Info),

    InvalidValue(Info, String),

    InvalidUseOfFlag(String, Vec::<String>),

    InvalidFlag(String),
}

impl Display for RakeError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        use RakeError::*;
        let expected_tab_width = Rakefile::TAB_WIDTH;
        match self {
            FailedToExecute(info)           => write!(f, "{f}:{r}: Failed to execute job", f = info.0, r = info.1),
            InvalidIndentation(info, w)     => write!(f, "{f}:{r}: Invalid indentation, expected: {expected_tab_width}, got: {w}", f = info.0, r = info.1),
            InvalidDependency(info, dep)    => write!(f, "{f}:{r}: Dependency: `{dep}` nor a defined job, nor existing file, nor directory", f = info.0, r = info.1),
            NoRakefileInDir(dir)            => write!(f, "No Rakefile in: `{dir}`"),
            DepsIndexOutOfBounds(info, len) => write!(f, "{f}:{r}: Index out of bounds, NOTE: treat your deps as zero-indexed array. Length of your deps-array is: {len}", f = info.0, r = info.1),
            DepsSSwithoutDeps(info)         => write!(f, "{f}:{r}: Special `deps` syntax without deps", f = info.0, r = info.1),
            NoTarget(info)                  => write!(f, "{f}:{r}: Target is mandatory", f = info.0, r = info.1),
            MultipleNames(info)             => write!(f, "{f}:{r}: Provide only one name of the variable", f = info.0, r = info.1),
            InvalidValue(info, value)       => write!(f, "{f}:{r}: Invalid value: {value}", f = info.0, r = info.1),
            InvalidUseOfFlag(flag, args)    => write!(f, "Invalid use of flag: `{flag}`, arg: {args}", args = args.join(" ")),
            InvalidFlag(flag)               => write!(f, "Unsupported flag: `{flag}`, supported flags: `-k`, `-s`, `-C`")
        }
    }
}

// I decided to implement this kinda method to be able to report errors
// in a more pretty and neat way in release mode, while keeping all the
// necessary information about the caller and shit in debug mode.

pub trait UnwrapOrReport<T> {
    fn unwrap_or_report(self) -> T;
}

impl<T, E> UnwrapOrReport<T> for Result<T, E>
where
    E: Display
{
    #[inline]
    #[track_caller]
    fn unwrap_or_report(self) -> T {
        match self {
            Ok(t) => t,
            Err(e) => {
                eprintln!("{lvl} {e}", lvl = LogLevel::PANIC);
                if cfg!(debug_assertions) {
                    panic!("called `Option::unwrap()` on a `None` value")
                } else {
                    exit(1)
                }
            }
        }
    }
}
