use std::io::{stdout, Write};
use std::process::exit;
use std::time::SystemTime;

use crate::config::{Config, Pack};
use crate::counter::{draw_counter, Process};
use crate::ux::format_duration;

// use log::*;

use crossterm::{cursor, /*execute,*/ queue, style, terminal, Result};
use regex::Regex;
use sysinfo::{ProcessExt, ProcessorExt, System, SystemExt};

pub struct Layout {
    pub w: u16,
    pub h: u16,
    pub procs: Vec<Process>,
    pub config: Config,
    pub system: System,
    pub cpu_usage: u64,  // total CPU%
    pub mem_usage: u64,  // total MEM%
    pub top_item: usize, // first shown counter (used only if there are hidden counters)
    pub mark_since: Option<SystemTime>,
}

pub const MIN_HEIGHT: u16 = 5;
pub const SCROLL_HOME: i32 = -9_999_999;
pub const SCROLL_END: i32 = 9_999_999;

impl Layout {
    pub fn new(config: Config) -> Layout {
        let (w, h) = if let Ok((cols, rows)) = terminal::size() {
            if cols < 30 || rows < 10 {
                eprintln!("Requires terminal width at least 30 and height at least 10 characters");
                exit(1);
            }
            (cols, rows)
        } else {
            (40, 20)
        };
        Layout {
            w,
            h,
            procs: Vec::new(),
            system: System::new_all(),
            config,
            cpu_usage: 0,
            mem_usage: 0,
            top_item: 0,
            mark_since: None,
        }
    }

    pub fn size_changed(&mut self, w: u16, h: u16) {
        if self.w == w && self.h == h {
            return;
        }
        self.w = w;
        self.h = h;
        self.place();
    }

    // Returns the number of maximum displayed process on the screen, height of every process, and
    // how to place mem and cpu counters: side-by-side or one-on-top-of-another
    fn max_shown(&self) -> (usize, u16, Pack) {
        let procs = self.procs.len();
        let h_side = MIN_HEIGHT + 3; // title*2+graph+extra
        let h_line = MIN_HEIGHT * 2 + 4; // title*2+2*graph+2*extra

        let h = self.h - 1; // total CPU%/MEM%, alive/hidden/dead
                            // info!("h_side {}, h_line {}, h {}", h_side, h_line, h);

        let max_n_l = h / h_line;
        let max_n_s = h / h_side;
        let (cnt, mut hgt, tp) = match self.config.pack {
            Pack::Line => (max_n_l as usize, h_line, Pack::Line),
            Pack::Side => (max_n_s as usize, h_side, Pack::Side),
            Pack::Auto => {
                if max_n_l >= procs as u16 {
                    (max_n_l as usize, h_line, Pack::Line)
                } else {
                    (max_n_s as usize, h_side, Pack::Side)
                }
            }
        };

        if procs != 0 {
            let shown = if procs < cnt { procs } else { cnt };
            let used = shown as u16 * hgt;
            // info!("procs {}, used {}, hgt {}, h {}", procs, used, hgt, h);
            if h - used > procs as u16 {
                hgt += (h - used) / procs as u16;
            }
        }
        // info!("cnt {}, hgt {}", cnt, hgt);

        (cnt, hgt, tp)
    }

