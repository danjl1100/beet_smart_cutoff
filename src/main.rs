use anyhow::Context as _;
use clap::Parser;
use std::{
    io::BufRead,
    process::{Command, Output},
};

const MAX_ENTRIES: usize = 200;

#[derive(clap::Parser)]
struct Args {
    #[clap(env)]
    beet_command: std::path::PathBuf,
    #[clap(env)]
    current_args: String,
    #[clap(env)]
    timeless_args: String,
}

fn main() -> anyhow::Result<()> {
    const TARGET_COUNTS: &[usize] = &[30, 50, 70];

    let Args {
        beet_command,
        current_args,
        timeless_args,
    } = Args::parse();

    let beets = BeetCommand(beet_command);

    let current_items = beets
        .query_current_items(current_args)
        .context("query current items")?;

    println!("current items:");
    for DateEntry { date, entry } in &current_items {
        println!("{date}|{entry}");
    }

    let mut prev_index = None;
    for &target_count in TARGET_COUNTS {
        if prev_index.is_some_and(|prev_index| prev_index >= target_count) {
            continue;
        }

        let transition = current_items
            .windows(2)
            .enumerate()
            .skip(target_count)
            .find_map(|(index, window)| {
                let [first, second] = window else {
                    panic!("windows(2) not yielding two")
                };
                if first.date != second.date {
                    Some((index, first, second))
                } else {
                    None
                }
            });
        println!("First breakpoint for {target_count}:");
        if let Some((index, included, excluded)) = transition {
            println!("    {}: {}|{}", index, included.date, included.entry);
            println!("    {}: {}|{}", index + 1, excluded.date, excluded.entry);
            prev_index = Some(index);
        }
    }

    Ok(())
}

trait CheckErrors {
    fn stdout_check_errors(self) -> anyhow::Result<Vec<u8>>;
}
impl CheckErrors for &mut std::process::Command {
    fn stdout_check_errors(self) -> anyhow::Result<Vec<u8>> {
        println!(
            "{} {:?}",
            self.get_program().to_str().unwrap_or("[non-utf8 str]"),
            &self.get_args().collect::<Vec<_>>()
        );
        self.output().stdout_check_errors()
    }
}
impl CheckErrors for Result<Output, std::io::Error> {
    fn stdout_check_errors(self) -> anyhow::Result<Vec<u8>> {
        let Output {
            status,
            stdout,
            stderr,
        } = self?;

        let stderr = std::str::from_utf8(&stderr).context("non-utf8 in beet stderr")?;
        if !stderr.is_empty() {
            anyhow::bail!("subprocess stderr: {stderr}");
        }

        if !status.success() {
            anyhow::bail!("subprocess status: {status:?}");
        }

        Ok(stdout)
    }
}

struct BeetCommand(std::path::PathBuf);
impl BeetCommand {
    fn new_command(&self) -> std::process::Command {
        Command::new(&self.0)
    }

    fn query_current_items(&self, current_args: String) -> anyhow::Result<Vec<DateEntry>> {
        let current_output = self
            .new_command()
            .arg("list")
            .args(current_args.lines())
            .arg("added-")
            .arg("--format")
            .arg("$added $artist - $album - $title")
            .stdout_check_errors()
            .context("beet ls [current_args]")?;

        current_output
            .lines()
            .enumerate()
            .take(MAX_ENTRIES)
            .map(|(number, line)| {
                DateEntry::try_from(line.with_context(|| {
                    format!("line {} from current_output beet command", number + 1)
                })?)
            })
            .collect::<anyhow::Result<Vec<_>>>()
    }
}

struct DateEntry {
    date: String,
    entry: String,
}
impl TryFrom<String> for DateEntry {
    type Error = anyhow::Error;

    fn try_from(s: String) -> anyhow::Result<Self> {
        // 01234567890123456789...
        // YYYY-MM-DD HH:MM:SS
        const DATE_LENGTH: usize = 10;
        const ENTRY_START: usize = 20;
        // NOTE: Date portion is guaranteed to be ascii
        if ENTRY_START < s.len() {
            Ok(DateEntry {
                date: s[..DATE_LENGTH].to_owned(),
                entry: s[ENTRY_START..].to_owned(),
            })
        } else {
            anyhow::bail!("entry too short: {s}")
        }
    }
}
