use {
    std::{
        path::PathBuf,
        fmt::{Display, Formatter},
    },
    crate::Rakefile
};

#[derive(Debug)]
pub enum RakeError<'a> {
    // File path, tab width, row
    InvalidIndent(&'a str, usize, usize),

    // Directory path
    NoRakefileInDir(PathBuf),
}

impl Display for RakeError<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        use RakeError::*;
        match self {
            InvalidIndent(file_path, width, row) =>
                write!(f, "{file_path}:{row} Invalid indent, expected: {exp}, got: {width}",
                       exp = Rakefile::TAB_WIDTH),
            NoRakefileInDir(dir) => write!(f, "No Rakefile in: {dir}", dir = dir.display()),
        }
    }
}
