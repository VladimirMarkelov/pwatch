#![allow(clippy::suspicious_map)]

use std::cmp::{Eq, Ordering};
use std::io::Write;
use std::time::{Duration, SystemTime};
use std::u64;

use crossterm::{cursor, queue, style, style::Color, Result};
// use log::*;
use sysinfo::Pid;
use unicode_width::UnicodeWidthStr;

use crate::config::{Config, Detail};
use crate::ux::{fade_str_left, format_bytes, format_diff, format_duration, format_mem, round_to_hundred, short_round};

const LOW: [char; 2] = [' ', '\u{2588}'];
const MED: [char; 3] = [' ', '\u{2584}', '\u{2588}'];
const HGH: [char; 9] =
    [' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

struct DrawRect {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}
impl Default for DrawRect {
    fn default() -> DrawRect {
        DrawRect { x: 0, y: 0, w: 0, h: 0 }
    }
}
struct DrawVal {
    curr: u64,
    max: u64,
}

pub struct Counter {
    pub values: Vec<u64>,        // last values
    pub display_cnt: usize,      // number or last items to show
    pub max: u64,                // all time max
    pub scale_to: u64,           // scale to this value if auto_scale == false
    pub auto_scale: bool,        // scale to max in range or to max_val
    pub mark_value: Option<u64>, // value when a user pressed a key to mark the position
    w: u16,                      // width and height of graph area
    h: u16,
    pub screen: Vec<char>, // precalculated graph: WxH
    pub gmin: u64,         // range of the graphic
    pub gmax: u64,
}

impl Default for Counter {
    fn default() -> Counter {
        Counter {
            values: Vec::new(),
            display_cnt: 40,
            scale_to: 0,
            max: 0,
            auto_scale: false,
            w: 0,
            h: 0,
            screen: Vec::new(),
            gmin: 0,
            gmax: 0,
            mark_value: None,
        }
    }
}

impl Counter {
    pub fn add(&mut self, val: u64) {
        if val > self.max {
            self.max = val;
        }
        if self.scale_to != 0 && self.scale_to < val {
            self.scale_to = round_to_hundred(val);
        }
        let l = self.values.len();
        if l == 0 || l < self.display_cnt {
            self.values.push(val);
            return;
        }
        for idx in 0..l - 1 {
            self.values[idx] = self.values[idx + 1];
        }
        self.values[l - 1] = val;
    }

    pub fn last(&self) -> u64 {
        if self.values.is_empty() {
            return 0;
        }
        self.values[self.values.len() - 1]
    }

    pub fn last_diff(&self) -> i64 {
        let l = self.values.len();
        if l < 2 {
            return 0;
        }
        let last = self.values[l - 1] as i64;
        let prev = if let Some(p) = self.mark_value { p } else { self.values[l - 2] };
        last as i64 - prev as i64
    }

    pub fn max_last_n(&self, n: usize) -> u64 {
        let mut max = 0u64;
        let vs: &[u64] = if n >= self.values.len() {
            &self.values
        } else {
            let l = self.values.len();
            &self.values[l - n..]
        };
        if let Some(m) = vs.iter().max() {
            max = *m;
        }
        max
    }

    pub fn update(&mut self, neww: u16, newh: u16, conf: &Config) {
        if self.w != neww || self.h != newh {
            self.screen = vec![' '; neww as usize * (newh + 1) as usize];
        } else {
            let _c = self.screen.iter_mut().map(|c| *c = ' ').count();
        }

        if self.values.is_empty() {
            return;
        }

        let max_w = neww as usize;
        let (scale_to, scale_min) = if self.auto_scale {
            if conf.scale_max {
                (self.max, 0)
            } else {
                (self.gmax - self.gmin, self.gmin)
            }
        } else {
            (self.scale_to, 0)
        };
        if scale_to == 0 {
            return;
        }
        let l = self.values.len();
        let vs = if l <= max_w { &self.values } else { &self.values[l - max_w..] };
        let mut start = if l <= neww as usize { neww - (l as u16) } else { 0 };

        let step = scale_to as f64 / newh as f64;
        let mut prev: u64 = u64::MAX;
        for v in vs.iter() {
            let delta = if self.auto_scale { *v - scale_min } else { *v };
            let val = if delta > scale_to { scale_to as f64 } else { delta as f64 };
            let full = (val / step).trunc() as u16;
            let part = (val - (full as f64) * step) / step;

            let xx = start as usize;
            for yy in 0..full {
                let pos = xx + (newh - yy - 1) as usize * neww as usize;
                self.screen[pos] = '\u{2588}';
            }
            let c = char_for_value(part, conf);
            if c != ' ' {
                let pos = xx + (newh - full - 1) as usize * neww as usize;
                self.screen[pos] = c;
            }
            let ch = if prev == u64::MAX || prev == *v {
                ' '
            } else if prev > *v {
                '-'
            } else {
                '+'
            };
            let pos = xx + (newh) as usize * neww as usize;
            self.screen[pos] = ch;
            start += 1;
            prev = *v;
        }
    }

    pub(crate) fn calculate_range(&mut self) {
        if !self.auto_scale || self.values.is_empty() {
            return;
        }
        let mut min = self.values[0];
        let mut max = 0;
        for v in self.values.iter() {
            if min > *v {
                min = *v;
            }
            if max < *v {
                max = *v;
            }
        }
        let (min_rnd, min_coef) = short_round(min, true);
        let (mut max_rnd, max_coef) = short_round(max, false);
        if min_rnd == max_rnd {
            max_rnd += 1;
        }
        self.gmin = min_rnd * min_coef;
        self.gmax = max_rnd * max_coef;
    }

    fn toggle_mark(&mut self) {
        let is_off = self.mark_value.is_none();
        if is_off {
            self.mark_value = Some(self.last());
        } else {
            self.mark_value = None;
        }
    }

    fn reset_max(&mut self) {
        self.max = self.max_last_n(self.w as usize);
        if self.scale_to != 0 {
            self.scale_to = round_to_hundred(self.max);
        }
    }
}

pub struct Process {
    pub cpu: Counter, // CPU history
    pub mem: Counter, // MEM history
    pub pid: Pid,     // process PID
    pub dead: bool,   // whether process is active
    pub cmd: String,  // process command line
    pub x: u16,       // box coordinates to draw all counters
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub sided: bool,     // true: CPU and MEM in one line, false: CPU on top of MEM
    pub io_w_total: u64, // total IO write since start
    pub io_r_total: u64, // total IO read since start
    pub io_w_delta: u64, // IO write since last check
    pub io_r_delta: u64, // IO read since last check
    pub dead_since: Option<SystemTime>,
}

impl PartialEq for Process {
    fn eq(&self, other: &Self) -> bool {
        self.pid == other.pid
    }
}

impl Eq for Process {}

impl Ord for Process {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.dead && !other.dead {
            return Ordering::Greater;
        }
        if !self.dead && other.dead {
            return Ordering::Less;
        }
        if self.dead {
            let sd = self.dead_since.unwrap();
            let od = self.dead_since.unwrap();
            if sd < od {
                return Ordering::Greater;
            }
            if sd > od {
                return Ordering::Less;
            }
            return Ordering::Equal;
        }
        if self.pid > other.pid {
            return Ordering::Less;
        }
        Ordering::Greater
    }
}

