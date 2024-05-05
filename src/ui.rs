use console::Style;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time;

pub const PROGRESS_BARS_STYLE: &str =
    "{prefix:>12.bold.cyan} [{bar:40}] {pos}/{len}: {msg} ({eta} left)";

pub struct Spinner {
    pub spinner: ProgressBar,
    pub finished: Arc<Mutex<bool>>,
    pub thread: JoinHandle<()>,
}

impl Spinner {
    pub fn start(message: &str) -> Self {
        let spinner = ProgressBar::new(0).with_style(
            ProgressStyle::with_template(&("{spinner:.cyan} ".to_owned() + message)).unwrap(),
        );
        spinner.tick();

        let thread_spinner = spinner.clone();
        let finished = Arc::new(Mutex::new(false));
        let thread_finished = Arc::clone(&finished);
        let spinner_thread = thread::spawn(move || {
            while !*thread_finished.lock().unwrap() {
                thread_spinner.tick();
                thread::sleep(time::Duration::from_millis(100));
            }
            thread_spinner.finish_and_clear();
        });

        Self {
            spinner: spinner.clone(),
            finished,
            thread: spinner_thread,
        }
    }

    pub fn end(self, message: &str) {
        *self.finished.lock().unwrap() = true;
        self.thread.join().unwrap();
        println!("{}", message);
    }
}

pub fn setup_progress_bar(total: u64, verb: &'static str) -> ProgressBar {
    indicatif::ProgressBar::new(total)
        .with_prefix(verb)
        .with_style(
            indicatif::ProgressStyle::with_template(PROGRESS_BARS_STYLE)
                .unwrap()
                .progress_chars("=> "),
        )
}

pub trait Log {
    fn log(&self, verb: &'static str, message: &str);
}

pub fn format_log_msg(verb: &'static str, message: &str) -> String {
    let style = Style::new().bold().cyan();
    format!("{}: {}", style.apply_to(format!("{verb:>12}")), message)
}

impl Log for ProgressBar {
    fn log(&self, verb: &'static str, message: &str) {
        self.println(format_log_msg(verb, message));
    }
}
