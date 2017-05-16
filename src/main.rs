#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate slog_async;
extern crate time;
extern crate egg_mode;
#[macro_use]
extern crate error_chain;

use egg_mode::{ Token, KeyPair };
use egg_mode::tweet::DraftTweet;
use time::Duration;
use slog::{ Drain, Logger };

use std::process::{ Command, Output };
use std::io::prelude::*;
use std::io::BufReader;
use std::fs::{ File, OpenOptions };

error_chain!();

const MESSAGE: &str =
    "A new nightly has been released. See what has changed since the last \
    one https://github.com/rust-lang/rust/commits/master?since=";

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

    loop {
        if let Err(ref e) = run(&log) {
            error!(log, "{}", e);
            for e in e.iter().skip(1) {
                error!(log, "Caused by: {}", e);
            }

            if let Some(backtrace) = e.backtrace() {
                error!(log, "Backtrace: {:?}", backtrace);
            }
        }
    }
}

fn run(log: &Logger) -> Result<()> {
    // Update the current running version each day
    let curr_version = get_rustc_version()?;

    // Check if there is an update and if there is tweet it
    match update()? {
        Updated::Yes => {
            // Create the tweet and token then send it
            DraftTweet::new(&[MESSAGE, &curr_version].concat().to_owned())
                .send(&Token::Access {
                    consumer: KeyPair::new(read("consumer.key")?,
                                           read("consumer.secret")?),
                    access: KeyPair::new(read("access.key")?,
                                         read("access.secret")?),
                })
                .chain_err(|| "Tweet failed to send")?;
            info!(log, "New update posted");
        },
        Updated::No => info!(log, "No update for {}", curr_version),
    }
    // Wait till tomorrow to do it again
    std::thread::sleep(Duration::days(1)
                       .to_std()
                       .chain_err(|| "Unable to turn Time into StdTime")?);
    Ok(())
}

#[inline(always)]
/// Get the nightly version of rustc on the system
fn get_rustc_version() -> Result<String> {
    Ok(String::from_utf8(rustc(vec!["+nightly", "--version"])?.stdout)
                        .chain_err(|| "Unable to convert Vec<u8> to String")?
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
    Ok(String::from_utf8(rustup(vec!["update", "nightly"])?.stdout)
                        .chain_err(|| "Unable to convert Vec<u8> to String")?
                        .contains("unchanged")
                        .into())
}

#[inline(always)]
/// Run rustup commands given a vector of arguments to it
fn rustup(args: Vec<&str>) -> Result<Output> {
    Command::new("rustup")
        .args(&args)
        .output()
        .chain_err(|| "Unable to execute rustup ".to_owned() +
                      &args.iter().fold(String::new(), |acc, &x| acc + x + " "))
}

#[inline(always)]
/// Run rustc commands given a vector of arguments to it
fn rustc(args: Vec<&str>) -> Result<Output> {
    Command::new("rustc")
        .args(&args)
        .output()
        .chain_err(|| "Unable to execute rustc ".to_owned() +
                      &args.iter().fold(String::new(), |acc, &x| acc + x + " "))
}

#[inline(always)]
/// Read the token from a file into a String.
fn read(path: &str) -> Result<String> {
    let mut buffer = String::new();
    let mut reader =
        BufReader::new(
            File::open(path)
                .chain_err(|| "Unable to open ".to_owned() + path)?);
    reader
        .read_line(&mut buffer)
        .chain_err(|| "Unable to read from ".to_owned() + path)?;
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
