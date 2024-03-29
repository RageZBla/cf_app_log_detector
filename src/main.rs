use clap::{crate_version, value_t, App, Arg};
use std::fs;
use std::io;
use std::io::prelude::*;

use cf_app_log_detector::parse_cf_app_log;

fn main() {
    let matches = App::new("cf-app-log-detector")
       .version(crate_version!())
       .author("Olivier Lechevalier <olivier.lechevalier@gmail.com>")
       .about("Try to detect log outputted by CF cli")
       .arg(Arg::with_name("percentage_matching")
          .short("p")
          .long("percentage-matching")
          .value_name("PERCENTAGE_MATCHING")
          .help("Percentage of line matching expected format for the file to be considered an application log")
          .takes_value(true)
          .default_value("90"))
        .arg(Arg::with_name("one_line_match")
          .value_name("ONE_LINE_MATCH")
          .long("one-line-match")
          .help("Consider the file to be CF app log if a single line matches expected format")
          .takes_value(false))
        .arg(Arg::with_name("debug")
          .value_name("DEBUG")
          .long("debug")
          .short("d")
          .help("Enable debugging")
          .takes_value(false))
        .arg(Arg::with_name("log")
          .value_name("LOG")
          .help("Log file")
          .index(1)
          .takes_value(true))
       .get_matches();

    let mut detector = CfAppLogDetector::new(
        value_t!(matches, "percentage_matching", usize).unwrap(),
        matches.is_present("one_line_match"),
    );

    let filename = matches.value_of("log").unwrap();
    match detector.process_file(filename) {
        Ok(()) => (),
        Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("File {} not found", filename);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed parsing file: {}, message: {}", filename, e);
            std::process::exit(2);
        }
    }

    std::process::exit(detector.show_results(filename, matches.is_present("debug")));
}

pub struct CfAppLogDetector {
    one_line_match: bool,
    total_log_lines: usize,
    log_lines_matching: usize,
    trigger_percentage: usize,
}

impl CfAppLogDetector {
    pub fn new(trigger_percentage: usize, one_line_match: bool) -> CfAppLogDetector {
        CfAppLogDetector {
            trigger_percentage,
            one_line_match,
            total_log_lines: 0,
            log_lines_matching: 0,
        }
    }

    pub fn process_file(&mut self, path: &str) -> io::Result<()> {
        let reader = io::BufReader::new(fs::File::open(path)?);

        for line in reader.lines() {
            match CfAppLogDetector::parse_line(&line?) {
                Ok(_log) => {
                    self.total_log_lines += 1;
                    self.log_lines_matching += 1;
                    if self.one_line_match {
                        break;
                    }
                }
                Err(_err) => {
                    // eprintln!("parsing error: {}", _err);
                    self.total_log_lines += 1;
                }
            };
        }
        Ok(())
    }

    pub fn show_results(&mut self, path: &str, debug: bool) -> i32 {
        if debug {
            println!("[DEBUG] total number of lines: {}", self.total_log_lines);
            println!("[DEBUG] log lines matching: {}", self.log_lines_matching);
        }
        let percentage_matching = if self.total_log_lines > 0 {
            (self.log_lines_matching as f64 / self.total_log_lines as f64 * 100.0).floor()
        } else {
            0.0
        };

        if debug {
            println!("[DEBUG] percentage matching: {}", percentage_matching)
        }

        let matching = percentage_matching >= self.trigger_percentage as f64
            || (self.log_lines_matching > 0 && self.one_line_match);
        if matching {
            eprintln!(
                "{} is a CF application log [{}% line matching]",
                path, percentage_matching
            );
            0
        } else {
            eprintln!(
                "{} is NOT CF application log [{}% line matching]",
                path, percentage_matching
            );
            1
        }
    }

    fn parse_line(line: &str) -> Result<bool, Box<dyn std::error::Error>> {
        // 136 |                     Err(err) => Err(Box::new(err)),
        //                  ^^^^^^^^^^^^^^^^^^ returns a value referencing data owned by the current function
        let stripped_line: String;
        match strip_ansi_escapes::strip(line) {
            Ok(stripped_vector) => {
                stripped_line = String::from_utf8(stripped_vector)?;
                match parse_cf_app_log(&stripped_line) {
                    Ok(_) => Ok(true),
                    Err(_) => Ok(false), // TODO: can't do better now
                }
            }
            Err(err) => Err(Box::new(err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use assert_cmd::Command;
    use predicates::prelude::*;

    #[test]
    fn file_doesnt_exist() {
        let mut cmd = Command::cargo_bin("cf-app-log-detector").unwrap();
        cmd.arg("test/file/doesnt/exists");
        cmd.assert().failure().stderr(predicate::str::contains(
            "File test/file/doesnt/exists not found",
        ));
    }

    #[test]
    fn matching_file() {
        let mut cmd = Command::cargo_bin("cf-app-log-detector").unwrap();
        cmd.arg("test/file/matching.txt");
        cmd.assert().success().stderr(predicate::str::contains(
            "test/file/matching.txt is a CF application log [100% line matching]",
        ));
    }

    #[test]
    fn debug() {
        let mut cmd = Command::cargo_bin("cf-app-log-detector").unwrap();
        cmd.arg("test/file/matching.txt").arg("--debug");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("[DEBUG]"));
    }

    #[test]
    fn first_line_match() {
        let mut cmd = Command::cargo_bin("cf-app-log-detector").unwrap();
        cmd.arg("test/file/matching.txt")
            .arg("--one-line-match")
            .arg("--debug");
        cmd.assert()
            .success()
            .stdout(predicate::str::contains("total number of lines: 1"));
    }
}
