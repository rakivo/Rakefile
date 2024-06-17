use crate::RakeError;

// NOTE: Update `supported flags` message if you updated this enum:
pub enum Flag {
    Keepgoing,
    Silent,
    Cd(String)
}

impl ToString for Flag {
    fn to_string(&self) -> String {
        use Flag::*;
        let s = match self {
            Keepgoing => "k",
            Silent    => "t",
            Cd(arg)   => &format!("C {arg}"),
        };
        format!("-{s}")
    }
}

type FlagAndArg = (String, Option::<String>);

impl TryFrom::<FlagAndArg> for Flag {
    type Error = RakeError;

    fn try_from(farg: FlagAndArg) -> Result<Self, Self::Error> {
        use Flag::*;
        let (f, arg) = farg;
        match f.as_str() {
            "-k" => Ok(Keepgoing),
            "-s" => Ok(Silent),
            "-C" => if let Some(arg) = arg {
                if arg.is_empty() {
                    return Err(RakeError::InvalidUseOfFlag(f, vec![String::default()]))
                }

                let first = arg.chars().nth(0).unwrap() as u8;
                match first {
                    32..46 => Err(RakeError::InvalidUseOfFlag(f, vec![arg])),
                    _      => Ok(Cd(arg.to_owned()))
                }
            } else {
                Err(RakeError::InvalidUseOfFlag(f, vec![String::default()]))
            }
            _ => Err(RakeError::InvalidScheisse)
        }
    }
}
