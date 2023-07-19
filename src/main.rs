#[macro_use]
extern crate log;
extern crate henchman;

use henchman::{ServerConfig, run_server};

use tokio::runtime::Runtime;

use getopts::Options;

use std::env;
use std::path::{PathBuf};

use futures::FutureExt;

// current directory by default
// const DEFAULT_TASK_DIR: &'static str = ".";
// const DEFAULT_LISTEN_ADDR: &'static str = "0.0.0.0:8080";

#[derive(Debug)]
struct ParsedArgs {
    // dir: Option<String>,
    // listen: Option<String>,
    config: String
}

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

fn parse_args() -> Option<ParsedArgs> {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.reqopt("c", "config", "Server configuration file", "CONFIG");
    // opts.optopt("d", "dir", "Tasks directory (default is current directory)", "DIR");
    // opts.optopt("l", "listen", &format!("Address to listen on (default {})", DEFAULT_LISTEN_ADDR), "LISTEN");

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            eprintln!("Error: {}\n", f.to_string());
            print_usage(&program, opts);
            return None;
        }
    };

    let parsed = ParsedArgs {
        // dir: matches.opt_str("dir"),
        // listen: matches.opt_str("listen"),
        config: matches.opt_str("config").unwrap_or_else(||
            panic!("Required field not provided")) // should have already been validated
    };

    Some(parsed)
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Stdout)
        .init();

    let args = match parse_args() {
        Some(a) => a,
        None => {
            std::process::exit(1) // already printed usage
        }
    };

    // let listen_addr = match env::var("LISTEN_ADDR") {
    //     Ok(s) => s.parse::<SocketAddr>().map_err(|e| {
    //         error!("Error parsing listen address: {:?}", e);
    //         AppError::InvalidListenAddr
    //     }),
    //     Err(env::VarError::NotPresent) => Ok(DEFAULT_LISTEN_ADDR.parse().unwrap()),
    //     Err(e) => {
    //         error!("Error reading listen address: {:?}", e);
    //         Err(AppError::InvalidListenAddr)
    //     }
    // }?;

    // let task_dir: PathBuf = args.dir.map(|x| PathBuf::from(x)).unwrap_or(PathBuf::from(DEFAULT_TASK_DIR));

    let config = ServerConfig {
        // listen_addr,
        // task_dir,
        config: PathBuf::from(args.config)
    };

    let runtime = Runtime::new()?;

    let (_shutdown_signal_tx, shutdown_signal_rx) = tokio::sync::oneshot::channel::<()>();

    runtime.block_on(run_server(
        config, |addr| {
            info!("Listening on: http://{}", addr.to_string());
        },
        shutdown_signal_rx.map(|_| ())))
        .map_err(|e| e as Box<dyn std::error::Error>)?; // explicit cast to avoid strange 'into' conversions

    info!("Clean shutdown completed");

    Ok(())
}
