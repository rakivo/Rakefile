use std::{
    env,
    result,
    str::Lines,
    path::PathBuf,
    io::ErrorKind,
    iter::Peekable,
    process::Output,
    default::Default,
    fs::read_to_string,
    collections::{HashSet, HashMap}
};
use regex::*;
use robuild::*;

mod error;
mod ss;
mod flag;
mod cfg;

use error::*;
use ss::*;
use flag::*;
use cfg::*;

type RResult<T> = result::Result::<T, RakeError>;

#[derive(Clone)]
struct RJob(Job, Info);

struct Rakefile<'a> {
    row: usize,

    cfg: Config,

    deps_re: Regex,
    vars_re: Regex,

    file_path: String,

    jobs: Vec::<RJob>,
    jobmap: HashMap::<String, usize>,

    // When parsing flags, you can come across a string,
    // that is not a defined flag, and it may be a potential job,
    // similar to how in Makefile you can do `make examples` when `examples`
    // is a defined job. Why potential? Because we are parsing flags before
    // parsing `Rakefile` whether it exists or not, and after we parsed it,
    // we can check, if the potential job is actually a defined one.
    potential_jobs: Vec::<String>,

    vars: HashMap::<&'a str, &'a str>,

    iter: Peekable::<Lines<'a>>
}

impl Default for Rakefile<'_> {
    fn default() -> Self {
        Self {
            row: 1,
            cfg: Config::default(),
            deps_re: Regex::new("").unwrap(),
            vars_re: Regex::new("").unwrap(),
            file_path: String::default(),
            jobs: Vec::default(),
            jobmap: HashMap::default(),
            potential_jobs: Vec::default(),
            vars: HashMap::default(),
            iter: "".lines().peekable(),
        }
    }
}

impl<'a> Rakefile<'a> {
    pub const TAB_WIDTH: usize = 4;
    pub const MAX_DIR_LVL: usize = 2;

    pub const RAKE_FILE_NAME: &'static str = "Rakefile";

    pub const DEPS_REGEX: &'static str = r"\$d\[(.*?)\]";
    pub const VARS_REGEX: &'static str = r"\$\((.*?)\)";

    fn find_rakefile() -> RResult::<PathBuf> {
        let dir_path = env::current_dir().unwrap_or_report();
        Dir::new(&dir_path).into_iter()
            .find(|f| matches!(f.file_name(), Some(name) if name == Self::RAKE_FILE_NAME))
            .ok_or_else(move || RakeError::NoRakefileInDir(dir_path.to_owned()))
    }

    fn pretty_file_path(file_path: &str) -> String {
        let mut count = 0;
        file_path.chars().rev().take_while(|c| {
            if *c == DELIM_CHAR { count += 1; }
            count < Self::MAX_DIR_LVL
        }).collect::<Vec::<_>>().into_iter().rev().collect()
    }

    #[inline]
    fn append_job(&mut self, job: RJob) {
        let key = job.0.target();

        if let Some(idx) = self.jobmap.get(key) {
            let old_job = self.jobs.get(*idx).unwrap();
            let f = &job.1.0;
            log!(WARN, "{f}:{l1}: Overriding recipe for target: '{key}'", l1 = job.1.1);
            log!(WARN, "{f}:{l2}: Defined here", l2 = old_job.1.1);
            self.jobs.remove(*idx);
        }

        self.jobmap.insert(key.to_owned(), self.jobs.len());
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
        target: &str,
        deps_joined: &str,
        deps: &Vec::<&str>,
        line: &str
    ) -> RResult::<String>
    {
        use SSymbol::*;

        let mut line = self.parse_deps_ss(&line, &deps)?;

        sreplace!(line, MakeTarget, &target);
        sreplace!(line, RakeTarget, &target);

        sreplace!(line, MakeDeps, &deps_joined);
        sreplace!(line, RakeDeps, &deps_joined);

        if line.contains(&SSymbol::MakeDep.to_string())
        || line.contains(&SSymbol::RakeDep.to_string())
        {
            let Some(first_dep) = deps.get(0) else {
                return Err(RakeError::DepsSSwithoutDeps(Info::from(self)))
            };
            sreplace!(line, MakeDep, &first_dep);
            sreplace!(line, RakeDep, &first_dep);
        }

        Ok(line)
    }

    fn parse_vars(&self, line: &str) -> RResult::<String> {
        for caps in self.vars_re.captures_iter(&line) {
            if self.vars.get(&caps[1]).is_none() {
                return Err(RakeError::InvalidValue(Info::from(self), caps[1].to_owned()))
            }
        }

        Ok(self.vars_re.replace_all(&line, |caps: &Captures| self.vars.get(&caps[1]).unwrap()).to_string())
    }

