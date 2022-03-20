use std::path::PathBuf;
use std::sync::mpsc::channel;

use clap::Parser;

#[macro_use]
extern crate lazy_static;

mod projfs_provider;
mod zeros_provider;


#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[clap(short, long, default_value = "C:\\proj", parse(from_os_str))]
    projection: PathBuf,

    /// Number of times to greet
    #[clap(short, long, default_value_t = 1)]
    count: u8,
}

fn wait_for_shutdown() {
    let (tx, rx) = channel();

    ctrlc::set_handler(move || tx.send(()).expect("Could not send signal on channel."))
        .expect("Error setting Ctrl-C handler");
    
    println!("Waiting for Ctrl-C...");
    rx.recv().expect("Could not receive from channel.");
    println!("Got it! Exiting..."); 
}


fn main() {
    let args = Args::parse();

    println!("Hello {:?}!", &args.projection);

    let mut runner = projfs_provider::ProjFSRunner::new();
    runner.start(&args.projection, Box::new(zeros_provider::ZerosProvider::new())).unwrap();

    wait_for_shutdown();

    runner.stop().unwrap();

    println!("Shut down");
}
