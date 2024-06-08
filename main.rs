use std::{
    env,
    str::Lines,
    path::PathBuf,
    io::ErrorKind,
    iter::Peekable,
    fs::read_to_string,
};
use robuild::*;

struct Rakefile<'a> {
    jobs: Vec::<Job>,
    iter: Peekable::<Lines<'a>>
}

impl<'a> Rakefile<'a> {
    fn find_rakefile() -> IoResult::<PathBuf> {
        let path = env::current_dir()?;
        let dir = Dir::new(path);

        for f in dir {
            if matches!(f.file_name(), Some(name) if name == "Rakefile") {
                return Ok(f);
            }
        }

        let err = IoError::new(ErrorKind::NotFound, "Rakefile not found in current directory");
        Err(err)
    }

    fn add_job() {}

    fn parse_job(&mut self, idx: &usize, line: &str) {
        let (target, deps) = line.split_at(*idx);
        let deps = deps.chars().skip_while(|c| c.is_whitespace() || c.eq(&':')).collect::<String
>();

        let mut body = Vec::new();
        while let Some(next_line) = self.iter.next() {
            let whitespace_count = next_line.chars().take_while(|c| c.is_whitespace()).count();
            if whitespace_count == 4 {
                body.push(next_line.trim());
            } else if whitespace_count < 4 && whitespace_count > 0 {
                panic!("Invalid indenting")
            }
        }
();

        println!("target: {target}");
        println!("deps: {deps}");
        println!("body: {body:?}");
    }

    fn parse_line(&mut self, line: &str) -> IoResult::<()> {
        if let Some(colon_idx) = line.chars().position(|x| x == ':') {
            self.parse_job(&colon_idx, line);
        }
        Ok(())
    }

    pub fn perform() -> IoResult::<()> {
        let file = Self::find_rakefile()?;
        let file_str = read_to_string(file)?;
        let iter = file_str.lines().peekable();

        let mut rakefile = Rakefile {
            jobs: Vec::new(),
            iter
        };

        while let Some(line) = rakefile.iter.next() {
            rakefile.parse_line(&line)?;
        }

        Ok(())
    }
}

fn main() -> IoResult::<()> {
    Rakefile::perform()
}
