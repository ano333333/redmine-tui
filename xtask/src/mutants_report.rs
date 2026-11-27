//! cargo-mutantsの結果を集計し、前回結果との差分をmarkdownで出力する。

use std::collections::HashMap;
use std::fs;
use std::path::Path;

const OUTCOMES: [&str; 4] = ["caught", "missed", "timeout", "unviable"];
const MISSED: usize = 1;

pub(crate) fn mutants_report(args: Vec<String>) -> Result<(), String> {
    let Some((current, previous)) = parse_args(args)? else {
        println!("{}", crate::usage());
        return Ok(());
    };
    let current = Outcomes::read(Path::new(&current))?;
    let previous = previous
        .map(|dir| Outcomes::read(Path::new(&dir)))
        .transpose()?;
    print!("{}", render(&current, previous.as_ref()));
    Ok(())
}

fn parse_args(args: Vec<String>) -> Result<Option<(String, Option<String>)>, String> {
    let mut current = None;
    let mut previous = None;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => return Ok(None),
            "--previous" => {
                previous = Some(args.next().ok_or_else(|| {
                    format!("--previous needs a directory\n\n{}", crate::usage())
                })?);
            }
            _ if arg.starts_with('-') || current.is_some() => {
                return Err(format!(
                    "unknown mutants-report option: {arg}\n\n{}",
                    crate::usage()
                ));
            }
            _ => current = Some(arg),
        }
    }
    let current =
        current.ok_or_else(|| format!("mutants-report needs a directory\n\n{}", crate::usage()))?;
    Ok(Some((current, previous)))
}

struct Outcomes {
    lists: [Vec<String>; 4],
}

impl Outcomes {
    fn read(dir: &Path) -> Result<Self, String> {
        let mut lists: [Vec<String>; 4] = Default::default();
        for (list, name) in lists.iter_mut().zip(OUTCOMES) {
            let path = dir.join(format!("{name}.txt"));
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
            *list = text
                .lines()
                .filter(|line| !line.is_empty())
                .map(str::to_string)
                .collect();
        }
        Ok(Self { lists })
    }

    fn missed(&self) -> &[String] {
        &self.lists[MISSED]
    }
}

/// 行番号と列はコードの移動で変わるため外し、ファイルと変更内容が同じmutantを同一とみなす。
fn mutant_key(line: &str) -> String {
    let Some((file, rest)) = line.split_once(':') else {
        return line.to_string();
    };
    let mut parts = rest.splitn(3, ':');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(row), Some(column), Some(description))
            if row.parse::<u32>().is_ok() && column.parse::<u32>().is_ok() =>
        {
            format!("{file}:{description}")
        }
        _ => line.to_string(),
    }
}

/// 同じkeyのmutantが複数あり得るため、`base`での出現回数を超えた分だけを返す。
fn excess<'a>(lines: &'a [String], base: &[String]) -> Vec<&'a str> {
    let mut remaining: HashMap<String, usize> = HashMap::new();
    for line in base {
        *remaining.entry(mutant_key(line)).or_default() += 1;
    }
    lines
        .iter()
        .filter(|line| match remaining.get_mut(&mutant_key(line)) {
            Some(count) if *count > 0 => {
                *count -= 1;
                false
            }
            _ => true,
        })
        .map(String::as_str)
        .collect()
}

fn render(current: &Outcomes, previous: Option<&Outcomes>) -> String {
    let mut out = String::from("# cargo-mutants\n\n");
    match previous {
        Some(_) => out.push_str("| outcome | count | change |\n|---|---:|---:|\n"),
        None => out.push_str("| outcome | count |\n|---|---:|\n"),
    }
    for (index, name) in OUTCOMES.iter().enumerate() {
        let count = current.lists[index].len();
        match previous {
            Some(previous) => {
                let change = count as i64 - previous.lists[index].len() as i64;
                out.push_str(&format!("| {name} | {count} | {change:+} |\n"));
            }
            None => out.push_str(&format!("| {name} | {count} |\n")),
        }
    }

    if let Some(previous) = previous {
        push_list(
            &mut out,
            "Newly missed",
            &excess(current.missed(), previous.missed()),
        );
        push_list(
            &mut out,
            "No longer missed (lines as of the previous run)",
            &excess(previous.missed(), current.missed()),
        );
    }

    let missed = current.missed();
    out.push_str(&format!(
        "\n<details><summary>All missed ({})</summary>\n\n",
        missed.len()
    ));
    for line in missed {
        out.push_str(&format!("- `{line}`\n"));
    }
    out.push_str("\n</details>\n");
    out
}

fn push_list(out: &mut String, title: &str, lines: &[&str]) {
    out.push_str(&format!("\n## {title} ({})\n\n", lines.len()));
    if lines.is_empty() {
        out.push_str("None.\n");
    }
    for line in lines {
        out.push_str(&format!("- `{line}`\n"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| line.to_string()).collect()
    }

    fn outcomes(caught: &[&str], missed: &[&str]) -> Outcomes {
        Outcomes {
            lists: [lines(caught), lines(missed), Vec::new(), Vec::new()],
        }
    }

    #[test]
    fn mutant_key_drops_line_and_column() {
        assert_eq!(
            mutant_key("src/vos/id.rs:9:14: replace == with != in EntityIdValue::same_as"),
            "src/vos/id.rs: replace == with != in EntityIdValue::same_as"
        );
    }

    #[test]
    fn excess_ignores_moved_lines_and_counts_duplicates() {
        let current = lines(&[
            "src/a.rs:12:5: replace f -> bool with true",
            "src/a.rs:30:9: replace + with -",
            "src/a.rs:31:9: replace + with -",
        ]);
        let previous = lines(&[
            "src/a.rs:10:5: replace f -> bool with true",
            "src/a.rs:28:9: replace + with -",
            "src/b.rs:3:5: replace g with ()",
        ]);

        assert_eq!(
            excess(&current, &previous),
            vec!["src/a.rs:31:9: replace + with -"]
        );
        assert_eq!(
            excess(&previous, &current),
            vec!["src/b.rs:3:5: replace g with ()"]
        );
    }

    #[test]
    fn render_shows_changes_from_previous_run() {
        let current = outcomes(
            &[
                "src/a.rs:1:1: replace f with ()",
                "src/a.rs:2:1: replace g with ()",
            ],
            &["src/b.rs:4:1: replace h with ()"],
        );
        let previous = outcomes(
            &["src/a.rs:1:1: replace f with ()"],
            &["src/a.rs:2:1: replace g with ()"],
        );

        assert_eq!(
            render(&current, Some(&previous)),
            "# cargo-mutants\n\
             \n\
             | outcome | count | change |\n\
             |---|---:|---:|\n\
             | caught | 2 | +1 |\n\
             | missed | 1 | +0 |\n\
             | timeout | 0 | +0 |\n\
             | unviable | 0 | +0 |\n\
             \n\
             ## Newly missed (1)\n\
             \n\
             - `src/b.rs:4:1: replace h with ()`\n\
             \n\
             ## No longer missed (lines as of the previous run) (1)\n\
             \n\
             - `src/a.rs:2:1: replace g with ()`\n\
             \n\
             <details><summary>All missed (1)</summary>\n\
             \n\
             - `src/b.rs:4:1: replace h with ()`\n\
             \n\
             </details>\n"
        );
    }
}
