use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use notify::{recommended_watcher, Error, Event, RecursiveMode, Watcher};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, sleep};
use std::time::Duration;
use std::{
    env::args,
    process::{exit, Command},
};

static OK_COLOR: &str = "\x1b[32;1;4m";
static DEF_COLOR: &str = "\x1b[0m";
static ERR_COLOR: &str = "\x1b[31;1;4m";

fn main() {
    let input = &args().collect::<Vec<String>>()[1..];
    let split_pos = input.iter().position(|i| i == "--");
    let cmd_pos = match split_pos {
        Some(pos) => pos + 1,
        None => 0,
    };

    if cmd_pos >= input.len() {
        println!(
            "
Usage: 
    ./rerun <cmd>

With args:
    ./rerun <cmd> [arg1] [arg2] [...]

With deps watch:
    ./rerun <file1> <file2> <...> -- <cmd> [arg1] [arg2] [...]
"
        );

        exit(1)
    } else {
        let running = Arc::new(AtomicBool::new(true));
        let running_ref = running.clone();
        ctrlc::set_handler(move || {
            running_ref.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");

        while running.load(Ordering::SeqCst) {
            rerun(
                if cmd_pos > 0 {
                    &input[..cmd_pos - 1]
                } else {
                    &[]
                },
                &input[cmd_pos],
                &input[cmd_pos + 1..],
                running.clone(),
            );
        }
    }
}

fn rerun(deps: &[String], cmd: &String, args: &[String], running: Arc<AtomicBool>) {
    let env_same = Arc::new(AtomicBool::new(true));
    let env_same_ref = env_same.clone();
    let _watcher = match recommended_watcher(move |res: Result<Event, Error>| match res {
        Ok(event) => {
            println!("{}rerun: changed {:?}{}", OK_COLOR, event.paths, DEF_COLOR);
            env_same_ref.store(false, Ordering::SeqCst);
        }
        Err(e) => eprintln!("{}watch error: {:?}{}", ERR_COLOR, e, ERR_COLOR),
    }) {
        Ok(mut watcher) => {
            let mut watch_fn =
                |next: &String| match watcher.watch(Path::new(next), RecursiveMode::Recursive) {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!(
                            "{}rerun: can't watch for {}, cause {:?}, skipping{}",
                            ERR_COLOR, next, e, DEF_COLOR
                        )
                    }
                };
            for next in deps {
                watch_fn(&next)
            }
            watch_fn(cmd);
            Some(watcher)
        }
        Err(e) => {
            eprintln!(
                "{}rerun: can't create watcher, cause {:?}, skipping deps...{}",
                ERR_COLOR, e, DEF_COLOR
            );
            None
        }
    };

    println!(
        "{}rerun: \"{}\", deps={:?} + {}, args={:?}{}",
        OK_COLOR, cmd, deps, cmd, args, DEF_COLOR
    );

    match Command::new(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .args(args)
        .spawn()
    {
        Ok(mut child) => match (child.stdout.take(), child.stderr.take()) {
            (Some(stdout), Some(stderr)) => {
                let stdout_thread = thread::spawn(move || {
                    let stdout_lines = BufReader::new(stdout).lines();
                    for line in stdout_lines {
                        if let Ok(line) = line {
                            println!("{}", line);
                        }
                    }
                });

                let stderr_thread = thread::spawn(move || {
                    let stderr_lines = BufReader::new(stderr).lines();
                    for line in stderr_lines {
                        if let Ok(line) = line {
                            eprintln!("{}", line);
                        }
                    }
                });

                while running.load(Ordering::SeqCst) && env_same.load(Ordering::SeqCst) {
                    match child.try_wait() {
                        Ok(Some(code)) => {
                            println!(
                                "{}rerun: \"{}\" exited with code {}{}",
                                OK_COLOR, cmd, code, DEF_COLOR
                            );
                            break;
                        }
                        Err(e) => {
                            eprintln!(
                                "{}rerun: \"{}\" errored, cause: {:?}{}",
                                ERR_COLOR, cmd, e, DEF_COLOR
                            );
                            break;
                        }
                        Ok(_) => {
                            sleep(Duration::from_millis(10));
                        }
                    }
                }

                if !running.load(Ordering::SeqCst) || !env_same.load(Ordering::SeqCst) {
                    signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM).unwrap();
                    match child.wait() {
                        Ok(code) => {
                            println!(
                                "{}rerun: \"{}\" exited with code {}{}",
                                OK_COLOR, cmd, code, DEF_COLOR
                            );
                        }
                        Err(e) => {
                            eprintln!(
                                "{}rerun: \"{}\" errored, cause: {:?}{}",
                                ERR_COLOR, cmd, e, DEF_COLOR
                            );
                        }
                    }
                }

                let _ = stdout_thread.join();
                let _ = stderr_thread.join();
            }
            _ => {
                eprintln!(
                    "{}rerun: can't get stdout/stderr of spawned process{}",
                    ERR_COLOR, DEF_COLOR
                );
            }
        },
        Err(e) => {
            eprintln!(
                "{}rerun: can't start \"{}\", cause: {:?}{}",
                ERR_COLOR, cmd, e, DEF_COLOR
            )
        }
    }
}
