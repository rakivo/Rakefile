use std::{
    env,
    result,
    str::Lines,
    path::PathBuf,
    process::exit,
    iter::Peekable,
    default::Default,
    fs::read_to_string,
};
use regex::*;
use robuild::*;

mod error;
use error::*;
mod ss;
use ss::*;

type RResult<'a, T> = result::Result::<T, RakeError<'a>>;

struct Rakefile<'a> {
    row: usize,
    deps_re: Regex,
    file_path: String,
    jobs: Vec::<(String, Job)>,
    iter: Peekable::<Lines<'a>>
}

impl Default for Rakefile<'_> {
    fn default() -> Self {
        Self {
            row: 1,
            deps_re: Regex::new("").unwrap(),
            file_path: String::default(),
            jobs: Vec::default(),
            iter: "".lines().peekable(),
        }
    }
}

impl<'a> Rakefile<'a> {
    pub const TAB_WIDTH: usize = 4;
    pub const MAX_DIR_LVL: usize = 2;

    pub const RAKE_FILE_NAME: &'static str = "Rakefile";
    pub const DEPS_REGEX: &'static str = r"\$d\[(.*?)\]";

    pub const PHONY: &'static str = ".PHONY";

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
    }

    #[inline]
    fn append_job(&mut self, job: Job) {
        let target = job.target().to_owned();
        self.jobs.push((target, job));
    }

    fn parse_deps_ss(&self, line: &str, deps: &Vec::<&str>) -> RResult::<String> {
        for caps in self.deps_re.captures_iter(&line) {
            let idx = caps[1].parse::<usize>().unwrap_or(0);
            if deps.get(idx).is_none() {
                return Err(RakeError::DepsIndexOutOfBounds(Info::from(self)));
            }
        }

        let deps = self.deps_re.replace_all(&line, |caps: &Captures| {
            let idx = caps[1].parse::<usize>().unwrap_or(0);
            deps[idx]
        }).to_string();

        Ok(deps)
    }

    fn parse_special_symbols
    (
        &self,
        target:      &str,
        deps_joined: &str,
        deps: &Vec::<&str>,
        line: &mut String
    ) -> RResult::<()>
    {
        *line = self.parse_deps_ss(&line, &deps)?;

        sreplace!(line, MakeTarget, &target);
        sreplace!(line, RakeTarget, &target);

        if line.contains(&SSymbol::MakeDep.to_string())
        || line.contains(&SSymbol::RakeDep.to_string())
        {
            let Some(first_dep) = deps.get(0) else {
                return Err(RakeError::DepsSSwithoutDeps(Info::from(self)))
            };
            sreplace!(line, MakeDep, &first_dep);
            sreplace!(line, RakeDep, &first_dep);
        }

        sreplace!(line, MakeDeps, &deps_joined);
        sreplace!(line, RakeDeps, &deps_joined);

        Ok(())
    }

    // fn find_job_by_key(&self, key: &str) -> Option::<&Job> {
    //     self.jobs.iter()
    //         .find(|(k, _)| k.eq(key))
    //         .map(|(_, j)| j)
    // }

    #[inline]
    fn find_job_by_key_mut(&mut self, key: &str) -> Option::<&mut Job> {
        self.jobs.iter_mut()
            .find(|(k, _)| k.eq(key))
            .map(|(_, j)| j)
    }

    fn parse_job(&mut self, idx: &usize, line: &str) -> RResult::<()> {
        let (target_untrimmed, deps_untrimmed) = line.split_at(*idx);
        let target = target_untrimmed.trim();

        if target.is_empty() {
            return Err(RakeError::NoTarget(Info::from(self)))
        }

        let deps_str = deps_untrimmed.chars()
            .skip_while(|c| c.is_whitespace() || c.eq(&':'))
            .collect::<String>();

        let deps = deps_str
            .split_whitespace()
            .collect::<Vec::<_>>();

        println!("{deps:?}");

        let deps_joined = deps.join(" ");

        let mut body = Vec::new();
        while let Some(next_line) = self.iter.peek() {
            self.row += 1;

            let mut next_line = (*next_line).to_owned();
            self.parse_special_symbols(&target, &deps_joined,
                                       &deps, &mut next_line)
                .unwrap_or_report();

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
                i @ 1.. => return Err(RakeError::InvalidIndentation(Info::from(self), i)),
                _ => if trimmed.is_empty() { self.advance(); } else { break }
            };
        }

        let cmd = body.iter().fold(RobCommand::new(), |mut cmd, line| {
            cmd.append_mv(&[line]);
            cmd
        });

        let phony = target == Self::PHONY;
        let mut job = Job::new(target, deps, cmd);
        job.phony(phony);

        self.append_job(job);

        Ok(())
    }

    fn job_as_dep_check(&mut self, job: &Job) -> IoResult::<()> {
        for dep in job.deps().iter() {
            if let Some(dep_job) = self.find_job_by_key_mut(dep) {
                dep_job.execute_async()?;
                let job = dep_job.to_owned();
                self.job_as_dep_check(&job)?;
            } else if !Rob::is_file(dep) {
                // TBD: proper error reporting here.
                eprintln!("You did a fucky wucky, mister programmer ;-)");
                eprintln!("This dependency: `{dep}`, nor defined job nor file ._.");
                exit(1);
            }
        }
        Ok(())
    }

    fn execute_job(&mut self) -> IoResult::<()> {
        // Borrow checker SeemsGood
        if self.jobs.first().is_some() {
            let job = self.jobs[0].1.to_owned();
            self.job_as_dep_check(&job)?;
            let job = &mut self.jobs[0].1;
            job.execute_async()?;
        } Ok(())
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
            deps_re: Regex::new(Self::DEPS_REGEX).unwrap(),
            file_path: Self::pretty_file_path(file_path.to_str().expect("Failed to convert file path to str")),
            iter: file_str.lines().peekable(),
            ..Self::default()
        };

        while let Some(line) = rakefile.iter.next() {
            rakefile.parse_line(&line).unwrap_or_report();
        }

        rakefile.execute_job().unwrap_or_report();
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
