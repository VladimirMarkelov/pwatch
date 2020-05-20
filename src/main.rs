mod config;
mod counter;
mod layout;
mod ux;

use std::fs::File;
use std::io::{stdout, Write};
use std::process::exit;
use std::time::{Duration, Instant};

// use log::*;
use simplelog::*;

use crossterm::event::{poll, read /*, DisableMouseCapture, EnableMouseCapture*/, Event, KeyCode};
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, ClearType};
use crossterm::{/*execute,*/ queue, style, Result};

// use sysinfo::{ProcessExt, System, SystemExt};

fn process_events(lay: &mut layout::Layout) -> Result<()> {
    // let mut s = System::new_all();
    let mut tm = Instant::now();
    let mut force_redraw = false;
    let mut resized = false;

    // draw immediately
    lay.update();
    lay.place();
    lay.draw_counters()?;
    let mut prev_h = lay.counter_height();

    loop {
        let (tot, hid, _dead) = lay.proc_totals();
        let page = (tot - hid) as i32;
        if poll(Duration::from_millis(lay.config.freq))? {
            match read()? {
                Event::Key(ev) => {
                    match ev.code {
                        KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                        // KeyCode::Down | KeyCode::Up => {
                        //     lay.scroll(ev.code == KeyCode::Down);
                        //     force_redraw = true;
                        // },
                        KeyCode::Down => force_redraw = lay.scroll(1),
                        KeyCode::Up => force_redraw = lay.scroll(-1),
                        KeyCode::Home => force_redraw = lay.scroll(layout::SCROLL_HOME),
                        KeyCode::End => force_redraw = lay.scroll(layout::SCROLL_END),
                        KeyCode::PageDown => force_redraw = lay.scroll(page),
                        KeyCode::PageUp => force_redraw = lay.scroll(-page),
                        KeyCode::Char(' ') => {
                            lay.toggle_mark();
                            force_redraw = true;
                        }
                        KeyCode::Char('r') => {
                            lay.reset_max();
                            force_redraw = true;
                        }
                        KeyCode::F(12) => {
                            lay.config.scale_max = !lay.config.scale_max;
                            force_redraw = true;
                        }
                        _ => {}
                    }
                }
                Event::Resize(width, height) => {
                    if width < 30 || height < 10 {
                        eprintln!("Requires terminal width at least 30 and height at least 10 characters");
                        exit(1);
                    }
                    lay.size_changed(width, height);
                    force_redraw = true;
                    resized = true;
                }
                // Event::Mouse(ev) => ,
                _ => {}
            }
        }
        let must_update = tm.elapsed() >= Duration::from_millis(lay.config.freq);
        if !force_redraw && !must_update {
            continue;
        }
        force_redraw = false;

        //print!(".");
        // s.refresh_processes();
        // println!("------");
        // for process in s.get_process_by_name("pwatch") {
        //     println!("{}: {} - {}", process.pid(), process.memory(), process.cpu_usage());
        // }
        // for (p, pp) in s.get_processes() {
        //     if pp.cpu_usage() > 0.1 {
        //         println!("[{}]{}: {} - {}", p, pp.name(), pp.memory()/1024, pp.cpu_usage());
        //     }
        // }
        //

        if must_update {
            lay.update();
        }
        // lay.cleanup();
        lay.place();

        let new_h = lay.counter_height();
        if resized || (new_h != 0 && new_h != prev_h) {
            prev_h = new_h;
            resized = false;
            let mut stdout = stdout();
            queue!(stdout, style::ResetColor, terminal::Clear(ClearType::All))?;
            stdout.flush()?;
        }

        // let mut stdout = stdout();
        // layout::draw_totals(&mut stdout, &lay)?;
        // for (idx, proc) in lay.procs.iter_mut().enumerate() {
        //     if proc.w == 0 {
        //         break;
        //     }
        //     counter::draw_counter(&mut stdout, proc, idx+1, &lay.config)?;
        // }
        // stdout.flush()?;
        lay.draw_counters()?;
        if must_update {
            tm = Instant::now();
        }
    }
}

fn main() -> Result<()> {
    let cb = ConfigBuilder::new().set_time_format("[%Y-%m-%d %H:%M:%S%.3f]".to_string()).build();
    CombinedLogger::init(vec![WriteLogger::new(LevelFilter::Info, cb, File::create("app.log").unwrap())]).unwrap();
    println!();
    enable_raw_mode()?;
    {
        let mut stdout = stdout();
        queue!(stdout, style::ResetColor, terminal::Clear(ClearType::All))?;
        stdout.flush()?;
    }
    // let mut stdout = stdout();
    // let config = Default::default();
    let config = config::parse_args();
    let mut lay = layout::Layout::new(config);

    // execute!(stdout, EnableMouseCapture)?;
    if let Err(e) = process_events(&mut lay) {
        eprintln!("{:?}", e);
    }
    // execute!(stdout, DisableMouseCapture)?;

    disable_raw_mode()?;
    Ok(())
}
