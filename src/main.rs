extern crate clap;
use clap::{App, SubCommand};


fn main() {
    let matches = App::new("SuperClip")
                      .version("0.1")
                      .author("Simon M. <git@simon.marache.net>")
                      .about("Your global clipboard")
                      .subcommand(SubCommand::with_name("copy")
                                  .about("Copy data into the clipboard"))
                      .subcommand(SubCommand::with_name("paste")
                                  .about("Print the clipboard content"))
                      .get_matches();

    println!("Hello, world!");

    if let Some(_matches) = matches.subcommand_matches("copy") {
        println!("Copy");
    }
    if let Some(_matches) = matches.subcommand_matches("paste") {
        println!("Paste");
    }
}
