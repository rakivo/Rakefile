use std::{
    env,
    result,
    str::Lines,
    io::ErrorKind,
    path::PathBuf,
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
    cfg: Config,
    deps_re: Regex,
    file_path: String,
    jobs: Vec::<Job>,
    iter: Peekable::<Lines<'a>>
}

/* TODO:
    RakeJob struct, that will contain robuild::Job and
    info about that struct, like row or something.
*/

impl Default for Rakefile<'_> {
    fn default() -> Self {
        Self {
            row: 1,
            cfg: Config::default(),
            deps_re: Regex::new("").unwrap(),
            iter: "".lines().peekable(),
            file_path: String::default(),
            jobs: Vec::default(),
        }
    }
}

impl<'a> Rakefile<'a> {
    pub const TAB_WIDTH: usize = 4;
    pub const MAX_DIR_LVL: usize = 2;

    pub const RAKE_FILE_NAME: &'static str = "Rakefile";
    pub const DEPS_REGEX: &'static str = r"\$d\[(.*?)\]";

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
        self.jobs.push(job);
    }

    fn parse_deps_ss(&self, line: &str, deps: &Vec::<&str>) -> RResult::<String> {
        for caps in self.deps_re.captures_iter(&line) {
            let idx = caps[1].parse::<usize>().unwrap_or(0);
            if deps.get(idx).is_none() {
                return Err(RakeError::DepsIndexOutOfBounds(Info::from(self), deps.len()));
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

    // TBD: Non linear search
    #[inline]
    fn find_job_by_target_mut(&mut self, target: &str) -> Option::<&mut Job> {
        self.jobs.iter_mut()
            .find(|j| j.target().eq(target))
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

        let cmd = body.iter()
            .fold(RobCommand::from(self.cfg.to_owned()), |mut cmd, line| {
                cmd.append_mv(&[line]);
                cmd
            });

        let ss_check1 = parse_special_job_by_target!(self, target, deps, cmd, phony, true, SSymbol::MakePhony, SSymbol::RakePhony);
        let ss_check2 = parse_special_job_by_target!(self, target, deps, cmd, echo, false, SSymbol::MakeSilent);

        if !(ss_check1 && ss_check2) {
            let job = Job::new(target, deps, cmd);
            self.append_job(job);
        }

        Ok(())
    }

    // TBD: Return row of the job that has invalid dependency and not just the last one.
    // Find a way to do that without cloning each job if it's even possible.
    fn job_as_dep_check(&mut self, job: Job) -> RResult<&mut Self> {
        let mut stack = vec![job];

        while let Some(current_job) = stack.pop() {
            for dep in current_job.deps().iter() {
                if let Some(dep_job) = self.find_job_by_target_mut(&dep.to_owned()) {
                    if let Err(err) = dep_job.execute_async() {
                        match err.kind() {
                            ErrorKind::NotFound => return Err(RakeError::InvalidDependency(Info::from(self), dep.to_owned())),
                            err @ _             => return Err(RakeError::FailedToExecute(err.to_string()))
                        };
                    }
                    stack.push(dep_job.to_owned());
                } else if !Rob::is_file(&dep) {
                    return Err(RakeError::InvalidDependency(Info::from(self), dep.to_owned()));
                }
            }
        }

        Ok(self)
    }

    fn execute_job(&mut self) -> RResult::<()> {
        // Borrow checker SeemsGood
        if self.jobs.is_empty() { return Ok(()) }

        let rakefile = {
            let job = self.jobs[0].to_owned();
            self.job_as_dep_check(job)?
        };

        let job = &mut rakefile.jobs[0];
        if let Err(err) = job.execute_async() {
            return Err(RakeError::FailedToExecute(err.to_string()))
        }

        Ok(())
    }

    fn parse_line(&mut self, line: &str) -> RResult::<()> {
        if let Some(colon_idx) = line.chars().position(|x| x == ':') {
            self.parse_job(&colon_idx, line)?;
        } Ok(())
    }

    const KEEPGOING_FLAG: &'static str = "-k";
    const SILENT_FLAG: &'static str = "-s";

    fn parse_flags() -> Config {
        let args = env::args().collect::<Vec::<_>>();
        let keepgoing = args.iter().any(|x| x == Self::KEEPGOING_FLAG);
        let silent = args.iter().any(|x| x == Self::SILENT_FLAG);
        let mut cfg = Config::default();
        cfg.keepgoing(keepgoing).echo(!silent);
        cfg
    }

    fn init()  {
        let file_path = Self::find_rakefile().unwrap_or_report();
        let file_str = read_to_string(&file_path).unwrap_or_report();

        let mut rakefile = Rakefile {
            cfg: Self::parse_flags(),
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
    4. Async mode | Sync mode,
    5. Variables
 */
