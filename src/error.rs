use {
    std::{
        process::exit,
        path::PathBuf,
        fmt::{Display, Formatter},
    },
    crate::Rakefile
};

#[derive(Debug)]
pub enum RakeError<'a> {
    // File path, tab width, row
    InvalidIndentation(&'a str, usize, usize),

    // Directory path
    NoRakefileInDir(PathBuf),
}

impl Display for RakeError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use RakeError::*;
        let expected = Rakefile::TAB_WIDTH;
        match self {
            InvalidIndentation(file_path, width, row) => write!(f, "{file_path}:{row}: Invalid indentation, expected: {expected}, got: {width}"),
            NoRakefileInDir(dir) => write!(f, "No Rakefile in: `{dir}`", dir = dir.display()),
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
                    exit(1);
                }
            }
        }
    }
}
