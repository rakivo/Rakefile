use std::{
    env,
    result,
    str::Lines,
    path::PathBuf,
    sync::LazyLock,
    iter::Peekable,
    process::Output,
    default::Default,
    fs::read_to_string,
    collections::{
        VecDeque,
        HashSet,
        HashMap
    }
};
use regex::*;
use robuild::*;

mod ss;
mod ct;
mod cfg;
mod flag;
mod error;

use ss::*;
use ct::*;
use cfg::*;
use flag::*;
use error::*;

type RResult<T> = result::Result::<T, RakeError>;

#[derive(Debug, Clone)]
struct RJob(Job, Info);

const DEPS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$d\[(.*?)\]").unwrap());
const VARS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\((.*?)\)").unwrap());

struct Rakefile<'a> {
    row: usize,

    file_path: PathBuf,

    jobs: VecDeque::<RJob>,
    jobmap: HashMap::<String, usize>,

    comptime: Comptime,

    vars: HashMap::<&'a str, &'a str>,

    iter: Peekable::<Lines<'a>>
}

impl Default for Rakefile<'_> {
    fn default() -> Self {
        Self {
            row: 1,
            file_path: PathBuf::default(),
            jobs: VecDeque::default(),
            jobmap: HashMap::default(),
            vars: HashMap::default(),
            comptime: Comptime::default(),
            iter: "".lines().peekable(),
        }
    }
}

impl<'a> Rakefile<'a> {
    pub const TAB_WIDTH: usize = 4;
    pub const MAX_DIR_LVL: usize = 3;

    pub const RAKE_FILE_NAME: &'static str = "Rakefile";

    fn find_rakefile() -> RResult::<PathBuf> {
        let dir_path = env::current_dir().unwrap_or_report();
        let pretty_path = Self::pretty_path(&dir_path);
        Dir::new(&dir_path).into_iter()
            .find(|f| matches!(f.file_name(), Some(name) if name == Self::RAKE_FILE_NAME))
            .ok_or_else(move || RakeError::NoRakefileInDir(pretty_path))
    }

