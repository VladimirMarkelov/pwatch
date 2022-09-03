use std::env;
use std::process::exit;

use getopts::{Matches, Options};
use sysinfo::Pid;

const GRAPH_AREA: u16 = 5;

// How CPU and memory graphs of the same process are displayed
#[derive(PartialEq)]
pub(crate) enum Pack {
    // TODO: Auto, // depends on the number of processes
    Line, // CPU and MEM graphs occupy the whole terminal width
    Side, // CPU goes first and takes half of the screen, MEM follows it and takes the rest
}

// Graph details: a user can choose lower details if terminal font does not include all required characters
#[derive(Copy, Clone)]
pub(crate) enum Detail {
    Low,    // Only full and empty blocks are used
    Medium, // Full, half-full, and empty blocks are used
    High,   // Nine blocks from empty one to full one with 1/8 step
}

// What to show as process title
#[derive(Copy, Clone)]
pub(crate) enum TitleMode {
    Cmd,   // full command line (the end of it if it is too long)
    Exe,   // full path to binary
    Title, // binary name
}

// Which resource graphs to show
#[derive(Copy, Clone, PartialEq)]
pub(crate) enum Graph {
    All,
    Mem,
    Cpu,
}

// How to show CPU and MEM graphs
#[derive(Copy, Clone, PartialEq)]
pub(crate) enum GraphPosition {
    Auto,  // Automatic selection
    Sided, // Always side by side
    Top,   // One on top of another
}

pub(crate) struct Config {
    // TODO: pub(crate) pack: Pack,            // How to show CPU and MEM of the same process
    // TODO: pub(crate) no_cpu: bool,          // do not show CPU (unused yet)
    // TODO: pub(crate) no_mem: bool,          // do not show MEM (unused yet)
    pub(crate) pid_list: Vec<Pid>, // list of process PIDs provided by a user in command-line
    pub(crate) filter: String,     // regular expression to filter process by their name/path to binary
    pub(crate) detail: Detail,     // Graph details (set of characters used to display graphs)
    pub(crate) scale_max: bool, // How to scale MEM graph: true - from 0 ro all-time max, false - from displayed min to max
    pub(crate) freq: u64,       // process stats refresh rate in range 0.25s .. 10s
    pub(crate) title_mode: TitleMode, // what use for a process title when displaying it
    pub(crate) graphs: Graph,
    pub(crate) graph_pos: GraphPosition,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            // TODO: pack: Pack::Auto,
            // TODO: no_cpu: false,
            // TODO: no_mem: false,
            pid_list: Vec::new(),
            detail: Detail::High,
            filter: String::new(),
            scale_max: false,
            freq: 1_000,
            title_mode: TitleMode::Cmd,
            graphs: Graph::All,
            graph_pos: GraphPosition::Auto,
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

    pub(crate) fn switch_title_type(&mut self) {
        let old = self.title_mode;
        self.title_mode = match old {
            TitleMode::Cmd => TitleMode::Exe,
            TitleMode::Exe => TitleMode::Title,
            TitleMode::Title => TitleMode::Cmd,
        };
    }

    pub(crate) fn switch_quality(&mut self) {
        let old = self.detail;
        self.detail = match old {
            Detail::High => Detail::Medium,
            Detail::Medium => Detail::Low,
            Detail::Low => Detail::High,
        };
    }

    pub(crate) fn switch_graphs(&mut self) {
        let old = self.graphs;
        self.graphs = match old {
            Graph::All => Graph::Mem,
            Graph::Mem => Graph::Cpu,
            Graph::Cpu => Graph::All,
        };
    }

    pub(crate) fn min_graph_height(&self) -> u16 {
        if self.graph_pos == GraphPosition::Top && self.graphs == Graph::All {
            // 2 graphs with +/-, title, IO
            GRAPH_AREA * 2 + 2 + 2
        } else {
            // Graph with +/-, title, IO
            GRAPH_AREA + 1 + 1 + 1
        }
    }

    pub(crate) fn max_graph_height(&self) -> u16 {
        if self.graph_pos == GraphPosition::Sided || self.graphs != Graph::All {
            // Graph with +/-, title, IO
            GRAPH_AREA + 1 + 1 + 1
        } else {
            // 2 graphs with +/-, title, IO
            GRAPH_AREA * 2 + 2 + 2
        }
    }

    pub(crate) fn packer(&self, proc_count: usize, height: u16) -> Pack {
        match self.graph_pos {
            GraphPosition::Sided => Pack::Side,
            GraphPosition::Top => Pack::Line,
            GraphPosition::Auto => {
                let max = self.max_graph_height();
                if max as usize * proc_count <= height as usize {
                    Pack::Line
                } else {
                    Pack::Side
                }
            }
        }
    }
    pub(crate) fn graph_height(&self, proc_count: usize, height: u16) -> u16 {
        let mut h = if self.packer(proc_count, height) == Pack::Line {
            self.max_graph_height()
        } else {
            self.min_graph_height()
        };
        if proc_count == 0 {
            return h;
        }
        let ph = h as usize * proc_count;
        if ph < height as usize {
            let diff = height as usize - ph;
            h += (diff / proc_count) as u16;
        }
        h
    }

    pub(crate) fn visible_count(&self, proc_count: usize, height: u16) -> usize {
        let h = self.graph_height(proc_count, height);
        let vis = (height / h) as usize;
        if vis < proc_count {
            vis
        } else {
            proc_count
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
    opts.optopt("s", "scale", "Memory graph scaling mode", "zero | min");
    opts.optopt("t", "title", "Set process title", "name | path | cmd");
    opts.optopt("g", "graphs", "Select which graphs to show", "all | mem | cpu");

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

    if let Some(val) = matches.opt_str("q") {
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

    if let Some(s) = matches.opt_str("s") {
        conf.scale_max = match s.as_str() {
            "zero" => true,
            "min" => false,
            _ => {
                eprintln!("Invalid value '{}' for scale. Must be 'zero' or 'min'", s);
                print_usage(&program, &opts);
                exit(1);
            }
        }
    }

    if let Some(t) = matches.opt_str("t") {
        conf.title_mode = match t.as_str() {
            "name" => TitleMode::Title,
            "path" => TitleMode::Exe,
            "cmd" => TitleMode::Cmd,
            _ => {
                eprintln!("Invalid value '{}' for title. Must be one of 'name', 'path', and 'cmd'", t);
                print_usage(&program, &opts);
                exit(1);
            }
        }
    }

    if let Some(t) = matches.opt_str("g") {
        conf.graphs = match t.as_str() {
            "all" => Graph::All,
            "mem" => Graph::Mem,
            "cpu" => Graph::Cpu,
            _ => {
                eprintln!("Invalid value '{}' for graphs. Must be one of 'all', 'mem', and 'cpu'", t);
                print_usage(&program, &opts);
                exit(1);
            }
        }
    }

    let names = &matches.free[0];
    let is_pid = names.chars().all(|c| c.is_numeric() || c == ',');
    if is_pid {
        for pd in names.split(',') {
            if let Ok(i) = pd.parse::<usize>() {
                conf.pid_list.push(Pid::from(i));
            }
        }
    } else {
        conf.filter = names.to_string();
    }

    conf
}
