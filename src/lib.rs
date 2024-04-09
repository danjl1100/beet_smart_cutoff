type JsonMap = serde_json::Map<String, serde_json::Value>;
pub mod json;

pub mod prompt;

pub mod beet_command;

#[derive(Debug)]
pub struct DateEntry {
    pub date: String,
    pub entry: String,
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

#[derive(Clone, Copy, Debug)]
pub struct Transition<'a> {
    pub index: usize,
    pub included: &'a DateEntry,
    pub excluded: &'a DateEntry,
}
pub fn find_transition(items: &[DateEntry], target_count: usize) -> Option<Transition<'_>> {
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