    fn pretty_path(file_path: &PathBuf) -> String {
        let mut count = 0;
        let string = file_path.display().to_string();
        string.chars().rev().take_while(|c| {
            if *c == DELIM_CHAR { count += 1; }
            count < Self::MAX_DIR_LVL
        }).collect::<Vec::<_>>().into_iter().rev().collect()
    }

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
        self.jobs.push_back(job);
    }

    fn parse_deps_ss(info: Info, line: &str, deps: &Vec::<&str>) -> RResult::<String> {
        for caps in DEPS_REGEX.captures_iter(&line) {
            let idx = caps[1].parse::<usize>().unwrap_or(0);
            if deps.get(idx).is_none() {
                return Err(RakeError::DepsIndexOutOfBounds(info, deps.len()));
            }
        }

        let deps = DEPS_REGEX.replace_all(&line, |caps: &Captures| {
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

        let mut line = Self::parse_deps_ss(Info::from(self), &line, &deps)?;

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
        for caps in VARS_REGEX.captures_iter(&line) {
            if self.vars.get(&caps[1]).is_none() {
                return Err(RakeError::InvalidValue(Info::from(self), caps[1].to_owned()))
            }
        }

        Ok(VARS_REGEX.replace_all(&line, |caps: &Captures| self.vars.get(&caps[1]).unwrap()).to_string())
    }

    #[inline(always)]
    fn find_job_by_target_mut(&mut self, target: &str) -> Option::<&mut RJob> {
        self.jobs.iter_mut().find(|j| j.0.target().eq(target))
    }

    #[inline(always)]
    fn advance(&mut self) {
        self.row += 1;
        self.iter.next();
    }

    fn parse_job(&mut self, line: &str) -> RResult::<()> {
        let line = self.parse_vars(&line)?;
        let new_idx = line.chars().position(|x| x.eq(&':')).unwrap();
        let (target_untrimmed, deps_untrimmed) = line.split_at(new_idx);
        let target = target_untrimmed.trim();

        if target.is_empty() {
            return Err(RakeError::NoTarget(Info::from(&*self)))
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
                continue
            }

            let line = self.parse_special_symbols(&target, &deps_joined, &deps, &line)?;
            let line = self.parse_vars(&line)?;

            let trimmed = line.trim().to_owned();

            // Allow people to use both tabs and spaces
            if line.starts_with('\t') {
                body.push(trimmed);
                self.advance();
                continue
            }

            let whitespace_count = line.chars().take_while(|c| c.is_whitespace()).count();
            match whitespace_count {
                Self::TAB_WIDTH => {
                    self.advance();
                    body.push(trimmed)
                }
                i @ 1.. => return Err(RakeError::InvalidIndentation(Info::from(&*self), i)),
                _ => if trimmed.is_empty() { self.advance(); } else { self.row += 1; break }
            };
        }

        let cfg = self.comptime.cfg();
        let cmd = body.iter().fold(RobCommand::from(cfg.to_owned()), |mut cmd, line| {
            cmd.append_mv(&[line]);
            cmd
        });

        let ss_check1 = parse_special_job_by_target!(self, target, deps, cmd, phony, true, SSymbol::MakePhony, SSymbol::RakePhony);
        let ss_check2 = parse_special_job_by_target!(self, target, deps, cmd, echo, false, SSymbol::MakeSilent);
        if !(ss_check1 && ss_check2) {
            let job = Job::new(target, deps, cmd);
            let info = Info::from((&*self, signature_row));
            let rjob = RJob(job, info);
            self.append_job(rjob);
        }

        Ok(())
    }

    fn handle_output
    (
        &self,
        dep_job_info: Info,
        output: RobResult::<Vec::<Output>>
    ) -> RResult::<()>
    {
        let keepgoing = self.comptime.cfg().keepgoing;
        match output {
            Ok(ok) => if let Some(err) = ok.into_iter().find(|out| matches!(out.status.code(), Some(code) if code != 0)) {
                // Error-message printing handled in robuild: https://github.com/rakivo/robuild
                if !keepgoing {
                    let err = String::from_utf8_lossy(&err.stderr);
                    Err(RakeError::FailedToExecute(dep_job_info, err.to_string()))
                } else { Ok(()) }
            } else { Ok(()) }
            Err(err) => match err {
                RobError::NotFound(file_path) => Err(RakeError::InvalidDependency(dep_job_info, file_path)),
                _ => Err(RakeError::FailedToExecute(dep_job_info, err.to_string()))
            }
        }
    }

    // Find a way to do that without cloning each job if it's even possible.
    fn job_as_dep_check(&mut self, job: RJob) -> RResult<&mut Self> {
        let mut stack = vec![job];

        while let Some(mut curr_job) = stack.pop() {
            if curr_job.0.deps().iter().any(|d| Rob::is_file(&d)) {
                let out = curr_job.0.execute_async_dont_exit();
                self.handle_output(curr_job.1, out)?;
            } else {
                for dep in curr_job.0.deps().iter() {
                    if let Some(dep_job) = self.find_job_by_target_mut(&dep.to_owned()) {
                        stack.push(dep_job.to_owned());
                    } else if !(Rob::is_file(&dep) || Rob::is_dir(&dep)) {
                        return Err(RakeError::InvalidDependency(curr_job.1, dep.to_owned()));
                    }
                }
            }
        }

        Ok(self)
    }

    fn execute_job(&mut self, mut job: RJob) -> RResult::<()> {
        let keepgoing = self.comptime.cfg().keepgoing;
        self.job_as_dep_check(job.to_owned())?;
        if let Err(err) = job.0.execute_async_dont_exit_unchecked() {
            // Error-message printing handled in robuild: https://github.com/rakivo/robuild
            if !keepgoing {
                Err(RakeError::FailedToExecute(job.1.to_owned(), err.to_string()))
            } else {
                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn parse_variable_declaration(&mut self, idx: usize, line: &'a str) -> RResult::<()> {
        let (name_untrimmed, value_untrimmed) = line.split_at(idx);
        let name = name_untrimmed.trim();

        if name.split_whitespace().count() > 1 {
            return Err(RakeError::MultipleNames(Info::from(&*self)))
        }

        let value_trimmed = value_untrimmed[2..].trim();

        let value = if value_trimmed.starts_with("$(") && value_trimmed.ends_with(')') {
            match self.vars.get(&value_trimmed[2..value_trimmed.len() - 1]) {
                Some(val) => val,
                _ => return Err(RakeError::InvalidValue(Info::from(&*self), value_trimmed.to_owned()))
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
        let ret = self.comptime.potential_jobs().iter().try_fold(HashSet::new(), |mut set, pj| {
            if let Some(idx) = self.jobs.iter().position(|j| j.0.target().eq(pj)) {
                set.insert(idx);
                Ok(set)
            } else {
                let names = self.jobs.iter()
                    .filter_map(|j| {
                        let tar = j.0.target().to_owned();
                        match SSymbol::try_from(&tar) {
                            Ok(..) => None,
                            Err(..) => Some(tar)
                        }
                    }).collect::<Vec::<_>>().join(", ");

                Err(RakeError::InvalidArgument(pj.to_owned(), names))
            }
        })?.into_iter().map(|idx| self.jobs[idx].to_owned()).collect();

        Ok(ret)
    }

    fn execute_jobs(&mut self) {
        let pot_jobs = self.check_potential_jobs().unwrap_or_report();

        let jobs = if !pot_jobs.is_empty() {
            pot_jobs
        } else {
            vec![self.jobs[0].to_owned()]
        };

        jobs.into_iter().for_each(|j| self.execute_job(j).unwrap_or_report());
    }

    fn init() {
        let comptime = Comptime::new().unwrap_or_report();

        let file_path = Self::find_rakefile().unwrap_or_report();
        let file_str = read_to_string(&file_path).unwrap_or_report();

        let mut rakefile = Rakefile {
            comptime,
            file_path,
            iter: file_str.lines().peekable(),
            ..Self::default()
        };

        while let Some(line) = rakefile.iter.next() {
            rakefile.parse_line(&line).unwrap_or_report();
        }

        rakefile.execute_jobs();
        rakefile.comptime.handle_ucd();
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
    9. Make it possible to declare dependencies of the special .PHONY, .SILENT, ... jobs, before declaration of the specified job if ykwim
    11. Factor out `MakePhony`, `RakePhony`, `MakeSilent` ..., to separate enum, because they're not special symbols
    12. Fix a shit ton of fucking bugs. It's so fucking annoying to realize that your program is useless shit after working on it for two weeks
    13. Make `Job::execute_all_async` execute async for real, and do not wait for every fucking child every fucking job, useless piece of shit.
 */
