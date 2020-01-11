#[macro_use]
extern crate log;
extern crate simple_logger;

extern crate tempfile;

extern crate byteorder;
extern crate clap;
extern crate nix;

#[macro_use(event_enum)]
extern crate wayland_client;

use clap::{App, Arg, SubCommand};

const VERSION: &str = "0.1";
const NAME: &str = "SuperClip";

mod copy;

fn main() {
    let matches = App::new(NAME)
        .version(VERSION)
        .author("Simon M. <git@simon.marache.net>")
        .about("Your global clipboard")
        .arg(Arg::with_name("debug").long("--debug").short("d"))
        .subcommand(SubCommand::with_name("copy").about("Copy data into the clipboard"))
        .subcommand(
            SubCommand::with_name("paste")
                .arg(
                    Arg::with_name("show_mime")
                        .long("--show_mime")
                        .short("s")
                        .help("Display the available mime types for the clipboard content"),
                )
                .about("Print the clipboard content"),
        )
        .get_matches();

    if matches.is_present("debug") {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
    } else {
        simple_logger::init_with_level(log::Level::Error).unwrap();
    }

    debug!("{} version {}", NAME, VERSION);

    if let Some(_matches) = matches.subcommand_matches("copy") {
        debug!("Copy");
    }
    if let Some(_matches) = matches.subcommand_matches("paste") {
        debug!("Paste");
        println!(
            "{}",
            copy::load_clipboard_content(_matches.is_present("show_mime"))
                .expect("Cannot load clipboard content")
        );
    }
}
