// Allow people to use both Makefiles and Rakefiles
// special symbols.
pub enum SSymbol {
    MakeTarget,
    RakeTarget,

    MakeDep,
    RakeDep,

    MakeDeps,
    RakeDeps,
}

impl ToString for SSymbol {
    fn to_string(&self) -> String {
        use SSymbol::*;
        match self {
            MakeTarget => "$@",
            RakeTarget => "$t",

            MakeDep => "$d",
            RakeDep => "$<",

            MakeDeps => "$ds",
            RakeDeps => "$^",
        }.to_owned()
    }
}

#[macro_export]
macro_rules! sreplace {
    ($line: expr, $variant: tt, $val: expr) => {
        #[allow(unused)]
        use SSymbol::*;
        *$line = $line.replace(&$variant.to_string(), $val);
    };
}