impl PartialOrd for Process {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Process {
    pub fn new(cmd: String, pid: Pid, sided: bool) -> Process {
        let mut p = Process {
            cpu: Default::default(),
            mem: Default::default(),
            dead: false,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            io_w_total: 0,
            io_r_total: 0,
            io_w_delta: 0,
            io_r_delta: 0,
            dead_since: None,
            sided,
            pid,
            cmd,
        };
        p.cpu.scale_to = 100;
        p.mem.auto_scale = true;
        p
    }

    // set new dimensions for a counter. Zero width disables drawing the counter
    pub fn dim(&mut self, x: u16, y: u16, w: u16, h: u16, sided: bool) {
        self.x = x;
        self.y = y;
        self.w = w;
        self.h = h;
        self.sided = sided;

        if w == 0 {
            return;
        }

        let cp_w = if self.sided { (w / 2) - 6 } else { w - 6 };
        let mm_w = if self.sided { w - cp_w - 7 } else { w - 7 };
        self.cpu.display_cnt = cp_w as usize;
        self.mem.display_cnt = mm_w as usize;
    }
    pub fn add(&mut self, cpu: u64, mem: u64) {
        if self.cpu.values.is_empty() {
            self.cpu.add(0);
        } else {
            self.cpu.add(cpu);
        }
        self.mem.add(mem);
    }
    pub fn toggle_mark(&mut self) {
        self.mem.toggle_mark();
    }
    pub fn reset_max(&mut self) {
        self.cpu.reset_max();
        self.mem.reset_max();
    }
}

// Blocks:
// 2581 - 1/8
// 2582 - 2/8
// 2583 - 3/8
// 2584 - 1/2  !!
// 2585 - 5/8
// 2586 - 6/8
// 2587 - 7/8
// 2588 - FULL !!
fn char_for_value(val: f64, conf: &Config) -> char {
    if val >= 1.0f64 {
        return '\u{2588}';
    } else if val <= 0.0f64 {
        return ' ';
    };
    let steps = (conf.steps() + 1) as f64;
    let idx = (val * steps) as usize;
    match conf.detail {
        Detail::Low => LOW[idx],
        Detail::Medium => MED[idx],
        Detail::High => HGH[idx],
    }
}

fn draw_spikes<W>(w: &mut W, cnt: &Counter, rect: DrawRect, xshift: u16, dt: Option<SystemTime>) -> Result<()>
where
    W: Write,
{
    if cnt.values.is_empty() {
        return Ok(());
    }

    for yy in 0..rect.h {
        let st = yy as usize * rect.w as usize;
        let end = st + rect.w as usize;
        let slice = &cnt.screen[st..end];
        let s: String = slice.iter().collect();
        queue!(w, cursor::MoveTo(rect.x, rect.y + yy), style::Print(&s))?;
    }

    if let Some(d) = dt {
        let since = if let Ok(elapsed) = d.elapsed() { elapsed } else { Duration::from_secs(0) };
        let st = format!("Exited {} ago", format_duration(since));
        let wd = st.width();
        let diff = rect.w as usize - wd; // TODO: can st be longer than width?
        let pad = " ".repeat(diff);
        queue!(w, cursor::MoveTo(rect.x - xshift, rect.y + rect.h), style::Print(" ".repeat(xshift as usize)))?;
        queue!(
            w,
            cursor::MoveTo(rect.x, rect.y + rect.h),
            style::SetForegroundColor(Color::Red),
            style::Print(&st),
            style::ResetColor,
            style::Print(pad)
        )?;
    } else {
        let st = rect.h as usize * rect.w as usize;
        let end = st + rect.w as usize;
        let slice = &cnt.screen[st..end];
        queue!(w, cursor::MoveTo(rect.x - xshift, rect.y + rect.h), style::Print(" ".repeat(xshift as usize)))?;
        queue!(w, cursor::MoveTo(rect.x, rect.y + rect.h))?;
        for c in slice.iter() {
            if *c == '+' {
                queue!(w, style::SetForegroundColor(Color::Red), style::Print(c))?;
            } else if *c == '-' {
                queue!(w, style::SetForegroundColor(Color::Green), style::Print(c))?;
            } else {
                queue!(w, style::Print(" "))?;
            }
        }
        queue!(w, style::ResetColor)?;
    }

    Ok(())
}

fn draw_cpu_head<W>(w: &mut W, rect: DrawRect, vals: DrawVal, scale_to: u64) -> Result<()>
where
    W: Write,
{
    // queue!(w, cursor::MoveTo(0, y), style::Print("CPU\u{2502}"))?;
    let sc = if scale_to > 9999 { "!!!!\u{2502}".to_string() } else { format!("{:4}\u{2502}", scale_to) };
    queue!(w, cursor::MoveTo(0, rect.y), style::Print(&sc))?;
    if vals.max != 0 {
        let s = if vals.max > 9999 { ">10K\u{2502}".to_string() } else { format!("{:4}\u{2502}", vals.max) };
        queue!(w, cursor::MoveTo(0, rect.y + 3), style::Print(s))?;
    } else {
        queue!(w, cursor::MoveTo(0, rect.y + 3), style::Print("    \u{2502}"))?;
    }
    queue!(w, cursor::MoveTo(0, rect.y + 2), style::Print("    \u{2502}"))?;
    let s = if vals.curr > 9999 { ">10K".to_string() } else { format!("{:4}", vals.curr) };
    queue!(
        w,
        cursor::MoveTo(0, rect.y + 1),
        style::SetForegroundColor(Color::Blue),
        style::Print(s),
        style::ResetColor,
        style::Print("\u{2502}")
    )?;
    for idx in 4..rect.h {
        queue!(w, cursor::MoveTo(0, rect.y + idx), style::Print("    \u{2502}"))?;
    }
    Ok(())
}

fn draw_mem_head<W>(w: &mut W, rect: DrawRect, vals: DrawVal, diff: i64, gmin: u64, gmax: u64) -> Result<()>
where
    W: Write,
{
    {
        // queue!(w, cursor::MoveTo(x, y), style::Print(" MEM \u{2502}"))?;
        let gmax_val = format_mem(gmax);
        let gmax_str = format!("{:>5}|", gmax_val);
        queue!(w, cursor::MoveTo(rect.x, rect.y), style::Print(gmax_str))?;
    }
    {
        let gmin_val = format_mem(gmin);
        let gmin_str = format!("{:>5}|", gmin_val);
        queue!(w, cursor::MoveTo(rect.x, rect.y + rect.h - 1), style::Print(gmin_str))?;
    }
    if vals.max != 0 {
        let max_val = format_mem(vals.max);
        let max_str = format!("{:>5}|", max_val);
        queue!(w, cursor::MoveTo(rect.x, rect.y + 3), style::Print(max_str))?;
    } else {
        queue!(w, cursor::MoveTo(rect.x, rect.y + 3), style::Print("     \u{2502}"))?;
    }
    if diff == 0 {
        queue!(w, cursor::MoveTo(rect.x, rect.y + 2), style::Print("  -  \u{2502}"))?;
    } else {
        let diff_val = format_diff(diff);
        let diff_str = format!("{:>5}|", diff_val);
        queue!(w, cursor::MoveTo(rect.x, rect.y + 2), style::Print(diff_str))?;
    }
    let curr_val = format_mem(vals.curr);
    let curr_str = format!("{:>5}", curr_val);
    queue!(
        w,
        cursor::MoveTo(rect.x, rect.y + 1),
        style::SetForegroundColor(Color::Blue),
        style::Print(curr_str),
        style::ResetColor,
        style::Print("\u{2502}")
    )?;
    for idx in 4..rect.h - 1 {
        queue!(w, cursor::MoveTo(rect.x, rect.y + idx), style::Print("     \u{2502}"))?;
    }
    Ok(())
}

fn draw_title<W>(w: &mut W, proc: &Process, cnt: usize) -> Result<()>
where
    W: Write,
{
    let y = proc.y;
    // TODO: PID in read for dead processes ?
    let pid = format!("[{}]-[{}] ", cnt, proc.pid);
    let maxw = proc.w as usize - pid.len();
    let cmd = fade_str_left(&proc.cmd, maxw);
    let spare = maxw - cmd.width();
    let title = if spare == 0 {
        format!("{}{}", pid, cmd)
    } else {
        let left = spare / 2;
        format!("{}{}{}{}", "-".repeat(left), pid, cmd, "-".repeat(spare - left))
    };
    queue!(w, cursor::MoveTo(0, y), style::Print(title))?;

    let y = y + 1;
    let mut title = if proc.w < 40 {
        format!(
            "R: {}({}) W: {}({})",
            format_bytes(proc.io_r_total),
            format_bytes(proc.io_r_delta),
            format_bytes(proc.io_w_total),
            format_bytes(proc.io_w_delta),
        )
    } else {
        format!(
            "IO: Read {}({}), Write {}({})",
            format_bytes(proc.io_r_total),
            format_bytes(proc.io_r_delta),
            format_bytes(proc.io_w_total),
            format_bytes(proc.io_w_delta),
        )
    };
    if title.width() < maxw {
        title += &" ".repeat(maxw - title.width());
    }
    queue!(w, cursor::MoveTo(0, y), style::Print(title))?;

    Ok(())
}

// fn draw_footer<W>(w: &mut W, x: u16, y: u16, width: u16) -> Result<()>
// where W: Write,
// {
//     queue!(w, cursor::MoveTo(x, y+MIN_HEIGHT+1), style::Print("=".repeat(width as usize)))
// }

// U+2502 - vertical line
pub fn draw_counter<W>(w: &mut W, proc: &mut Process, cnt: usize, conf: &Config) -> Result<()>
where
    W: Write,
{
    if proc.w == 0 {
        // hidden - skip it
        return Ok(());
    }

    draw_title(w, &proc, cnt)?;
    // draw_footer(w, 0, proc.y, proc.w)?; // TODO

    let (cpu_w, mem_w) = if proc.sided {
        let cw = (proc.w - 2) / 2 - 4;
        let mw = proc.w - cw;
        (cw, mw)
    } else {
        (proc.w, proc.w)
    };

    let dx = if proc.sided { cpu_w } else { 0 };
    let (hc, hm, dym, yshift) = if proc.sided {
        (proc.h, proc.h, 0, 2)
    } else {
        let hh = proc.h / 2;
        (hh, proc.h - hh, hh, 0)
    };

    proc.mem.calculate_range();

    // U+2502 - vertical line
    let head_cpu_rect = DrawRect { y: proc.y + 2, h: hc - 3, ..Default::default() };
    let head_cpu_val = DrawVal { curr: proc.cpu.last(), max: proc.cpu.max };
    draw_cpu_head(w, head_cpu_rect, head_cpu_val, proc.cpu.scale_to)?;
    let diff = proc.mem.last_diff();

    let (max_val, min_val) = if proc.mem.auto_scale {
        if conf.scale_max {
            (proc.mem.max, 0)
        } else {
            (proc.mem.gmax, proc.mem.gmin)
        }
    } else {
        (proc.mem.max, 0)
    };

    let mem_head_rect = DrawRect { x: dx, y: proc.y + dym + yshift, w: 0, h: hm - yshift - 1 };
    let mem_head_val = DrawVal { curr: proc.mem.last(), max: proc.mem.max };
    draw_mem_head(w, mem_head_rect, mem_head_val, diff, min_val, max_val)?;

    proc.cpu.update(cpu_w - 5, hc - 3, conf);
    proc.mem.update(mem_w - 6, hm - yshift - 1, conf);

    // TODO: do not subtract xshift here, pass XSTART & XSHIFT to draw_spikes
    let cpu_rect = DrawRect { x: 5, y: proc.y + 2, w: cpu_w - 5, h: hc - 3 };
    // draw_spikes(w, &proc.cpu, 5, proc.y + 2, cpu_w - 5, hc - 3, 5, proc.dead_since)?;
    draw_spikes(w, &proc.cpu, cpu_rect, 5, proc.dead_since)?;
    let mem_rect = DrawRect { x: dx + 6, y: proc.y + dym + yshift, w: mem_w - 6, h: hm - yshift - 1 };
    // draw_spikes(w, &proc.mem, dx + 6, proc.y + dym + yshift, mem_w - 6, hm - yshift - 1, 6, None)?;
    draw_spikes(w, &proc.mem, mem_rect, 6, None)?;

    Ok(())
}

#[cfg(test)]
mod var_test {
    use super::*;

