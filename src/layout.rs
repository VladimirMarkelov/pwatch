use std::io::{stdout, Write};
use std::time::SystemTime;

use crate::config::{Config, Pack};
use crate::counter::{draw_counter, Process};
use crate::ux::{cut_string, format_duration};

use crossterm::{cursor, queue, style, style::Color, terminal, Result};
use regex::Regex;
use sysinfo::{ProcessExt, ProcessorExt, System, SystemExt};
use unicode_width::UnicodeWidthStr;

pub(crate) struct Layout {
    pub(crate) w: u16,
    pub(crate) h: u16,
    pub(crate) procs: Vec<Process>, // list of monitored processes
    pub(crate) config: Config,
    pub(crate) system: System,
    pub(crate) cpu_usage: u64,  // total CPU%
    pub(crate) mem_usage: u64,  // total MEM%
    pub(crate) top_item: usize, // first shown counter (used only if there are hidden counters)
    pub(crate) mark_since: Option<SystemTime>,
    show_help: bool, // show help bar(true) or total CPU/MEM(false) in the top line
}

pub(crate) const MIN_HEIGHT: u16 = 5; // minimum height of a graph

pub(crate) enum Scroll {
    Home,
    End,
    Up(usize),
    Down(usize),
}

impl Layout {
    pub(crate) fn new(config: Config) -> Layout {
        let (w, h) = if let Ok((cols, rows)) = terminal::size() { (cols, rows) } else { (40, 20) };
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
            show_help: false,
        }
    }

    // Terminal resize event handler
    pub(crate) fn size_changed(&mut self, w: u16, h: u16) {
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
            if h - used > procs as u16 {
                hgt += (h - used) / procs as u16;
            }
        }

        (cnt, hgt, tp)
    }

    // Refresh process list, update CPU/MEM, mark dead ones, and add new ones
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
            for p in procs.values() {
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

    // Calculate total used CPU and MEM.
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

    pub(crate) fn update(&mut self) {
        self.system.refresh_processes();
        self.system.refresh_cpu();
        self.system.refresh_memory();

        self.update_procs();
        self.update_total();
    }

    // Recalculate position of all graphs. Mark ones that are out of screen.
    pub(crate) fn place(&mut self) {
        if self.procs.is_empty() {
            return;
        }
        let l = self.procs.len();
        let (mx, h, pack) = self.max_shown();
        for idx in 0..l {
            if idx < self.top_item || idx >= self.top_item + mx {
                self.procs[idx].dim(0, 0, 0, 0, false); // out of screen
            } else {
                self.procs[idx].dim(0, (idx - self.top_item) as u16 * h + 1, self.w, h, pack == Pack::Side);
            }
        }
    }

    // Returns total number of watched processes, hidden and dead ones
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

    pub(crate) fn draw_counters(&mut self) -> Result<()> {
        let mut stdout = stdout();
        if self.show_help {
            draw_help(&mut stdout, self)?;
        } else {
            draw_totals(&mut stdout, self)?;
        }
        for (idx, proc) in self.procs.iter_mut().enumerate() {
            if idx < self.top_item {
                continue;
            }
            if proc.w == 0 {
                break;
            }
            draw_counter(&mut stdout, proc, idx + 1, self.config.title_mode, &self.config)?;
        }
        stdout.flush()?;
        Ok(())
    }

    pub(crate) fn scroll(&mut self, dir: Scroll) -> bool {
        let (t, h, _d) = self.proc_totals();
        if h == 0 {
            return false;
        }
        let shown = t - h;
        match dir {
            Scroll::Home => {
                let res = self.top_item != 0;
                self.top_item = 0;
                res
            }
            Scroll::End => {
                let res = self.top_item < t - shown;
                self.top_item = t - shown;
                res
            }
            Scroll::Up(shift) => {
                if self.top_item == 0 {
                    return false;
                }
                if shift >= self.top_item {
                    self.top_item = 0;
                } else {
                    self.top_item -= shift;
                }
                true
            }
            Scroll::Down(shift) => {
                if self.top_item + shown >= t {
                    return false;
                }
                if self.top_item + shown + shift > t {
                    self.top_item = t - shown;
                    return true;
                }
                self.top_item += shift;
                true
            }
        }
    }

    pub(crate) fn toggle_mark(&mut self) {
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

    pub(crate) fn reset_max(&mut self) {
        for p in self.procs.iter_mut() {
            p.reset_max();
        }
    }

    pub(crate) fn counter_height(&self) -> u16 {
        let mut new_h = 0u16;
        for p in self.procs.iter() {
            if p.h != 0 {
                new_h = p.h;
                break;
            }
        }
        new_h
    }

    pub(crate) fn switch_help(&mut self) {
        self.show_help = !self.show_help;
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

    let mut cmd = String::new();
    for s in p.cmd().iter() {
        if !cmd.is_empty() {
            cmd += " ";
        }
        cmd += s;
    }
    let exe = p.exe().to_string_lossy().to_string();
    let title = p.name().to_string();

    let mut ap = Process::new(p.pid(), false, cmd, exe, title);
    let prc: u64 = p.cpu_usage().round() as u64;
    ap.add(prc, p.memory());
    let du = p.disk_usage();
    ap.io_w_total = du.total_written_bytes / 1024;
    ap.io_r_total = du.total_read_bytes / 1024;
    procs.push(ap);
}

fn draw_help<W>(w: &mut W, layout: &Layout) -> Result<()>
where
    W: Write,
{
    let help_str = "SPACE: Mark | r: Reset max | F7: Quality | F9: Title | F12: Scale";
    let mut s = cut_string(help_str, layout.w as usize);
    let width = s.width();
    if width < layout.w as usize {
        s += &" ".repeat(layout.w as usize - width);
    }
    queue!(
        w,
        cursor::MoveTo(0, 0),
        style::SetForegroundColor(Color::Black),
        style::SetBackgroundColor(Color::White),
        style::Print(s),
        style::ResetColor
    )
}

fn draw_totals<W>(w: &mut W, layout: &Layout) -> Result<()>
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
