#[macro_use] extern crate slog;
extern crate egg_mode;
extern crate slog_term;
extern crate slog_async;
extern crate time;
extern crate tokio_core;

use egg_mode::{ Token, KeyPair };
use egg_mode::tweet::DraftTweet;
use time::Duration;
use slog::{ Drain, Logger };
use tokio_core::reactor::Core;

use std::error::Error;
use std::process::{ Command, Output };
use std::io::prelude::*;
use std::io::BufReader;
use std::fs::{ File, OpenOptions };

const MESSAGE: &str =
    "A new nightly has been released. See what has changed since the last \
    one https://github.com/rust-lang/rust/commits/master?since=";

type Result<T> = std::result::Result<T, Box<Error>>;

fn main() {
    let log =
        slog::Logger::root(slog_async::Async::new(
                slog_term::FullFormat::new(
                    slog_term::PlainDecorator::new(
                        OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open("tweet.log").unwrap()
                    )
                ).build().fuse()
        ).build().fuse(), o!());

    info!(log, "Starting up");

    let mut core = Core::new().unwrap();
    loop {
        if let Err(ref e) = run(&log, &mut core) {
            error!(log, "{}", e);
        }
    }
}

#[inline(always)]
/// This is the main driver for whther there should be a tweet or not. All
/// of the failures get passed up to this return value, or it gets on `Ok`
/// if everything worked out. The `main` function handles logging any errors
/// if they occur.
fn run(log: &Logger, core: &mut Core) -> Result<()> {
    // Update the current running version each day
    let curr_version = get_rustc_version()?;

    // Check if there is an update and if there is tweet it
    match update()? {
        Updated::Yes => {
            let handle = core.handle();
            // Create the tweet and token then send it
            core.run(DraftTweet::new(String::from(MESSAGE) + &curr_version)
                .send(&Token::Access {
                    consumer: KeyPair::new(read("consumer.key")?,
                                           read("consumer.secret")?),
                    access: KeyPair::new(read("access.key")?,
                                         read("access.secret")?),
                }, &handle))?;
            info!(log, "New update posted");
        },
        Updated::No => info!(log, "No update for {}", curr_version),
    }
    // Wait till tomorrow to do it again
    std::thread::sleep(Duration::days(1).to_std()?);
    Ok(())
}

#[inline(always)]
/// Get the nightly version of rustc on the system
fn get_rustc_version() -> Result<String> {
    Ok(String::from_utf8(rustc(["+nightly", "--version"])?.stdout)?
                        // This part extracts the date string from asking what
                        // version is in use. Since nightly is based off a
                        // YYYY-MM-DD string this is the best way to get it
                        .split_whitespace()
                        // It should be noted that this is guaranteed to only
                        // return one value. Basically this checks if the
                        // string has two - values.
                        .filter(|s| s.find('-') != s.rfind('-'))
                        .map(|s| {
                            // We need to pop off the ) in the string here
                            // which is YYYY-MM-DD)
                            let mut s = s.to_owned();
                            s.pop();
                            s
                        }).collect())
}

#[inline(always)]
/// Run the update function and let us know if anything is unchanged
fn update() -> Result<Updated> {
    Ok(String::from_utf8(rustup(["update", "nightly"])?.stdout)?
                        .contains("unchanged")
                        .into())
}

#[inline(always)]
/// Run rustup commands given a vector of arguments to it
fn rustup<T: AsRef<[&'static str]>>(args: T) -> Result<Output> {
    Ok(Command::new("rustup")
        .args(args.as_ref())
        .output()?)
}

#[inline(always)]
/// Run rustc commands given a vector of arguments to it
fn rustc<T: AsRef<[&'static str]>>(args: T) -> Result<Output> {
    Ok(Command::new("rustc")
        .args(args.as_ref())
        .output()?)
}

#[inline(always)]
/// Read the token from a file into a String.
fn read(path: &str) -> Result<String> {
    let mut buffer = String::new();
    let mut reader = BufReader::new(File::open(path)?);
    reader.read_line(&mut buffer)?;
    Ok(buffer.trim().to_owned())
}

/// Lets us know if the nightly version changed at all
enum Updated {
    Yes,
    No,
}

/// Convenience function to automatically convert the boolean into the Enum
/// Updated
impl From<bool> for Updated {
    fn from(val: bool) -> Self {
        // Unchanged contains("unchanged") returns true if unchanged
        // so we need to flip the values here in this enum
        match val {
            true => Updated::No,
            false => Updated::Yes
        }
    }
}
