use {
    std::{
        process::exit,
        path::PathBuf,
        fmt::{Display, Formatter},
    },
    crate::Rakefile
};

// File path, tab width, row
#[derive(Debug)]
pub struct Info<'a>(pub &'a str, pub usize);

impl<'a> From::<&'a Rakefile<'_>> for Info<'a> {
    #[inline]
    fn from(rake: &'a Rakefile) -> Self {
        Self(&rake.file_path, rake.row)
    }
}

impl<'a> From::<&'a mut Rakefile<'_>> for Info<'a> {
    #[inline]
    fn from(rake: &'a mut Rakefile) -> Self {
        Self::from(&*rake)
    }
}

#[derive(Debug)]
pub enum RakeError<'a> {
    InvalidIndentation(Info<'a>, usize),
    /// Directory path
    NoRakefileInDir(PathBuf),

    /// Can be happen in case of $d[index] syntax.
    DepsIndexOutOfBounds(Info<'a>),

    /// SS -> Special Symbol
    /// Can be happen in here:
    /// ```
    /// : foo.c
    ///     mkdir -p build
    ///     clang -o $t $d
    /// ```
    DepsSSwithoutDeps(Info<'a>),

    /// Target is mandatory
    NoTarget(Info<'a>)
}

impl Display for RakeError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use RakeError::*;
        let expected_tab_width = Rakefile::TAB_WIDTH;
        match self {
            InvalidIndentation(info, w) => write!(f, "{f}:{r}: Invalid indentation, expected: {expected_tab_width}, got: {w}", f = info.0, r = info.1),
            NoRakefileInDir(dir) => write!(f, "No Rakefile in: `{dir}`", dir = dir.display()),
            DepsIndexOutOfBounds(info) => write!(f, "{f}:{r}: Index out of bounds", f = info.0, r = info.1),
            DepsSSwithoutDeps(info) => write!(f, "{f}:{r}: Special `deps` syntax without deps", f = info.0, r = info.1),
            NoTarget(info) => write!(f, "{f}:{r}: Target is mandatory", f = info.0, r = info.1)
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
                eprintln!("ERROR: {e}");
                if cfg!(debug_assertions) {
                    panic!("called `Option::unwrap()` on a `None` value")
                } else {
                    exit(1)
                }
            }
        }
    }
}
