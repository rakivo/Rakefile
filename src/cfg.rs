use crate::Flag::{self, *};

#[derive(Debug, Default)]
pub struct RConfig {
    flags: Vec::<Flag>
}

macro_rules! setter {
    ($fn: tt, $if_fn: tt, $name: tt, $arg: tt: $ty: ty) => {
        pub fn $fn(&mut self, $arg: $ty) -> &mut Self {
            self.flags.push($name($arg));
            self
        }

        pub fn $if_fn(&self) -> Option::<$ty> {
            self.flags.iter().position(|e| matches!(e, $name(_)))
                .and_then(|idx| {
                    match &self.flags[idx] {
                        $name(arg) => Some(arg.to_owned()),
                        _ => None,
                    }
                })
        }
    }
}

impl RConfig {
    setter!{cd, if_cd, Cd, path: String}
}
