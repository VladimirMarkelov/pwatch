use std::fs::File;
use std::io::{Result, Write};
use std::path::Path;
use std::str;

use chrono::prelude::*;

pub(crate) struct ScreenShot {
    w: u16,
    h: u16,
    x: u16,
    y: u16,
    screen: Vec<char>,
    in_esc: bool,
    esc_seq: String,
}

impl ScreenShot {
    pub fn new(w: u16, h: u16) -> Self {
        ScreenShot { w, h, x: 0, y: 0, in_esc: false, esc_seq: String::new(), screen: vec![' '; (w * h).into()] }
    }
}

impl Write for ScreenShot {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        if buf.len() > 1 && buf[0] == 27 && buf[1] == 91 {
            self.esc_seq = String::new();
            // Check for color reset - it does not have extra args
            if buf.len() != 4 || buf[2] != 48 || buf[3] != 109 {
                self.in_esc = true;
            }
        } else if self.in_esc {
            if buf[0] == 72 {
                // GOTO
                let v: Vec<&str> = self.esc_seq.split(';').collect();
                if v.len() == 2 {
                    self.y = match v[0].parse::<u16>() {
                        Ok(n) => n - 1,
                        Err(_) => self.y,
                    };
                    self.x = match v[1].parse::<u16>() {
                        Ok(n) => n - 1,
                        Err(_) => self.x,
                    };
                }
                self.in_esc = false;
            } else if buf[0] == 109 {
                // COLOR
                self.in_esc = false;
            } else {
                let st: String = buf.iter().map(|n| char::from(*n)).collect();
                self.esc_seq += &st;
            }
        } else {
            let ustr = match str::from_utf8(buf) {
                Ok(ss) => ss,
                Err(_) => "",
            };
            for c in ustr.chars().into_iter() {
                let p: usize = self.y as usize * self.w as usize + self.x as usize;
                if p < self.w as usize * self.h as usize {
                    self.screen[p] = c;
                    self.x += 1;
                    if self.x >= self.w {
                        self.x = 0;
                        self.y += 1;
                    }
                }
            }
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> Result<()> {
        let time_now = Local::now();
        let fname = time_now.format("shot-%Y%m%d-%H%M%S.txt").to_string();
        let mut f = File::create(Path::new(&fname))?;
        let cr = b"\n";
        for y in 0..self.h {
            let w = self.w as usize;
            let p = y as usize * w;
            let st: String = self.screen[p..p + w].iter().collect();
            f.write_all(st.as_bytes())?;
            f.write_all(cr)?;
        }
        Ok(())
    }
}