    fn update_procs(&mut self) {
        let procs = self.system.get_processes();
        for ap in self.procs.iter_mut() {
            if ap.dead {
                continue;
            }
            if procs.get(&ap.pid).is_none() {
                ap.dead = true;
                ap.dead_since = Some(SystemTime::now());
            }
        }

        if !self.config.filter.is_empty() {
            let flt = format!("(?i){}", self.config.filter);
            let rx = Regex::new(&flt);
            let low_flt = self.config.filter.to_lowercase();
            // let procs = self.system.get_process_by_name(&self.config.filter);
            // for p in &procs {
            for p in procs.values() {
                // println!("{} - {} : {}", p.name(), p.memory(), p.cpu_usage());
                let pname = p.exe().to_string_lossy();
                let full_name = format!("{} {}", pname, p.name());
                let low_name = full_name.to_lowercase();
                let include = if let Ok(ref rr) = rx { rr.is_match(&low_name) } else { low_name.contains(&low_flt) };
                if !include {
                    continue;
                }

                update_proc(&mut self.procs, p);
            }
            self.procs.sort();
            return;
        }

        for pd in &self.config.pid_list {
            match procs.get(pd) {
                None => {
                    for ap in self.procs.iter_mut() {
                        if ap.dead {
                            continue;
                        }
                        if ap.pid == *pd {
                            ap.dead = true;
                            ap.dead_since = Some(SystemTime::now());
                        }
                    }
                }
                Some(p) => {
                    update_proc(&mut self.procs, p);
                }
            }
        }
        self.procs.sort();
    }

    fn update_total(&mut self) {
        let mut total = 0.0f32;
        let mut used = 0.0f32;
        for pr in self.system.get_processors().iter() {
            total += 100.0;
            used += pr.get_cpu_usage();
        }
        total = used / total;
        self.cpu_usage = total.round() as u64;
        self.mem_usage = self.system.get_used_memory() * 100 / self.system.get_total_memory();
    }

    pub fn update(&mut self) {
        self.system.refresh_processes();
        self.system.refresh_cpu();
        self.system.refresh_memory();

        self.update_procs();
        self.update_total();
    }

    // delete all dead processes that are invisible
    // pub fn cleanup(&mut self) {
    //     let (mx, _h, _pack) = self.max_shown();
    //     let l = self.procs.len() ;
    //     if l <= mx {
    //         return;
    //     }
    //     let mut i = 0;
    //     self.procs.retain(|v| (i < l || !v.dead, i += 1).0);
    // }

    pub fn place(&mut self) {
        if self.procs.is_empty() {
            return;
        }
        let l = self.procs.len();
        let (mx, h, pack) = self.max_shown();
        // info!("Max {}, height {}", mx, h);
        // for idx in 0..mx {
        //     if idx >= l {
        //         return;
        //     }
        //     self.procs[idx].dim(0,  idx as u16 * h + 1, self.w, h, pack == Pack::Side);
        // }
        // for idx in mx..l {
        //     self.procs[idx].dim(0, 0, 0, 0, false);
        // }
        for idx in 0..l {
            if idx < self.top_item || idx >= self.top_item + mx {
                self.procs[idx].dim(0, 0, 0, 0, false);
            // info!("HIDE: {}", idx);
            } else {
                self.procs[idx].dim(0, (idx - self.top_item) as u16 * h + 1, self.w, h, pack == Pack::Side);
                // info!("SHOW: {}", idx);
            }
        }
    }

    // Retunrs total number of watched processes, hidden, and dead
    pub(crate) fn proc_totals(&self) -> (usize, usize, usize) {
        let total = self.procs.len();
        if total == 0 {
            return (0, 0, 0);
        }
        let mut hidden = 0usize;
        let mut dead = 0usize;
        for p in self.procs.iter() {
            if p.dead {
                dead += 1;
            }
            if p.w == 0 {
                hidden += 1;
            }
        }
        (total, hidden, dead)
    }

    pub fn draw_counters(&mut self) -> Result<()> {
        let mut stdout = stdout();
        // queue!(stdout, style::ResetColor, terminal::Clear(ClearType::All))?;
        draw_totals(&mut stdout, self)?;
        for (idx, proc) in self.procs.iter_mut().enumerate() {
            if idx < self.top_item {
                continue;
            }
            if proc.w == 0 {
                break;
            }
            draw_counter(&mut stdout, proc, idx + 1, &self.config)?;
        }
        stdout.flush()?;
        Ok(())
    }

