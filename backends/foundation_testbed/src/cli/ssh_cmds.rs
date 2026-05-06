//! SSH-based CLI commands (exec, shell, run, screenshot, logs, push, pull).
//! These work across providers since they only need SSH access.

use std::path::Path;

use clap::ArgMatches;
use crate::config::{get_profile};
use crate::ssh;
use crate::runner;
use crate::runner::logs;

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

fn resolve_profile(name: &str) -> std::result::Result<crate::config::VmProfile, BoxedError> {
    get_profile(name).map(|p| p.clone()).map_err(|e| Box::new(e) as BoxedError)
}

pub fn cmd_exec(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let cmd = args.get_one::<String>("cmd").unwrap();
    let method = args.get_one::<String>("method").map(|s| s.as_str()).unwrap_or("ssh");
    let profile = resolve_profile(name)?;

    match method {
        "winrm" => {
            let port = profile.winrm_port.ok_or_else(|| {
                format!("No WinRM port configured for '{}'", profile.name)
            })?;
            let winrm = crate::winrm::WinRM::new("127.0.0.1", port, profile.user, profile.pass);
            let result = winrm.run_ps(cmd)?;
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprintln!("{}", result.stderr);
            }
            if result.exit_code != 0 {
                return Err(format!("Command exited with code {}", result.exit_code).into());
            }
        }
        "ssh" => {
            let mut session = ssh::connect(&profile)?;
            let output = ssh::exec(&mut session, cmd)?;
            print!("{}", output);
        }
        _ => unreachable!(),
    }
    Ok(())
}

pub fn cmd_shell(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    ssh::shell(&profile)?;
    Ok(())
}

pub fn cmd_run(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let bin = args.get_one::<String>("bin").map(|s| s.as_str());
    let profile = resolve_profile(name)?;

    let mut session = ssh::connect(&profile)?;
    let result = runner::run_in_vm(&profile, &mut session, bin)?;
    println!("Launched binary, log at: {}", result.log_path);
    Ok(())
}

pub fn cmd_screenshot(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let out = args.get_one::<String>("out").unwrap();
    let profile = resolve_profile(name)?;

    let mut session = ssh::connect(&profile)?;
    runner::screenshot::capture(&profile, &mut session, Path::new(out))?;
    println!("Screenshot saved to {}", out);
    Ok(())
}

pub fn cmd_logs(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let profile = resolve_profile(name)?;

    let kind_str = args.get_one::<String>("kind").map(|s| s.as_str()).unwrap_or("build");
    let kind = kind_str.parse::<logs::LogKind>()
        .map_err(|e| format!("Invalid log kind: {}", e))?;

    let mode = if args.get_flag("follow") {
        logs::LogMode::Follow
    } else if args.get_flag("errors") {
        logs::LogMode::Errors
    } else if let Some(n) = args.get_one::<u32>("tail") {
        logs::LogMode::Tail(*n)
    } else {
        logs::LogMode::Dump
    };

    let opts = logs::LogOpts { kind, mode };

    let mut session = ssh::connect(&profile)?;
    let output = logs::get_logs(&profile, &mut session, &opts)?;
    print!("{}", output);
    Ok(())
}

pub fn cmd_push(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let from = args.get_one::<String>("from").unwrap();
    let to = args.get_one::<String>("to").unwrap();
    let profile = resolve_profile(name)?;

    let mut session = ssh::connect(&profile)?;
    runner::transfer::push(&mut session, Path::new(from), to)?;
    println!("Pushed {} -> {}", from, to);
    Ok(())
}

pub fn cmd_pull(args: &ArgMatches) -> std::result::Result<(), BoxedError> {
    let name = args.get_one::<String>("profile").unwrap();
    let from = args.get_one::<String>("from").unwrap();
    let to = args.get_one::<String>("to").unwrap();
    let profile = resolve_profile(name)?;

    let mut session = ssh::connect(&profile)?;
    runner::transfer::pull(&mut session, from, Path::new(to))?;
    println!("Pulled {} -> {}", from, to);
    Ok(())
}
