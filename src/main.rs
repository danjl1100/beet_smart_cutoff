use anyhow::Context as _;
use beet_smart_cutoff::{
    beet_command::BeetCommand, find_transition, json, prompt::Prompt, DateEntry, Transition,
};
use clap::Parser;
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

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let ParsedArgs {
        beets,
        max_entries,
        output_file_key,
    } = {
        let beets = BeetCommand::new(args.beet_command, &args.timeless_args, args.max_entries);
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

    let subtitle = output_file_key
        .as_ref()
        .map(|(_, key)| format!(" - key {key:?}"))
        .unwrap_or_default();
    println!("## ");
    println!("## beet_smart_cutoff{subtitle}");
    println!("## ");

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
    // FIXME debug format is tacky
    println!("Final {final_count} entries, from choice {date_entry:?}");

    if let Some((json_file, key)) = json_file_key {
        let json::JsonFile { map, path } = json_file;
        let path = &path;
        let mut map = map.unwrap_or_default();

        map.insert(key, date_entry.date.clone().into());
        json::write_json_file(path, map).with_context(|| format!("writing json file {path:?}"))?;
    }

    Ok(())
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
