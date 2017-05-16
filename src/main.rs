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
    let curr_version = get_current_version()?;
    let token = Token::Access {
        consumer: KeyPair::new(read("consumer.key")?, read("consumer.secret")?),
        access: KeyPair::new(read("access.key")?, read("access.secret")?),
    };

    // Check if there is an update and if there is tweet it
    if update()? {
        DraftTweet::new(&[MESSAGE, &curr_version].concat().to_owned())
            .send(&token)
            .chain_err(|| "Tweet failed to send")?;
        info!(log, "New update posted");
    } else {
        info!(log, "No update for {}", curr_version);
    }
    // Wait till tomorrow to do it again
    std::thread::sleep(Duration::days(1)
                       .to_std()
                       .chain_err(|| "Unable to turn Time into StdTime")?);
    Ok(())
}

#[inline(always)]
fn get_current_version() -> Result<String> {
    let output = rustc(vec!["+nightly", "--version"])?;
    let stdout = String::from_utf8(output.stdout)
                        .chain_err(|| "Unable to convert Vec<u8> to String")?;
    // This extracts the date string from asking what version is in use. Since
    // nightly is based off a YYYY-MM-DD string this is the best way to get it
    let mut done: String =
        stdout.split_whitespace()
          .filter(|s| s.find('-') != s.rfind('-'))
          .collect();
    done.pop();
    Ok(done)
}

#[inline(always)]
fn update() -> Result<bool> {
    let output = rustup(vec!["update", "nightly"])?;
    let stdout = String::from_utf8(output.stdout)
                        .chain_err(|| "Unable to convert Vec<u8> to String")?;

    Ok(!stdout.contains("unchanged"))
}

#[inline(always)]
fn rustup(args: Vec<&str>) -> Result<Output> {
    Command::new("rustup")
        .args(&args)
        .output()
        .chain_err(|| "Unable to execute rustc".to_owned() +
                      &args.iter().fold(String::new(), |acc, &x| acc + x + " "))
}

#[inline(always)]
fn rustc(args: Vec<&str>) -> Result<Output> {
    Command::new("rustc")
        .args(&args)
        .output()
        .chain_err(|| "Unable to execute rustc".to_owned() +
                      &args.iter().fold(String::new(), |acc, &x| acc + x + " "))
}

#[inline(always)]
fn read(path: &str) -> Result<String> {
    let mut buffer = String::new();
    let mut reader =
        BufReader::new(
            File::open(path)
                .chain_err(|| "Unable to open ".to_owned() + path)?);
    reader.read_line(&mut buffer).chain_err(|| "Unable to read from ".to_owned() + path)?;
    Ok(buffer.trim().to_owned())
}
