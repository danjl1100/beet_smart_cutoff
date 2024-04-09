use anyhow::Context as _;
use beet_command::BeetCommand;
use clap::Parser;
use prompt::Prompt;
use std::{num::NonZeroUsize, str::FromStr};

#[derive(clap::Parser)]
struct Args {
    /// Path to the `beet` command from the package `beets`
    #[clap(env, long)]
    beet_command: std::path::PathBuf,
    /// Newline separated list of filter arguments to `beet list` (excluding the date "added" filter)
    #[clap(env, long)]
    timeless_args: String,
    #[clap(long, default_value_t = 400)]
    max_entries: usize,
    /// Output JSON file
    #[clap(env, long)]
    output_file: Option<std::path::PathBuf>,
    /// Key for the output file date
    #[clap(env, long)]
    output_key: Option<String>,
}

type JsonMap = serde_json::Map<String, serde_json::Value>;

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let ParsedArgs {
        beets,
        max_entries,
        output_file_key,
    } = {
        let beets = BeetCommand::new(args.beet_command, &args.timeless_args, args.max_entries)?;
        let output_file_key = match (args.output_file, args.output_key) {
            (Some(file), Some(key)) => Some((file, key)),
            (None, None) => None,
            (Some(_), None) => anyhow::bail!("missing output_key for provided output_file"),
            (None, Some(_)) => anyhow::bail!("missing output_file for provided output_key"),
        };
        ParsedArgs {
            beets,
            max_entries: args.max_entries,
            output_file_key,
        }
    };

    let json_file_key = if let Some((output_file, output_key)) = output_file_key {
        // fail-fast if file cannot be read
        let json_file = json::read_json_file(output_file).context("reading json file")?;
        Some((json_file, output_key))
    } else {
        None
    };

    let entries = beets.query_timeless().context("query current items")?;

    let date_entry = select_end(&entries, max_entries)?;

    let Some(date_entry) = date_entry else {
        return Ok(());
    };

    let final_count = beets
        .count_entries_after(date_entry)
        .context("counting entries with chosen date bound")?;
    println!("Chose {date_entry:?}, which gives {final_count} entries");

    if let Some((json_file, key)) = json_file_key {
        let json::JsonFile { map, path } = json_file;
        let path = &path;
        let mut map = map.unwrap_or_default();

        map.insert(key, date_entry.date.clone().into());
        json::write_json_file(path, map).with_context(|| format!("writing json file {path:?}"))?;
    }

    Ok(())
}

mod json {
    use crate::JsonMap;
    use std::{
        fs::File,
        io::{BufReader, BufWriter},
        path::{Path, PathBuf},
    };

    pub struct JsonFile {
        pub map: Option<JsonMap>,
        pub path: PathBuf,
    }
    pub fn read_json_file(path: PathBuf) -> anyhow::Result<JsonFile> {
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(JsonFile { map: None, path })
            }
            Err(e) => Err(e)?,
        };
        let file = BufReader::new(file);
        let value: serde_json::Value = serde_json::from_reader(file)?;

        let serde_json::Value::Object(map) = value else {
            anyhow::bail!("unexpected JSON value: {value:?}")
        };

        let entry_count = map.len();
        let filename = path.display();
        println!("Loaded {entry_count} entries from {filename}");

        Ok(JsonFile {
            map: Some(map),
            path,
        })
    }
    pub fn write_json_file(path: impl AsRef<Path>, value: JsonMap) -> anyhow::Result<()> {
        let file = File::options()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, &value)?;

        let entry_count = value.len();
        let filename = path.as_ref().display();
        println!("Saved {entry_count} entries to {filename}");

        Ok(())
    }
}

struct ParsedArgs<'a> {
    beets: BeetCommand<'a>,
    max_entries: usize,
    output_file_key: Option<(std::path::PathBuf, String)>,
}

