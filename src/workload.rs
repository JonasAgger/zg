use crate::matcher::MatchEngine;
use crate::VERBOSE;
use anyhow::{Context, Result};
use glob;
use std::fmt::{Display, Formatter};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub(crate) type FileWorkload<Matcher> = Arc<Mutex<Workload<Matcher>>>;

#[derive(Debug)]
pub(crate) struct Workload<Matcher: MatchEngine> {
    file: PathBuf,
    file_type: FileType,
    last_modified: u64,
    matcher: Matcher,
    matches: Vec<String>,
}

#[derive(Debug)]
enum FileType {
    Txt,
    Zip,
    GZip,
}

impl<Matcher: MatchEngine> Workload<Matcher> {
    pub(crate) fn generate_workloads(
        source_path: &str,
        glob: Option<String>,
        take_count: Option<usize>,
        matcher: Matcher,
    ) -> Result<Vec<FileWorkload<Matcher>>> {

        let mut workloads = vec![];
        let glob = match glob {
            Some(glob) => glob,
            None => String::from("*"),
        };

        let file_glob = format!("{}/{}", source_path.trim_end_matches("/"), glob);

        for entry in glob::glob(file_glob.as_str()).context("Could not parse file glob")? {
            if let Ok(path) = entry {
                if VERBOSE.load(Ordering::Relaxed) {
                    println!("Adding file: '{:?}'", path.file_name().unwrap());
                }

                let file_type = match path.extension().unwrap().to_str().unwrap() {
                    "zip" => FileType::Zip,
                    "gz" => FileType::GZip,
                    _ => FileType::Txt, // Assume txt
                };

                let file_metadata = std::fs::metadata(&path)?;
                let last_modified = file_metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();

                workloads.push(Workload {
                    file: path,
                    file_type,
                    last_modified,
                    matcher: matcher.clone(),
                    matches: vec![],
                })
            }
        }

        workloads.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

        let workloads = match take_count {
            Some(take) if take < workloads.len() => {
                workloads
                    .into_iter()
                    .take(take)
                    .map(|item| Arc::new(Mutex::new(item)))
                    .collect()
            }
            _ => workloads
                .into_iter()
                .map(|item| Arc::new(Mutex::new(item)))
                .collect(),
        };

        Ok(workloads)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &String> {
        self.matches.iter()
    }

    pub(crate) fn any(&self) -> bool {
        self.matches.len() > 0
    }

    pub(crate) fn run(&mut self) -> Result<()> {
        let start_time = Instant::now();
        if VERBOSE.load(Ordering::Relaxed) {
            println!("{} is starting...!", self);
        }
        let file = std::fs::File::open(&self.file)?;

        if matches!(self.file_type, FileType::Txt) {
            let reader = BufReader::new(file);
            self.match_entries(reader);
        } else if matches!(self.file_type, FileType::Zip) {
            let mut archive = zip::ZipArchive::new(file)?;
            let inner_file = archive.by_index(0)?;
            let reader = BufReader::new(inner_file);
            self.match_entries(reader);
        }

        let duration = Instant::now().duration_since(start_time);
        if VERBOSE.load(Ordering::Relaxed) {
            println!("scanning of {} took {:?}", self, duration);
        }
        Ok(())
    }

    fn match_entries<R: BufRead>(&mut self, reader: R) {
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => break,
            };

            if self.matcher.match_line(line.as_str()) {
                self.matches.push(line);
            }
        }
    }
}

impl<M: MatchEngine> Display for Workload<M> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} -- {:?}", self.file, self.file_type)
    }
}