    #[test]
    fn idx_low() {
        let cfg = Config { detail: Detail::Low, ..Config::default() };
        let vals: [char; 8] = [' ', ' ', ' ', ' ', '\u{2588}', '\u{2588}', '\u{2588}', '\u{2588}'];
        for idx in 0u64..8u64 {
            let v = (idx as f64) / 8.0f64;
            let c = char_for_value(v, &cfg);
            let r = vals[idx as usize];
            assert_eq!(r, c);
        }
    }

    #[test]
    fn idx_med() {
        let cfg = Config { detail: Detail::Medium, ..Config::default() };
        let vals: [char; 8] = [' ', ' ', ' ', '\u{2584}', '\u{2584}', '\u{2584}', '\u{2588}', '\u{2588}'];
        for idx in 0u64..8u64 {
            let v = (idx as f64) / 8.0f64;
            let c = char_for_value(v, &cfg);
            let r = vals[idx as usize];
            assert_eq!(r, c);
        }
    }

    #[test]
    fn idx_high() {
        let cfg = Config { detail: Detail::High, ..Config::default() };
        let vals: [char; 8] = [' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}'];
        for idx in 0u64..8u64 {
            let v = (idx as f64) / 8.0f64;
            let c = char_for_value(v, &cfg);
            let r = vals[idx as usize];
            assert_eq!(r, c);
        }

        let vals: [char; 14] = [
            ' ', ' ', '\u{2581}', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2585}',
            '\u{2586}', '\u{2587}', '\u{2587}', '\u{2588}',
        ];
        for idx in 0u64..14 {
            let v = (idx as f64) / 14.0f64;
            let c = char_for_value(v, &cfg);
            let r = vals[idx as usize];
            assert_eq!(r, c);
        }
    }
}