    #[inline(always)]
    fn find_job_by_target_mut(&mut self, target: &str) -> Option::<&mut RJob> {
        self.jobs.iter_mut().find(|j| j.0.target().eq(target))
    }

    #[inline(always)]
    fn advance(&mut self) {
        self.iter.next();
    }

    fn parse_job(&mut self, line: &str) -> RResult::<()> {
        let line = self.parse_vars(&line)?;
        let new_idx = line.chars().position(|x| x.eq(&':')).unwrap();
        let (target_untrimmed, deps_untrimmed) = line.split_at(new_idx);
        let target = target_untrimmed.trim();

        if target.is_empty() {
            return Err(RakeError::NoTarget(Info::from(self)))
        }

        let deps = deps_untrimmed
            .split_whitespace()
            .skip(1)
            .collect::<Vec::<_>>();

        let deps_joined = deps.join(" ");
        let signature_row = self.row;
        let mut body = Vec::new();
        while let Some(next_line) = self.iter.peek() {
            let line = next_line.to_owned();
            if line.starts_with('#') {
                self.advance();
                self.row += 1;
                continue
            }

            let line = self.parse_special_symbols(&target, &deps_joined, &deps, &line)?;
            let line = self.parse_vars(&line)?;

            self.row += 1;

            let trimmed = line.trim().to_owned();

            // Allow people to use both tabs and spaces
            if line.starts_with('\t') {
                body.push(trimmed);
                self.advance();
                continue
            }

            let whitespace_count = line.chars()
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

        let ss_check1 = parse_special_job_by_target!(self, target,
                                                     deps, cmd,
                                                     phony, true,
                                                     SSymbol::MakePhony,
                                                     SSymbol::RakePhony);

        let ss_check2 = parse_special_job_by_target!(self, target,
                                                     deps, cmd,
                                                     echo, false,
                                                     SSymbol::MakeSilent);

        if !(ss_check1 && ss_check2) {
            let job = Job::new(target, deps, cmd);
            let info = Info(self.file_path.to_owned(), signature_row);
            let rjob = RJob(job, info);
            self.append_job(rjob);
        }

        Ok(())
    }

    fn handle_output
    (
        &self,
        output: IoResult::<Vec::<Output>>,
        dep: String,
        dep_job_info: Info,
        curr_job_info: Info
    ) -> RResult::<()>
    {
        match output {
            Ok(ok) => if ok.iter().find(|out| !out.stderr.is_empty()).is_some() && !self.cfg.keepgoing {
                // Error-message printing handled in robuild: https://github.com/rakivo/robuild
                Err(RakeError::FailedToExecute(dep_job_info))
            } else { Ok(()) }
            Err(err) => if err.kind().eq(&ErrorKind::NotFound) {
                Err(RakeError::InvalidDependency(curr_job_info, dep))
            } else {
                Err(RakeError::FailedToExecute(curr_job_info))
            }
        }
    }

    // Find a way to do that without cloning each job if it's even possible.
    fn job_as_dep_check(&mut self, job: RJob) -> RResult<&mut Self> {
        let mut stack = vec![job];

        while let Some(curr_job) = stack.pop() {
            for dep in curr_job.0.deps().iter() {
                if let Some(dep_job) = self.find_job_by_target_mut(&dep.to_owned()) {
                    stack.push(dep_job.to_owned());
                    let out = dep_job.0.execute_async_dont_exit();
                    let dep_job_info = dep_job.1.to_owned();
                    let curr_job_info = curr_job.1.to_owned();
                    self.handle_output(out, dep.to_owned(), dep_job_info, curr_job_info)?;
                } else if !(Rob::is_file(&dep) || Rob::is_dir(&dep)) {
                    return Err(RakeError::InvalidDependency(curr_job.1, dep.to_owned()));
                }
            }
        }

        Ok(self)
    }

    fn execute_job(&mut self, mut job: RJob) -> RResult::<()> {
        self.job_as_dep_check(job.to_owned())?;
        if job.0.execute_async_dont_exit().is_err() && !self.cfg.keepgoing {
            // Error-message printing handled in robuild: https://github.com/rakivo/robuild
            Err(RakeError::FailedToExecute(job.1.to_owned()))
        } else {
            Ok(())
        }
    }

    fn parse_variable_declaration(&mut self, idx: usize, line: &'a str) -> RResult::<()> {
        let (name_untrimmed, value_untrimmed) = line.split_at(idx);
        let name = name_untrimmed.trim();

        if name.split_whitespace().count() > 1 {
            return Err(RakeError::MultipleNames(Info::from(self)))
        }

        let value_trimmed = value_untrimmed[2..].trim();

        let value = if value_trimmed.starts_with("$(") && value_trimmed.ends_with(')') {
            match self.vars.get(&value_trimmed[2..value_trimmed.len() - 1]) {
                Some(val) => val,
                _ => return Err(RakeError::InvalidValue(Info::from(self), value_trimmed.to_owned()))
            }
        } else {
            value_trimmed
        };

        self.vars.insert(name, value);
        self.row += 1;

        Ok(())
    }

    fn parse_line(&mut self, line: &'a str) -> RResult::<()> {
        if line.trim().is_empty() || line.starts_with('#') {
            self.row += 1;
        } else if line.chars().find(|x| x.eq(&':')).is_some() {
            self.parse_job(line)?;
        } else if let Some(eq_idx) = line.chars().position(|x| x.eq(&'=')) {
            self.parse_variable_declaration(eq_idx, line)?;
        } else if !line.trim().is_empty() {
            panic!("Wtf is dis scheisse: `{line}` ??? ");
        }
        Ok(())
    }

    fn check_potential_jobs(&mut self) -> RResult::<Vec<RJob>> {
        let ret = self.potential_jobs.iter().try_fold(HashSet::new(), |mut set, pj| {
            if let Some(idx) = self.jobs.iter().position(|j| j.0.target().eq(pj)) {
                set.insert(idx);
                Ok(set)
            } else {
                Err(RakeError::InvalidFlag(pj.to_owned()))
            }
        })?.into_iter().map(|idx| self.jobs[idx].to_owned()).collect();

        Ok(ret)
    }

    fn parse_flags() -> RResult::<(RConfig, Config, Vec::<String>)> {
        use Flag::*;
        use RakeError::*;

        let mut iter = env::args().skip(1).into_iter();
        let mut rcfg = RConfig::default();
        let mut cfg = Config::default();
        let mut jobs = Vec::new();

        while let Some(f) = iter.next() {
            let farg = (f.to_owned(), iter.next());
            match Flag::try_from(farg) {
                Ok(flag) => match flag {
                    Keepgoing => { cfg.keepgoing(true); }
                    Silent    => { cfg.echo(false); }
                    Cd(arg)   => { rcfg.cd(arg); }
                }
                Err(err) => match err {
                    InvalidUseOfFlag(..) => return Err(err),
                    _ => jobs.push(f.to_owned())
                }
            }
        }

        Ok((rcfg, cfg, jobs))
    }

    fn init() {
        let (rcfg, cfg, potential_jobs) = Self::parse_flags().unwrap_or_report();

        let (curr_dir, entered_dir) = if let Some(dir) = rcfg.if_cd() {
            log!(INFO, "Entering directory `{dir}`");
            env::set_current_dir(&dir).unwrap_or_report();
            (dir, true)
        } else {
            (env::current_dir().unwrap_or_report().to_string_lossy().into_owned(), false)
        };

        let file_path = Self::find_rakefile().unwrap_or_report();
        let file_str = read_to_string(&file_path).unwrap_or_report();

        let mut rakefile = Rakefile {
            cfg,
            deps_re: Regex::new(Self::DEPS_REGEX).unwrap(),
            vars_re: Regex::new(Self::VARS_REGEX).unwrap(),
            file_path: Self::pretty_file_path(file_path.to_str().expect("Failed to convert file path to str")),
            potential_jobs,
            iter: file_str.lines().peekable(),
            ..Self::default()
        };

        while let Some(line) = rakefile.iter.next() {
            rakefile.parse_line(&line).unwrap_or_report();
        }

        let pot_jobs = rakefile.check_potential_jobs().unwrap_or_report();

        let jobs = if !pot_jobs.is_empty() {
            pot_jobs
        } else {
            vec![rakefile.jobs[0].to_owned()]
        };

        jobs.into_iter().for_each(|j| {
            rakefile.execute_job(j).unwrap_or_report()
        });

        if entered_dir {
            log!(INFO, "Leaving directory `{curr_dir}`");
        }
    }
}

fn main() {
    Rakefile::init()
}

/* TODO:
    4. Async mode | Sync mode,
    5. Variables and :=, ?=, += syntax.
    6. @ Syntax to disable echo for specific line.
    7. % syntax for pattern matching.
    8. Refactor out (RConfig, Config, Vec::<String>) to separate structure.
 */
