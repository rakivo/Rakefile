use std::{
    env,
    result,
    str::Lines,
    process::exit,
    path::PathBuf,
    iter::Peekable,
    default::Default,
    fs::read_to_string,
};
use robuild::*;

mod error;
use error::*;

type RResult<'a, T> = result::Result::<T, RakeError<'a>>;

struct Rakefile<'a> {
    file_path: String,
    #[allow(unused)]
    jobs: Vec::<Job>,
    row: usize,
    iter: Peekable::<Lines<'a>>
}

impl Default for Rakefile<'_> {
    fn default() -> Self {
        Self {
            file_path: String::default(),
            jobs: Vec::default(),
            row: 1,
            iter: "".lines().peekable(),
        }
    }
}

impl<'a> Rakefile<'a> {
    pub const TAB_WIDTH: usize = 4;
    pub const MAX_DIR_LVL: usize = 2;
    pub const RAKE_FILE_NAME: &'static str = "Rakefile";

    fn find_rakefile() -> RResult::<'a, PathBuf> {
        let dir_path = env::current_dir().unwrap_or_else(|err| {
            eprintln!("ERROR: Failed to get current dir: {err}");
            exit(1);
        });

        let dir = Dir::new(&dir_path);
        dir.into_iter()
           .find(|f| matches!(f.file_name(), Some(name) if name == Self::RAKE_FILE_NAME))
           .ok_or_else(move || RakeError::NoRakefileInDir(dir_path))
    }

    fn pretty_file_path(file_path: &str) -> String {
        let mut count = 0;
        file_path.chars().rev().take_while(|c| {
            if *c == DELIM_CHAR { count += 1; }
            count < Self::MAX_DIR_LVL
        }).collect::<Vec::<_>>().into_iter().rev().collect()
    }

    fn parse_job(&mut self, idx: &usize, line: &str) -> RResult::<()> {
        let (target, deps_untrimmed) = line.split_at(*idx);
        let deps = deps_untrimmed.chars()
            .skip_while(|c| c.is_whitespace() || c.eq(&':'))
            .collect::<String>();

        let mut body = Vec::new();
        while let Some(next_line) = self.iter.next() {
            self.row += 1;

            // Allow people to use both tabs and spaces
            if next_line.starts_with('\t') {
                body.push(next_line.trim());
                continue
            }

            let whitespace_count = next_line.chars()
                .take_while(|c| c.is_whitespace())
                .count();

            match whitespace_count {
                Self::TAB_WIDTH => body.push(next_line.trim()),
                i @ 1.. => return Err(RakeError::InvalidIndentation(&self.file_path, i, self.row)),
                _ => break
            };
        }

        println!("\ntarget: {target}");
        println!("deps: {deps}");
        println!("body: {body:?}");

        Ok(())
    }

    fn parse_line(&mut self, line: &str) -> RResult::<()> {
        if let Some(colon_idx) = line.chars().position(|x| x == ':') {
            self.parse_job(&colon_idx, line)?;
        }
        Ok(())
    }

    fn perform() -> RResult::<'a, ()> {
        let file_path = Self::find_rakefile().unwrap_or_else(|err| {
            eprintln!("ERROR: {err}");
            exit(1);
        });

        let file_str = read_to_string(&file_path).unwrap_or_else(|err| {
            eprintln!("ERROR: Failed to `read to string` from file: {file_path}: {err}",
                      file_path = file_path.display());
            exit(1);
        });

        let mut rakefile = Rakefile {
            file_path: Self::pretty_file_path(file_path.to_str().expect("Failed to convert file path to str")),
            iter: file_str.lines().peekable(),
            ..Self::default()
        };

        while let Some(line) = rakefile.iter.next() {
            rakefile.parse_line(&line).unwrap_or_else(|err| {
                eprintln!("ERROR: {err}");
                exit(1);
            });
            rakefile.row += 1;
        }

        Ok(())
    }
}

fn main() -> RResult::<'static, ()> {
    Rakefile::perform()
}
