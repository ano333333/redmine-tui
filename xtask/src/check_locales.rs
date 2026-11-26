//! 各言語のメッセージカタログが、英語のカタログと同じメッセージと変数を持つかを検査する。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
use fluent_syntax::parser;

const LOCALES_DIR: &str = "locales";
const REFERENCE_LANGUAGE: &str = "en";

/// メッセージと属性（`id.attr`）ごとの、参照する変数名。
type Catalog = BTreeMap<String, BTreeSet<String>>;

pub(crate) fn check_locales(args: Vec<String>) -> Result<(), String> {
    if let Some(arg) = args.first() {
        if arg == "--help" || arg == "-h" {
            println!("{}", crate::usage());
            return Ok(());
        }
        return Err(format!(
            "unknown check-locales option: {arg}\n\n{}",
            crate::usage()
        ));
    }

    let locales = Path::new(LOCALES_DIR);
    let reference_dir = locales.join(REFERENCE_LANGUAGE);
    let mut problems = Vec::new();
    for language in list_dir(locales)? {
        if language == REFERENCE_LANGUAGE {
            continue;
        }
        let target_dir = locales.join(&language);
        let reference_files = list_dir(&reference_dir)?;
        let target_files = list_dir(&target_dir)?;
        for file in reference_files.union(&target_files) {
            let label = format!("{language}/{file}");
            if !target_files.contains(file) {
                problems.push(format!("{label}: missing file"));
                continue;
            }
            if !reference_files.contains(file) {
                problems.push(format!(
                    "{label}: no {REFERENCE_LANGUAGE}/{file} to compare with"
                ));
                continue;
            }
            let reference = read_catalog(&reference_dir.join(file))?;
            let target = read_catalog(&target_dir.join(file))?;
            problems.extend(
                compare(&reference, &target)
                    .into_iter()
                    .map(|problem| format!("{label}: {problem}")),
            );
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

fn list_dir(dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries =
        fs::read_dir(dir).map_err(|error| format!("failed to read {}: {error}", dir.display()))?;
    entries
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|error| format!("failed to read {}: {error}", dir.display()))
        })
        .collect()
}

fn read_catalog(path: &Path) -> Result<Catalog, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    parse_catalog(&source).map_err(|reason| format!("{}: {reason}", path.display()))
}

fn parse_catalog(source: &str) -> Result<Catalog, String> {
    let resource = parser::parse(source).map_err(|(_, errors)| format!("{errors:?}"))?;
    let mut catalog = Catalog::new();
    for entry in resource.body {
        let (id, value, attributes) = match entry {
            Entry::Message(message) => (
                message.id.name.to_string(),
                message.value,
                message.attributes,
            ),
            Entry::Term(term) => (
                format!("-{}", term.id.name),
                Some(term.value),
                term.attributes,
            ),
            _ => continue,
        };
        if let Some(value) = value {
            catalog.insert(id.clone(), variables(&value));
        }
        for attribute in attributes {
            catalog.insert(
                format!("{id}.{}", attribute.id.name),
                variables(&attribute.value),
            );
        }
    }
    Ok(catalog)
}

fn variables(pattern: &Pattern<&str>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    collect_pattern(pattern, &mut names);
    names
}

fn collect_pattern(pattern: &Pattern<&str>, names: &mut BTreeSet<String>) {
    for element in &pattern.elements {
        if let PatternElement::Placeable { expression } = element {
            collect_expression(expression, names);
        }
    }
}

fn collect_expression(expression: &Expression<&str>, names: &mut BTreeSet<String>) {
    match expression {
        Expression::Select { selector, variants } => {
            collect_inline(selector, names);
            for variant in variants {
                collect_pattern(&variant.value, names);
            }
        }
        Expression::Inline(inline) => collect_inline(inline, names),
    }
}

fn collect_inline(expression: &InlineExpression<&str>, names: &mut BTreeSet<String>) {
    match expression {
        InlineExpression::VariableReference { id } => {
            names.insert(id.name.to_string());
        }
        InlineExpression::FunctionReference { arguments, .. } => {
            for argument in &arguments.positional {
                collect_inline(argument, names);
            }
            for argument in &arguments.named {
                collect_inline(&argument.value, names);
            }
        }
        InlineExpression::Placeable { expression } => collect_expression(expression, names),
        // Termへ渡す引数はTerm内だけの変数で、呼び出し側から渡す変数ではない。
        InlineExpression::StringLiteral { .. }
        | InlineExpression::NumberLiteral { .. }
        | InlineExpression::MessageReference { .. }
        | InlineExpression::TermReference { .. } => {}
    }
}

fn compare(reference: &Catalog, target: &Catalog) -> Vec<String> {
    let mut problems = Vec::new();
    for (id, reference_variables) in reference {
        match target.get(id) {
            None => problems.push(format!("missing `{id}`")),
            Some(target_variables) if target_variables != reference_variables => {
                problems.push(format!(
                    "`{id}` uses variables {target_variables:?}, expected {reference_variables:?}"
                ));
            }
            Some(_) => {}
        }
    }
    for id in target.keys() {
        if !reference.contains_key(id) {
            problems.push(format!("unexpected `{id}`"));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problems(reference: &str, target: &str) -> Vec<String> {
        compare(
            &parse_catalog(reference).unwrap(),
            &parse_catalog(target).unwrap(),
        )
    }

    #[test]
    fn matching_catalogs_have_no_problems() {
        assert!(problems("a = A { $x }\n", "a = エー { $x }\n").is_empty());
    }

    #[test]
    fn reports_missing_and_unexpected_messages() {
        assert_eq!(
            problems("a = A\nb = B\n", "a = エー\nc = シー\n"),
            ["missing `b`", "unexpected `c`"]
        );
    }

    #[test]
    fn reports_missing_attributes() {
        assert_eq!(
            problems("a = A\n    .title = T\n", "a = エー\n"),
            ["missing `a.title`"]
        );
    }

    #[test]
    fn reports_variables_that_differ_including_those_in_select_variants() {
        assert_eq!(
            problems(
                "a = { $n -> \n    [one] one { $item }\n   *[other] many\n}\n",
                "a = { $n -> \n    [one] 1つ\n   *[other] { $items }\n}\n"
            ),
            ["`a` uses variables {\"items\", \"n\"}, expected {\"item\", \"n\"}"]
        );
    }

    #[test]
    fn syntax_errors_are_reported() {
        assert!(parse_catalog("a = { $x\n").is_err());
    }
}
