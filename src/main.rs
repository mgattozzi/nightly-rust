#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate time;
extern crate egg_mode;

use egg_mode::{ Token, KeyPair };
use egg_mode::tweet::DraftTweet;
use time::Duration;
use slog::Drain;

use std::process::{ Command, Output };
use std::io::prelude::*;
use std::io::{ BufReader, Result };
use std::fs::{ File, OpenOptions };

static MESSAGE: &str =
    "A new nightly has been released. See what has changed since the last \
    one ";
static GITHUB_URL: &str =
    "https://github.com/rust-lang/rust/commits/master?since=";

fn main() {
    let mut curr_version = get_current_version();
    let one_day = Duration::days(1);
    let log_path = "tweet.log";
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(log_path)
        .unwrap();

    let decorator = slog_term::PlainDecorator::new(file);
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    let _log = slog::Logger::root(drain, o!());

    info!(_log, "Starting up");

    let token = Token::Access {
        consumer: KeyPair::new(read("consumer.key"), read("consumer.secret")),
        access: KeyPair::new(read("access.key"), read("access.secret")),
    };

    loop {
        // Check if there is an update and if there is tweet it
        if update() {
            let tweet_message = MESSAGE.to_owned() + GITHUB_URL + &curr_version;
            // Update the current running version
            curr_version = get_current_version();
            let tweet = DraftTweet::new(&tweet_message);
            tweet.send(&token).unwrap();
            info!(_log, "New update posted");
        } else {
            info!(_log, "No update for {}", curr_version);
        }
        // Wait till tomorrow to do it again
        std::thread::sleep(one_day.to_std().unwrap());
    }


}

fn get_current_version() -> String {
    let output = rustc(vec!["+nightly", "--version"]).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    // This extracts the date string from asking what version is in use. Since
    // nightly is based off a YYYY-MM-DD string this is the best way to get it
    let mut done: String =
        stdout.split_whitespace()
          .filter(|s| s.find('-') != s.rfind('-'))
          .collect();
    done.pop();
    done
}

fn update() -> bool {
    let output = rustup(vec!["update", "nightly"]).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    if stdout.contains("unchanged") {
        false
    } else {
        true
    }
}

fn rustup(args: Vec<&'static str>) -> Result<Output> {
    Command::new("rustup")
        .args(args)
        .output()
}

fn rustc(args: Vec<&str>) -> Result<Output> {
    Command::new("rustc")
        .args(args)
        .output()
}

fn read(path: &str) -> String {
    let mut buffer = String::new();
    let mut reader = BufReader::new(File::open(path).unwrap());
    let _ = reader.read_line(&mut buffer);
    buffer.trim().to_owned()
}
