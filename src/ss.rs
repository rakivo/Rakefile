// Allow people to use both Makefiles and Rakefiles
// special symbols.
pub enum SSymbol {
    MakeTarget,
    RakeTarget,

    MakeDep,
    RakeDep,

    MakeDeps,
    RakeDeps,

    MakePhony,
    RakePhony,

    MakeSilent
}

impl ToString for SSymbol {
    fn to_string(&self) -> String {
        use SSymbol::*;
        match self {
            MakeTarget => "$@",
            RakeTarget => "$t",

            MakeDep    => "$d",
            RakeDep    => "$<",

            MakeDeps   => "$ds",
            RakeDeps   => "$^",

            MakePhony  => ".PHONY",
            RakePhony  => ".ALWAYS",

            MakeSilent => ".SILENT"
        }.to_owned()
    }
}

#[macro_export]
macro_rules! sreplace {
    ($line: expr, $variant: tt, $val: expr) => {
        $line = $line.replace(&$variant.to_string(), $val);
    };
}

#[macro_export]
macro_rules! parse_special_job_by_target {
    ($self: ident, $tar: expr, $deps: expr, $cmd: expr, $field: tt, $val: expr, $($ss: expr), *) => {{
        let check = [$($ss), *].iter().any(|x| x.to_string().eq($tar));
        if check {
            for tar_ in $deps.iter() {
                if let Some(job) = $self.find_job_by_target_mut(tar_) {
                    job.0.$field($val);
                }
            }
        } check
    }}
}
