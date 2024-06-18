use std::{
    env,
    collections::HashSet
};
use robuild::*;

use crate::{
    Config,
    RConfig,
    RResult,
    ALL_FLAGS_STR,
    error::UnwrapOrReport
};

#[derive(Default)]
pub struct Comptime {
    cfg: Config,

    #[allow(unused)]
    rcfg: RConfig,
    entered_dir: Option::<String>,

    // When parsing flags, you can come across a string,
    // that is not a defined flag, and it may be a potential job,
    // similar to how in Makefile you can do `make examples` when `examples`
    // is a defined job. Why potential? Because we are parsing flags before
    // parsing `Rakefile` whether it exists or not, and after we parsed it,
    // we can check, if the potential job is actually a defined one.
    potential_jobs: HashSet::<String>,
}

macro_rules! getter {
    ($name: tt: $ty: ty) => {
        pub fn $name(&self) -> &$ty {
            &self.$name
        }
    }
}

impl Comptime {
    pub fn new() -> RResult::<Comptime> {
        use crate::Flag::{self, *};
        use crate::RakeError::*;

        let mut iter = env::args().skip(1).into_iter().peekable();

        let mut cfg = Config::default();
        let mut rcfg = RConfig::default();
        let mut potential_jobs = HashSet::new();

        let mut skip = false;
        while let Some(f) = iter.next() {
            if skip { continue }

            let arg = if let Some(arg) = iter.peek() {
                if ALL_FLAGS_STR.contains(&arg.as_str()) { None }
                else {
                    skip = true;
                    Some(arg.to_owned())
                }
            } else { None };

            let farg = (f.to_owned(), arg);
            match Flag::try_from(farg) {
                Ok(flag) => match flag {
                    Keepgoing => { cfg.keepgoing(true); }
                    Silent    => { cfg.echo(false); }
                    Cd(arg)   => { rcfg.cd(arg); }
                }
                Err(err) => match err {
                    InvalidUseOfFlag(..) => return Err(err),
                    _ => { potential_jobs.insert(f.to_owned()); }
                }
            }
        }

        let entered_dir = if let Some(dir) = rcfg.if_cd() {
            log!(INFO, "Entering directory `{dir}`");
            env::set_current_dir(&dir).unwrap_or_report();
            Some(dir)
        } else { None };

        Ok(Comptime {
            cfg,
            rcfg,
            entered_dir,
            potential_jobs
        })
    }

    #[inline(always)]
    pub fn handle_ucd(&self) {
        if let Some(ref dir) = self.entered_dir {
            log!(INFO, "Leaving directory `{dir}`");
        }
    }

    getter!{cfg: Config}
    getter!{potential_jobs: HashSet::<String>}
}
