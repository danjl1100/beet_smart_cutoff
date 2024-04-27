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
    ) -> BeetCommand<'_> {
        let timeless_filter_sets: Vec<Vec<_>> = timeless_args
            .split(',')
            .filter_map(|filter_set| {
                let elems: Vec<_> = filter_set.lines().collect();
                if elems.is_empty() {
                    None
                } else {
                    Some(elems)
                }
            })
            .collect();

        BeetCommand {
            beet_command,
            timeless_filter_sets,
            max_entries,
        }
    }
}
impl BeetCommand<'_> {
    fn new_list_command(&self, extra_filter: Option<&str>) -> std::process::Command {
        let mut command = std::process::Command::new(&self.beet_command);
        command.arg("list");
        match extra_filter {
            Some(extra_filter) if self.timeless_filter_sets.is_empty() => {
                command.arg(extra_filter);
            }
            _ => {
                for (index, filter_set) in self.timeless_filter_sets.iter().enumerate() {
                    let (filter_set, last): (&[&str], &str) = if let Some(extra_filter) =
                        extra_filter
                    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn beet_list_command(timeless_args: &str, extra_filter: Option<&str>) -> Vec<String> {
        let beet_command = PathBuf::from("beet");
        let max_entries = 0;
        let command = BeetCommand::new(beet_command, timeless_args, max_entries)
            .new_list_command(extra_filter);
        command
            .get_args()
            .map(|os_str| os_str.to_str().expect("valid utf8 in test case").to_owned())
            .collect()
    }

    #[test]
    fn beet_command_filter_args() {
        insta::assert_ron_snapshot!("empty", beet_list_command("", None));
        insta::assert_ron_snapshot!("args_simple", beet_list_command("a\nb\nc", None));
        insta::assert_ron_snapshot!(
            "args_compound",
            beet_list_command("a\nb\nc,d\ne,f\ng", None)
        );

        //

        let extra = Some("extra");
        insta::assert_ron_snapshot!("empty_extra", beet_list_command("", extra));
        insta::assert_ron_snapshot!("args_simple_extra", beet_list_command("a\nb\nc", extra));
        insta::assert_ron_snapshot!(
            "args_compound_extra",
            beet_list_command("a\nb\nc,d\ne,f\ng", extra)
        );
    }
}