    pub fn scroll(&mut self, shift: i32) -> bool {
        let (t, h, _d) = self.proc_totals();
        if h == 0 {
            return false;
        }
        let shown = t - h;
        // info!("{}. {}[{}] - {} down: {}", self.top_item, t, h, shown, shift);
        if shift == SCROLL_HOME {
            let res = self.top_item != 0;
            self.top_item = 0;
            return res;
        } else if shift == SCROLL_END {
            let res = self.top_item < t - shown;
            self.top_item = t - shown;
            return res;
        } else if shift < 0 {
            if self.top_item == 0 {
                return false;
            }
            let ushift = (-shift) as usize;
            if ushift >= self.top_item {
                self.top_item = 0;
            } else {
                self.top_item -= ushift;
            }
            return true;
        } else if shift > 0 {
            let ushift = shift as usize;
            if self.top_item + shown >= t {
                return false;
            }
            if self.top_item + shown + ushift > t {
                self.top_item = t - shown;
                return true;
            }
            self.top_item += ushift;
            return true;
        }
        false
    }

    pub fn toggle_mark(&mut self) {
        let is_off = self.mark_since.is_none();
        if is_off {
            self.mark_since = Some(SystemTime::now());
        } else {
            self.mark_since = None;
        };
        for p in self.procs.iter_mut() {
            p.toggle_mark();
        }
    }

    pub fn reset_max(&mut self) {
        for p in self.procs.iter_mut() {
            p.reset_max();
        }
    }

    pub fn counter_height(&self) -> u16 {
        let mut new_h = 0u16;
        for p in self.procs.iter() {
            if p.h != 0 {
                new_h = p.h;
                break;
            }
        }
        new_h
    }
}

fn update_proc<P>(procs: &mut Vec<Process>, p: &P)
where
    P: ProcessExt,
{
    for ap in procs.iter_mut() {
        if ap.dead {
            continue;
        }
        if ap.pid == p.pid() {
            let prc: u64 = p.cpu_usage().round() as u64;
            ap.add(prc, p.memory());
            let du = p.disk_usage();
            ap.io_w_total = du.total_written_bytes / 1024;
            ap.io_r_total = du.total_read_bytes / 1024;
            ap.io_w_delta = du.written_bytes / 1024;
            ap.io_r_delta = du.read_bytes / 1024;
            return;
        }
    }

    let mut title = String::new();
    for s in p.cmd().iter() {
        if !title.is_empty() {
            title += " ";
        }
        title += s;
    }
    // let mut title = p.exe().to_string_lossy().to_string();
    if title.is_empty() {
        title = p.name().to_string();
    };
    let mut ap = Process::new(title, p.pid(), false);
    let prc: u64 = p.cpu_usage().round() as u64;
    ap.add(prc, p.memory());
    let du = p.disk_usage();
    ap.io_w_total = du.total_written_bytes / 1024;
    ap.io_r_total = du.total_read_bytes / 1024;
    procs.push(ap);
}

pub fn draw_totals<W>(w: &mut W, layout: &Layout) -> Result<()>
where
    W: Write,
{
    let (t, h, d) = layout.proc_totals();
    let mut mark = if let Some(dt) = layout.mark_since {
        match dt.elapsed() {
            Err(_) => String::new(),
            Ok(since) => format_duration(since),
        }
    } else {
        String::new()
    };
    let mut title = if layout.w < 60 {
        if !mark.is_empty() {
            mark = format!("  D: {}", mark);
        };
        format!("{:3}%:{:3}% | {:3}:{:3}:{:3}{}", layout.cpu_usage, layout.mem_usage, t, h, d, mark)
    } else {
        if !mark.is_empty() {
            mark = format!("  Delta for last {}", mark);
        };
        format!(
            "CPU: {:3}%  MEM: {:3}% | Total: {}  Hidden: {}  Dead: {}{}",
            layout.cpu_usage, layout.mem_usage, t, h, d, mark
        )
    };
    if title.len() < layout.w as usize {
        title += &" ".repeat(layout.w as usize - title.len());
    }
    queue!(w, cursor::MoveTo(0, 0), style::Print(title))
}
