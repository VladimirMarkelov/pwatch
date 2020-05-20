use std::env;
use std::process::exit;

// use log::*;

use getopts::{Matches, Options};
use sysinfo::Pid;

#[derive(Clone, PartialEq)]
pub enum Pack {
    Auto,
    Line,
    Side,
}

#[derive(Clone)]
pub enum Detail {
    Low,
    Medium,
    High,
}

#[derive(Clone)]
pub struct Config {
    pub pack: Pack,
    pub no_cpu: bool,
    pub no_mem: bool,
    pub pid_list: Vec<Pid>,
    pub filter: String,
    pub detail: Detail,
    pub scale_max: bool,
    pub freq: u64,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            pack: Pack::Auto,
            no_cpu: false,
            no_mem: false,
            pid_list: Vec::new(), /*vec![13492]*/
            detail: Detail::High,
            // filter: "firefox".to_string()/*String::new()*/,
            filter: String::new(),
            scale_max: false,
            freq: 1_000,
        }
    }
}

impl Config {
    pub(crate) fn steps(&self) -> u16 {
        match self.detail {
            Detail::Low => 1,
            Detail::Medium => 2,
            Detail::High => 8,
        }
    }
}

fn print_usage(program: &str, opts: &Options) {
    let brief = format!("Usage: {} NAME|PID [options]", program);
    print!("{}", opts.usage(&brief));
}

// TODO: error?
pub fn parse_args() -> Config {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut conf = Config::default();

    let mut opts = Options::new();
    opts.optflag("h", "help", "Show this help");
    opts.optopt("q", "quality", "Graphics quality", "high | medium | low");
    opts.optopt("r", "refresh", "Refresh graphics every N milliseconds", "MILLISECONDS");
    // opts.optopt("n", "name", "process name", "NAME");

    let matches: Matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", e);
            print_usage(&program, &opts);
            exit(0);
        }
    };

    if matches.opt_present("h") || matches.free.is_empty() {
        print_usage(&program, &opts);
        exit(0);
    }
    if matches.opt_present("q") {
        let val = match matches.opt_str("q") {
            None => String::new(),
            Some(s) => s.to_lowercase(),
        };
        conf.detail = match val.as_str() {
            "high" => Detail::High,
            "medium" => Detail::Medium,
            "low" => Detail::Low,
            _ => {
                eprintln!("Invalid value for 'quality' {}. Must be one of 'high', 'medium', and 'low'", val);
                print_usage(&program, &opts);
                exit(1);
            }
        }
    }
    if matches.opt_present("r") {
        if let Some(v) = matches.opt_str("r") {
            if let Ok(n) = v.parse::<u64>() {
                conf.freq = if n < 250 {
                    250
                } else if n > 10_000 {
                    10_000
                } else {
                    n
                };
            }
        }
    }

    // TODO: multi pids? names?
    // Vec::from_iter(matches.free[1..].iter().cloned())
    // Get only the first value for now
    let names = &matches.free[0];
    let is_pid = names.chars().all(|c| c.is_numeric() || c == ',');
    // info!("{} -{}", names, is_pid);
    if is_pid {
        // TODO: string to list of PIDs
        for pd in names.split(',') {
            // info!(">> {}", pd);
            if let Ok(i) = pd.parse::<usize>() {
                conf.pid_list.push(i as Pid);
            }
        }
    } else {
        conf.filter = names.to_string();
    }

    conf
}
