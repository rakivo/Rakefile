use std::{
    env,
    result,
    str::Lines,
    path::PathBuf,
    iter::Peekable,
    process::Output,
    default::Default,
    fs::read_to_string,
};
use robuild::*;

mod error;
use error::*;

type RResult<'a, T> = result::Result::<T, RakeError<'a>>;

struct Rakefile<'a> {
    file_path: String,
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
        let dir_path = env::current_dir().unwrap_or_report();
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

    #[inline]
    fn advance(&mut self) {
        self.iter.next();
        self.row += 1;
    }

    #[inline]
    fn append_job(&mut self, job: Job) {
        self.jobs.push(job);
    }

    // Allow people to use both Makefiles and Rakefiles
    // special symbols.
    //
    // SS -> Special Symbol
    //
    const TARGET_MAKE_SS: &'static str = "$@";
    const TARGET_RAKE_SS: &'static str = "$t";

    const DEPS_MAKE_SS: &'static str = "$d";
    const DEPS_RAKE_SS: &'static str = "$<";

    const ALL_DEPS_MAKE_SS: &'static str = "$ad";
    const ALL_DEPS_RAKE_SS: &'static str = "$^";

    fn parse_special_symbols
    (
        target:      &str,
        deps_joined: &str,
        deps: &Vec::<&str>,
        line: &mut String
    ) {
        *line = line.replace(Self::TARGET_MAKE_SS, &target);
        *line = line.replace(Self::TARGET_RAKE_SS, &target);

        *line = line.replace(Self::DEPS_MAKE_SS, &deps[0]);
        *line = line.replace(Self::DEPS_RAKE_SS, &deps[0]);

        *line = line.replace(Self::ALL_DEPS_MAKE_SS, &deps_joined);
        *line = line.replace(Self::ALL_DEPS_RAKE_SS, &deps_joined);
    }

    fn parse_job(&mut self, idx: &usize, line: &str) -> RResult::<()> {
        let (target, deps_untrimmed) = line.split_at(*idx);

        let deps_str = deps_untrimmed.chars()
            .skip_while(|c| c.is_whitespace() || c.eq(&':'))
            .collect::<String>();

        let deps = deps_str
            .split_whitespace()
            .collect::<Vec::<_>>();

        let deps_joined = deps.join(" ");

        let mut body = Vec::new();
        while let Some(next_line) = self.iter.peek() {
            let mut next_line = (*next_line).to_owned();
            Self::parse_special_symbols(&target, &deps_joined, &deps, &mut next_line);

            let trimmed = next_line.trim().to_owned();

            // Allow people to use both tabs and spaces
            if next_line.starts_with('\t') {
                body.push(trimmed);
                self.advance();
                continue
            }

            let whitespace_count = next_line.chars()
                .take_while(|c| c.is_whitespace())
                .count();

            match whitespace_count {
                Self::TAB_WIDTH => {
                    self.advance();
                    body.push(trimmed)
                }
                i @ 1.. => return Err(RakeError::InvalidIndentation(&self.file_path, i, self.row + 1)),
                _ => if trimmed.is_empty() { self.advance(); } else { break }
            };
        }

        let mut cmd = RobCommand::new();
        body.iter().for_each(|line| { cmd.append_mv(&[line]); });
        let job = Job::new(target, deps, cmd);
        self.append_job(job);

        Ok(())
    }

    fn execute_jobs(&mut self) -> IoResult::<Vec::<Vec::<Output>>> {
        let mut jobss = Vec::new();

        for job in self.jobs.iter_mut() {
            jobss.push(job.execute_async()?);
        }

        Ok(jobss)
    }

    fn parse_line(&mut self, line: &str) -> RResult::<()> {
        if let Some(colon_idx) = line.chars().position(|x| x == ':') {
            self.parse_job(&colon_idx, line)?;
        } Ok(())
    }

    fn init()  {
        let file_path = Self::find_rakefile().unwrap_or_report();
        let file_str = read_to_string(&file_path).unwrap_or_report();

        let mut rakefile = Rakefile {
            file_path: Self::pretty_file_path(file_path.to_str().expect("Failed to convert file path to str")),
            iter: file_str.lines().peekable(),
            ..Self::default()
        };

        while let Some(line) = rakefile.iter.next() {
            rakefile.parse_line(&line).unwrap_or_report();
        }

        rakefile.execute_jobs().unwrap_or_report();
    }
}

fn main() {
    Rakefile::init()
}

/* TODO:
    1. Jobs as dependencies of other jobs,
    2. Special symbols: $@, $t, $d, $<, $*, %, CC,
    3. Parse flags,
    4. Async mode,
*/
