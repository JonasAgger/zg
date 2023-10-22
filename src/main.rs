mod matcher;
mod workload;

use anyhow::Result;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;

use crate::matcher::{ContainsMatcher, MatchEngine, RegexMatcher};
use crate::workload::{FileWorkload, Workload};
use clap::Parser;

static VERBOSE: AtomicBool = AtomicBool::new(false);

#[derive(Parser, Debug)]
struct ZgOptions {
    /// Source path to search for files
    source_path: String,

    /// Pattern to search for in files
    pattern: String,

    /// glob pattern to search for
    #[arg(short, long)]
    glob: Option<String>,

    /// number of files to search, ordered by modification date
    #[arg(short, long)]
    take: Option<usize>,

    /// output file
    #[arg(short, long)]
    output: Option<String>,

    /// Use Regex
    #[arg(short, long)]
    regex: bool,

    /// Verbose Logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let options = ZgOptions::parse();
    VERBOSE.store(options.verbose, Ordering::Release);

    if VERBOSE.load(Ordering::Relaxed) {
        println!("Starting...");
        println!("Options: {:?}", options);
    }

    let output_file = match options.output.as_ref() {
        Some(path) => Some(OpenOptions::new().create(true).write(true).open(path)?),
        None => None,
    };

    if VERBOSE.load(Ordering::Relaxed) {
        println!("Generating workloads:");
    }

    let result = match options.regex {
        true => {
            let matcher = RegexMatcher::new(&options.pattern);
            let workloads = Workload::generate_workloads(
                options.source_path.as_str(),
                options.glob,
                options.take,
                matcher,
            )?;
            process_workloads(workloads, output_file)
        }
        false => {
            let matcher = ContainsMatcher::new(&options.pattern);
            let workloads = Workload::generate_workloads(
                options.source_path.as_str(),
                options.glob,
                options.take,
                matcher,
            )?;
            process_workloads(workloads, output_file)
        }
    };

    result
}

fn process_workloads<Matcher: MatchEngine + 'static>(
    workloads: Vec<FileWorkload<Matcher>>,
    output_file: Option<File>,
) -> Result<()> {
    if VERBOSE.load(Ordering::Relaxed) {
        println!("Starting to parse...");
    }
    use rayon::prelude::*;

    workloads
        .par_iter()
        .for_each(|wl| {
            let mut owned = wl.lock().unwrap();
            if let Err(e) = owned.run() {
                eprintln!("Error running! {}", e);
            }
        });
    // // Startup threads to process
    // let workload_threads: Vec<JoinHandle<()>> = workloads
    //     .iter()
    //     .map(|wl| {
    //         let data = wl.clone();
    //         std::thread::spawn(move || {
    //             let mut owned = data.lock().unwrap();
    //             if let Err(e) = owned.run() {
    //                 eprintln!("Error running! {}", e);
    //             }
    //         })
    //     })
    //     .collect();

    // // Wait for threads to finish
    // let _: Vec<_> = workload_threads.into_iter().map(|jh| jh.join()).collect();

    if VERBOSE.load(Ordering::Relaxed) {
        println!("Done!");
    }
    // produce output!

    if let Some(mut output_file) = output_file {
        for wl in workloads {
            let owned = wl.lock().unwrap();

            if !owned.any() {
                continue;
            }

            writeln!(output_file, "{}", owned)?;
            for line in owned.iter() {
                writeln!(output_file, "{}", line)?;
            }
        }

        output_file.flush()?;
    } else {
        for wl in workloads.iter().rev() {
            let owned = wl.lock().unwrap();

            if !owned.any() {
                continue;
            }

            println!("\u{1b}[31m{}\u{1b}[39m", owned);
            for line in owned.iter() {
                println!("{}", line);
            }
        }
    }

    Ok(())
}
