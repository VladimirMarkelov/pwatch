mod config;
mod counter;
mod layout;
mod ux;

// use std::fs::File;
use std::io::{stdout, Write};
use std::process::exit;
use std::time::{Duration, Instant};

use atty::Stream;
// use simplelog::*;

use crossterm::event::{poll, read, Event, KeyCode};
use crossterm::terminal::{self, disable_raw_mode, enable_raw_mode, ClearType};
use crossterm::{queue, style, Result};

fn process_events(lay: &mut layout::Layout) -> Result<()> {
    let mut tm = Instant::now();
    let mut force_redraw = false;
    let mut resized = false;

    // draw immediately empty graphs
    lay.update();
    lay.place();
    lay.draw_counters()?;
    let mut prev_h = lay.counter_height();

    loop {
        let (tot, hid, _dead) = lay.proc_totals();
        let page = tot - hid;
        if poll(Duration::from_millis(lay.config.freq))? {
            match read()? {
                Event::Key(ev) => match ev.code {
                    KeyCode::Esc | KeyCode::Char('q') => return Ok(()),
                    KeyCode::Down => force_redraw = lay.scroll(layout::Scroll::Down(1)),
                    KeyCode::Up => force_redraw = lay.scroll(layout::Scroll::Up(1)),
                    KeyCode::Home => force_redraw = lay.scroll(layout::Scroll::Home),
                    KeyCode::End => force_redraw = lay.scroll(layout::Scroll::End),
                    KeyCode::PageDown => force_redraw = lay.scroll(layout::Scroll::Down(page)),
                    KeyCode::PageUp => force_redraw = lay.scroll(layout::Scroll::Up(page)),
                    KeyCode::Char(' ') => {
                        lay.toggle_mark();
                        force_redraw = true;
                    }
                    KeyCode::Char('r') => {
                        lay.reset_max();
                        force_redraw = true;
                    }
                    KeyCode::F(1) => {
                        lay.switch_help();
                        force_redraw = true;
                    }
                    KeyCode::F(9) => {
                        lay.switch_title_type();
                        force_redraw = true;
                    }
                    KeyCode::F(12) => {
                        lay.config.scale_max = !lay.config.scale_max;
                        force_redraw = true;
                    }
                    _ => {}
                },
                Event::Resize(width, height) => {
                    if width < 30 || height < 10 {
                        eprintln!("Requires terminal width at least 30 and height at least 10 characters");
                        exit(1);
                    }
                    lay.size_changed(width, height);
                    force_redraw = true;
                    resized = true;
                }
                _ => {}
            }
        }
        let must_update = tm.elapsed() >= Duration::from_millis(lay.config.freq);
        if !force_redraw && !must_update {
            continue;
        }
        force_redraw = false;
        if must_update {
            lay.update();
        }
        lay.place();

        let new_h = lay.counter_height();
        if resized || (new_h != 0 && new_h != prev_h) {
            prev_h = new_h;
            resized = false;
            let mut stdout = stdout();
            queue!(stdout, style::ResetColor, terminal::Clear(ClearType::All))?;
            stdout.flush()?;
        }

        lay.draw_counters()?;
        if must_update {
            tm = Instant::now();
        }
    }
}

fn main() -> Result<()> {
    if !atty::is(Stream::Stdout) {
        eprintln!("Only TTY is supported");
        exit(2);
    }
    // let cb = ConfigBuilder::new().set_time_format("[%Y-%m-%d %H:%M:%S%.3f]".to_string()).build();
    // CombinedLogger::init(vec![WriteLogger::new(LevelFilter::Info, cb, File::create("app.log").unwrap())]).unwrap();
    let config = config::parse_args();
    println!();
    enable_raw_mode()?;
    {
        let mut stdout = stdout();
        queue!(stdout, style::ResetColor, terminal::Clear(ClearType::All))?;
        stdout.flush()?;
    }
    let mut lay = layout::Layout::new(config);

    if let Err(e) = process_events(&mut lay) {
        eprintln!("{:?}", e);
    }

    disable_raw_mode()?;
    Ok(())
}
