use std::{
    process::{Command, Stdio},
    time::SystemTime,
};

use clap::Parser;
use rayon::prelude::*;

#[derive(Parser)]
struct Cli {
    #[arg(short = 'A')]
    attr: Vec<String>,
    file: String,
}

fn main() {
    let args = Cli::parse();

    let drvs = args
        .attr
        .into_par_iter()
        .map(|a| {
            let start = SystemTime::now();

            let res = Command::new("fluke")
                .arg(args.file.clone())
                .arg(&a)
                .stderr(Stdio::inherit())
                .output()
                .unwrap();

            let elapsed = start.elapsed().unwrap().as_secs();

            if elapsed > 0 {
                eprintln!("{a} in {} took {elapsed}s", args.file.clone());
            }

            assert!(res.status.success());

            String::from_utf8(res.stdout).unwrap().trim().to_string()
        })
        .collect::<Vec<_>>();

    assert!(
        Command::new("nix-build")
            .arg("--no-out-link")
            .args(drvs)
            .status()
            .unwrap()
            .success()
    );
}