fn select_end(entries: &[DateEntry], max_entries: usize) -> anyhow::Result<Option<&DateEntry>> {
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

        match prompt_user_selection(&transitions, max_entries)? {
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
    max_entries: usize,
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
                    .map(|token| {
                        let number = token.parse()?;
                        if number > max_entries {
                            anyhow::bail!("{number} exceeds max_entries ({max_entries}) command-line argument")
                        } else {
                            Ok(number)
                        }
                    })
                    .collect()
                {
                    Ok(new_counts) => {
                        return Ok(Some(UserSelection::NewCounts(new_counts)));
                    }
                    Err(err) => {
                        println!("invalid custom input {target_str:?}: {err}");
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

mod prompt {
    use std::io::{stdin, Write as _};

    #[derive(Default)]
    pub struct Prompt {
        buffer: String,
    }
    impl Prompt {
        pub fn read_line(&mut self, prompt: &str) -> anyhow::Result<&str> {
            print!("\n{prompt} ");
            let _ = std::io::stdout().flush();

            self.buffer.clear();
            stdin().read_line(&mut self.buffer)?;
            Ok(self.buffer.trim())
        }
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
        let count = index + 1;
        writeln!(f, "    {}: {} {}", count, included.date, included.entry)?;
        write!(f, "    {}: {} {}", count + 1, excluded.date, excluded.entry)
    }
}

mod beet_command {
    use crate::DateEntry;
    use anyhow::Context as _;
    use std::io::BufRead as _;

    pub struct BeetCommand<'a> {
        /// Path to the `beet` command from the package `beets`
        beet_command: std::path::PathBuf,
        /// List of argument tokens that were originally comma-separated
        ///
        /// Example:
        ///  - desired args "arg1" "arg2," "arg3" "arg4"
        ///  - argument TIMELESS_ARGS="arg1\narg2,\narg3\narg4"
        ///  - leads to this representation: &[ &["arg1", "arg2"], &["arg3", "arg4"] ]
        timeless_filter_sets: Vec<Vec<&'a str>>,
        /// truncates results to the specified entry count
        max_entries: usize,
    }
    impl BeetCommand<'_> {
        pub fn new(
            beet_command: std::path::PathBuf,
            timeless_args: &str,
            max_entries: usize,
        ) -> anyhow::Result<BeetCommand<'_>> {
            let timeless_filter_sets: anyhow::Result<Vec<Vec<_>>> = timeless_args
                .split(',')
                .map(|filter_set| {
                    let elems: Vec<_> = filter_set.lines().collect();
                    if elems.is_empty() {
                        anyhow::bail!("duplicate commas in timeless args")
                    } else {
                        Ok(elems)
                    }
                })
                .collect();

            Ok(BeetCommand {
                beet_command,
                timeless_filter_sets: timeless_filter_sets?,
                max_entries,
            })
        }
    }
    impl BeetCommand<'_> {
        fn new_list_command(&self, extra_filter: Option<&str>) -> std::process::Command {
            let mut command = std::process::Command::new(&self.beet_command);
            command.arg("list");
            for (index, filter_set) in self.timeless_filter_sets.iter().enumerate() {
                let (filter_set, last): (&[&str], &str) = if let Some(extra_filter) = extra_filter {
                    (filter_set, extra_filter)
                } else {
                    let (last, rest) = filter_set.split_last().expect("nonempty filter set");
                    (rest, last)
                };
                for filter_arg in filter_set {
                    command.arg(filter_arg);
                }
                if index + 1 == self.timeless_filter_sets.len() {
                    // final filter_set, no trailing comma
                    command.arg(last);
                } else {
                    // filter_set will follow, append comma to last arg
                    command.arg(&format!("{last},"));
                }
            }
            command
        }

        pub fn query_timeless(&self) -> anyhow::Result<Vec<DateEntry>> {
            let current_output = self
                .new_list_command(None)
                .arg("added-")
                .arg("--format")
                .arg("$added $artist - $album - $title")
                .stdout_check_errors()
                .context("beet ls [current_args]")?;

            current_output
                .lines()
                .enumerate()
                .take(self.max_entries)
                .map(|(number, line)| {
                    DateEntry::try_from(line.with_context(|| {
                        format!("line {} from current_output beet command", number + 1)
                    })?)
                })
                .collect::<anyhow::Result<Vec<_>>>()
        }

        pub fn count_entries_after(&self, entry: &DateEntry) -> anyhow::Result<usize> {
            let output = self
                .new_list_command(Some(&format!("added:{date}..", date = entry.date)))
                .arg("--format")
                .arg("$id")
                .stdout_check_errors()
                .context("beet ls [current_args] added:[selection]..")?;

            output
                .lines()
                .enumerate()
                .try_fold(0, |sum, (number, line)| {
                    let line = line.with_context(|| {
                        format!("line {} from current_output beet command", number + 1)
                    })?;
                    let current = if line.trim().is_empty() { 0 } else { 1 };
                    Ok(sum + current)
                })
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
