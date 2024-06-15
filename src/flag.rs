use std::process::exit;

pub enum Flag {
    Keepgoing,
    Silent
}

impl ToString for Flag {
    fn to_string(&self) -> String {
        use Flag::*;
        let s = match self {
            Keepgoing => "k",
            Silent    => "t",
        };
        format!("-{s}")
    }
}

impl TryFrom::<&str> for Flag {
    type Error = ();

    #[track_caller]
    fn try_from(val: &str) -> Result<Self, Self::Error> {
        use Flag::*;
        match val {
            "-k" => Ok(Keepgoing),
            "-s" => Ok(Silent),
            _    => {
                eprintln!("Unsupported flag: `{val}`");
                if cfg!(debug_assertions) {
                    todo!()
                } else {
                    exit(1)
                }
            }
        }
    }
}

impl TryFrom::<String> for Flag {
    type Error = ();

    #[track_caller]
    fn try_from(val: String) -> Result<Self, Self::Error> {
        Self::try_from(val.as_str())
    }
}

impl TryFrom::<&String> for Flag {
    type Error = ();

    #[track_caller]
    fn try_from(val: &String) -> Result<Self, Self::Error> {
        Self::try_from(val.as_str())
    }
}
