#![cfg(unix)]
use inotify::{Inotify, WatchMask};
use walkdir::{DirEntry, WalkDir};

use std::env;
use std::io::{self, Write};
use std::process::Command;

const CLEAR: &str = "\x1B[2J\x1B[1;1H";

#[derive(Debug)]
struct Runner {
    inotify: Inotify,
    command: String,
}

impl Runner {
    pub fn new(mode: Mode, command: Option<String>, directories: Option<Vec<String>>) -> Self {
        let inotify = Inotify::init().expect("Error initializing inotify");

        let (command, directories) = match mode {
            Mode::Rust => {
                let command = command.unwrap_or("cargo fmt; cargo clippy --color always -q".into());
                let directories = directories.unwrap_or(vec!["src".into()]);
                (command, directories)
            }
            Mode::Make => {
                let command = command.unwrap_or("make".into());
                let directories = directories.unwrap_or(vec![".".into()]);
                (command, directories)
            }
            Mode::Custom => {
                let command = command.expect("Command needs to be present for custom mode");
                let directories = directories.unwrap_or(vec![".".into()]);
                (command, directories)
            }
        };

        for directory in directories {
            println!("Watching {}", directory);

            inotify
                .watches()
                .add(directory, WatchMask::MODIFY)
                .expect("Failed to add file watch");
        }

        Self { inotify, command }
    }

    pub fn run(&mut self) -> ! {
        println!("{}", self.command.clone());
        loop {
            // Read events that were added with `Watches::add` above.
            let mut buffer = [0; 1024];
            let events = self
                .inotify
                .read_events_blocking(&mut buffer)
                .expect("Error while reading events");
            for _event in events {
                println!("{}", CLEAR);
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(self.command.clone())
                    .output();
                if let Ok(output) = output {
                    io::stdout().write_all(&output.stdout).unwrap();
                    io::stderr().write_all(&output.stderr).unwrap();
                }
            }
            let _ = self.inotify.read_events_blocking(&mut buffer);
        }
    }
}

fn main() {
    let args = env::args().collect::<Vec<_>>();
    let mut runner = if args.len() <= 1 {
        // gussing either rust or make usage.
        let mode = guess_mode_by_current_directory();
        Runner::new(mode, None, None)
    } else if args.len() == 2 {
        // provided the custom command, using default directory.
        let mut args = args.into_iter().rev();
        let _this_executable = args.next();
        let command = args.next();
        Runner::new(Mode::Custom, command, None)
    } else {
        // custom command with custom paths.
        let mut args = args.into_iter().rev();
        let _this_executable = args.next();
        let command = args.next();
        let directories: Vec<String> = args.collect();
        Runner::new(Mode::Custom, command, Some(directories))
    };
    runner.run();
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

fn guess_mode_by_current_directory() -> Mode {
    let mut cargo_toml_found = false;
    let mut makefile_found = false;
    for entry in WalkDir::new(".")
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if let Some(filename) = entry.file_name().to_str() {
            match filename {
                "Cargo.toml" => cargo_toml_found = true,
                "Makefile" => makefile_found = true,
                _ => (),
            }
        }
    }

    if cargo_toml_found {
        Mode::Rust
    } else if makefile_found {
        Mode::Make
    } else {
        Mode::Custom
    }
}

#[derive(Debug)]
enum Mode {
    Rust,
    Make,
    Custom,
}
