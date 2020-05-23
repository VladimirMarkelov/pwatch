use std::env;
use std::process::exit;

use getopts::{Matches, Options};
use sysinfo::Pid;

// How CPU and memory graphs of the same process are displayed
#[derive(Clone, PartialEq)]
pub(crate) enum Pack {
    Auto, // depends on the number of processes
    Line, // CPU and MEM graphs occupy the whole terminal width
    Side, // CPU goes first and takes half of the screen, MEM follows it and takes the rest
}

// Graph details: a user can choose lower details if terminal font does not include all required characters
#[derive(Clone)]
pub(crate) enum Detail {
    Low,    // Only full and empty blocks are used
    Medium, // Full, half-full, and empty blocks are used
    High,   // Nine blocks from empty one to full one with 1/8 step
}

#[derive(Clone)]
pub(crate) struct Config {
    pub(crate) pack: Pack,         // How to show CPU and MEM of the same process
    pub(crate) no_cpu: bool,       // do not show CPU (unused yet)
    pub(crate) no_mem: bool,       // do not show MEM (unused yet)
    pub(crate) pid_list: Vec<Pid>, // list of process PIDs provided by a user in command-line
    pub(crate) filter: String,     // regular expression to filter process by their name/path to binary
    pub(crate) detail: Detail,     // Graph details (set of characters used to display graphs)
    pub(crate) scale_max: bool, // How to scale MEM graph: true - from 0 ro all-time max, false - from displayed min to max
    pub(crate) freq: u64,       // process stats refresh rate in range 0.25s .. 10s
}

impl Default for Config {
    fn default() -> Config {
        Config {
            pack: Pack::Auto,
            no_cpu: false,
            no_mem: false,
            pid_list: Vec::new(),
            detail: Detail::High,
            filter: String::new(),
            scale_max: false,
            freq: 1_000,
        }
    }
}

impl Config {
    // Returns the number of used non-empty characters for graphs
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

pub(crate) fn parse_args() -> Config {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut conf = Config::default();

    let mut opts = Options::new();
    opts.optflag("h", "help", "Show this help");
    opts.optopt("q", "quality", "Graphics quality", "high | medium | low");
    opts.optopt("r", "refresh", "Refresh graphics every N milliseconds", "MILLISECONDS");
    opts.optflag("v", "version", "Print application version");

    let matches: Matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", e);
            print_usage(&program, &opts);
            exit(0);
        }
    };

    if matches.opt_present("version") {
        let version = env!("CARGO_PKG_VERSION");
        println!("PWatch Version {}", version);
        exit(0);
    }
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

    let names = &matches.free[0];
    let is_pid = names.chars().all(|c| c.is_numeric() || c == ',');
    if is_pid {
        for pd in names.split(',') {
            if let Ok(i) = pd.parse::<usize>() {
                conf.pid_list.push(i as Pid);
            }
        }
    } else {
        conf.filter = names.to_string();
    }

    conf
}
