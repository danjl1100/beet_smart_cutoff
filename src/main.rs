use anyhow::Context as _;
use clap::Parser;
use std::{
    io::{stdin, BufRead, Write as _},
    num::NonZeroUsize,
    str::FromStr,
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
    let Args {
        beet_command,
        current_args,
        timeless_args,
    } = Args::parse();

    let beets = BeetCommand(beet_command);

    let entries = beets
        .query_current_entries(current_args)
        .context("query current items")?;

    let date_entry = select_end(&entries)?;

    if let Some(date_entry) = date_entry {
        println!("Chose {date_entry:?}");
    }

    Ok(())
}

fn select_end(entries: &[DateEntry]) -> anyhow::Result<Option<&DateEntry>> {
    const TARGET_COUNTS: &[usize] = &[30, 50, 70];

    let mut target_counts = TARGET_COUNTS.to_vec();
    loop {
        let mut prev_index = None;
        let mut choice_index = 1;
        let transitions: Vec<_> = target_counts
            .iter()
            .cloned()
            .filter_map(|target_count| {
                if prev_index.is_some_and(|prev_index| prev_index >= target_count) {
                    println!("[skipping target: {target_count}]");
                    None
                } else {
                    let transition = find_transition(entries, target_count);
                    if let Some(transition) = transition {
                        println!("[#{choice_index}] Breakpoint for {target_count}:");
                        choice_index += 1;

                        println!("{transition}");

                        prev_index = Some(transition.index);
                        Some(transition)
                    } else {
                        println!("[out of range: {target_count}]");
                        None
                    }
                }
            })
            .collect();

        match prompt_user_selection(&transitions)? {
            Some(UserSelection::NewCounts(new_counts)) => {
                target_counts = new_counts;
            }
            Some(UserSelection::Entry(entry)) => return Ok(Some(entry)),
            None => return Ok(None),
        }
    }
}

enum UserSelection<'a> {
    Entry(&'a DateEntry),
    NewCounts(Vec<usize>),
}
fn prompt_user_selection<'a>(
    transitions: &[Transition<'a>],
) -> anyhow::Result<Option<UserSelection<'a>>> {
    let mut prompt = Prompt::default();
    loop {
        let input = prompt.read_line(Command::PROMPT)?;

        match Command::from_str(input)? {
            Command::Quit => return Ok(None),
            Command::Custom => {
                let target_str =
                    prompt.read_line("Enter custom target numbers (space separated):")?;
                match target_str
                    .split_whitespace()
                    .map(|token| token.parse())
                    .collect()
                {
                    Ok(new_counts) => {
                        return Ok(Some(UserSelection::NewCounts(new_counts)));
                    }
                    Err(err) => {
                        println!("invalid number {target_str:?}: {err}");
                    }
                }
            }
            Command::Number(number) => {
                let index = number.get() - 1;
                if let Some(Transition { included, .. }) = transitions.get(index) {
                    return Ok(Some(UserSelection::Entry(included)));
                } else {
                    println!("invalid number {number}");
                }
            }
            Command::Empty => {}
        }
    }
}

enum Command {
    Quit,
    Custom,
    Number(NonZeroUsize),
    Empty,
}
impl Command {
    const PROMPT: &'static str = "Enter selection [#/Custom/Quit]:";
}
impl FromStr for Command {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let result = match s.to_lowercase().as_str() {
            "q" | "quit" | "exit" => Self::Quit,
            "c" | "custom" => Self::Custom,
            "" => Self::Empty,
            input => {
                if let Ok(number) = input.parse() {
                    Self::Number(number)
                } else {
                    anyhow::bail!("unrecognized command {input:?}")
                }
            }
        };
        Ok(result)
    }
}

#[derive(Default)]
struct Prompt {
    buffer: String,
}
impl Prompt {
    fn read_line(&mut self, prompt: &str) -> anyhow::Result<&str> {
        print!("\n{prompt} ");
        let _ = std::io::stdout().flush();

        self.buffer.clear();
        stdin().read_line(&mut self.buffer)?;
        Ok(self.buffer.trim())
    }
}

#[derive(Clone, Copy, Debug)]
struct Transition<'a> {
    index: usize,
    included: &'a DateEntry,
    excluded: &'a DateEntry,
}
fn find_transition(items: &[DateEntry], target_count: usize) -> Option<Transition<'_>> {
    items
        .windows(2)
        .enumerate()
        .skip(target_count)
        .find_map(|(index, window)| {
            let [first, second] = window else {
                panic!("windows(2) not yielding two")
            };
            if first.date != second.date {
                Some(Transition {
                    index,
                    included: first,
                    excluded: second,
                })
            } else {
                None
            }
        })
}
impl std::fmt::Display for Transition<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Transition {
            index,
            included,
            excluded,
        } = self;
        writeln!(f, "    {}: {} {}", index, included.date, included.entry)?;
        write!(f, "    {}: {} {}", index + 1, excluded.date, excluded.entry)
    }
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
impl CheckErrors for Result<std::process::Output, std::io::Error> {
    fn stdout_check_errors(self) -> anyhow::Result<Vec<u8>> {
        let std::process::Output {
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
        std::process::Command::new(&self.0)
    }

    fn query_current_entries(&self, current_args: String) -> anyhow::Result<Vec<DateEntry>> {
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

#[derive(Debug)]
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
