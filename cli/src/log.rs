use colored::Colorize;
use spinoff::{spinners, Color, Spinner};

pub struct Log {
    spinner: Option<Spinner>,
}

impl Log {
    pub fn start(what: &str) -> Self {
        if atty::is(atty::Stream::Stdout) {
            let clone = what.to_string();

            // only show spinner when tty
            Self { spinner: Some(Spinner::new(spinners::Dots, clone, Color::White)) }
        } else {
            Self { spinner: None }
        }
    }

    pub fn next(&mut self, what: &str) {
        if let Some(spinner) = &mut self.spinner {
            let clone = what.to_string();
            spinner.update_text(clone);
        }
    }

    pub fn succeed(self, what: &str) {
        if let Some(mut spinner) = self.spinner {
            spinner.success(what)
        }
    }

    pub fn fail(self, what: &str) {
        if let Some(mut spinner) = self.spinner {
            spinner.fail(what)
        } else {
            eprintln!("error: {what}");
        }
    }

    pub fn success(what: &str) {
        if atty::is(atty::Stream::Stdout) {
            println!("{} {}", "✓".green().bold(), what);
        }
    }

    pub fn failure(what: &str) {
        if atty::is(atty::Stream::Stdout) {
            println!("{} {}", "✗".red().bold(), what);
        } else {
            eprintln!("error: {}", what)
        }
    }

    pub fn warning(what: &str) {
        if atty::is(atty::Stream::Stdout) {
            println!("{} {}", "~".yellow().bold(), what);
        } else {
            eprintln!("warn: {}", what)
        }
    }
}
