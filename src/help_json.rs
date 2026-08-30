//! Machine-readable dump of the command tree, for the documentation site.
//!
//! `teams --help-json` emits the whole clap command tree as JSON so
//! `ms-teams-cli-docs` can regenerate its command reference on each release
//! instead of hand-maintaining ~190 pages. The shape is the contract that
//! `docs/scripts/render-cli-reference.mjs` consumes; changing a field name
//! here breaks that renderer.
//!
//! `examples` and `exit_codes` are emitted empty on purpose. They are prose
//! rather than anything clap knows, so they are curated on the docs side and
//! merged in by command path when the reference is rendered.

use clap::{Arg, Command};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HelpJson {
    pub version: String,
    pub binary: String,
    pub description: String,
    pub commands: Vec<CommandJson>,
}

#[derive(Debug, Serialize)]
pub struct CommandJson {
    pub name: String,
    pub path: Vec<String>,
    pub summary: String,
    pub description: String,
    pub usage: String,
    pub flags: Vec<FlagJson>,
    pub examples: Vec<serde_json::Value>,
    pub exit_codes: Vec<serde_json::Value>,
    pub subcommands: Vec<CommandJson>,
}

#[derive(Debug, Serialize)]
pub struct FlagJson {
    pub name: String,
    pub description: String,
    pub alias: Option<String>,
    pub value_name: Option<String>,
    pub required: bool,
    pub env: Option<String>,
    pub default: Option<String>,
}

/// Render the tree rooted at `cmd`.
pub fn build(mut cmd: Command) -> HelpJson {
    // Populate propagated globals and generated help/version args, so the
    // filtering below sees the same argument set a real parse would.
    cmd.build();

    let description = cmd
        .get_about()
        .map(|s| s.to_string())
        .unwrap_or_else(|| "Microsoft Teams CLI".to_string());

    let commands = cmd
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set() && sub.get_name() != "help")
        .map(|sub| command_json(sub, &[]))
        .collect();

    HelpJson {
        version: env!("CARGO_PKG_VERSION").to_string(),
        binary: cmd.get_name().to_string(),
        description,
        commands,
    }
}

fn command_json(cmd: &Command, parents: &[String]) -> CommandJson {
    let name = cmd.get_name().to_string();
    let mut path = parents.to_vec();
    path.push(name.clone());

    let summary = cmd.get_about().map(|s| s.to_string()).unwrap_or_default();
    let description = cmd
        .get_long_about()
        .map(|s| s.to_string())
        .unwrap_or_else(|| summary.clone());

    let subcommands = cmd
        .get_subcommands()
        .filter(|sub| !sub.is_hide_set() && sub.get_name() != "help")
        .map(|sub| command_json(sub, &path))
        .collect();

    CommandJson {
        name,
        summary,
        description,
        usage: usage_for(cmd, &path),
        flags: cmd.get_arguments().filter_map(flag_json).collect(),
        examples: Vec::new(),
        exit_codes: Vec::new(),
        subcommands,
        path,
    }
}

/// `teams auth login [OPTIONS] --device-code`, without clap's leading
/// "Usage: ". clap already renders the full command path, so it is used as
/// given; the full path is only a fallback for a command that renders none.
fn usage_for(cmd: &Command, path: &[String]) -> String {
    let rendered = cmd.clone().render_usage().to_string();
    let line = rendered
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .trim()
        .to_string();
    let line = line
        .strip_prefix("Usage:")
        .unwrap_or(&line)
        .trim()
        .to_string();

    if line.is_empty() {
        format!("teams {}", path.join(" "))
    } else {
        line
    }
}

/// Command-specific options only. Globals are documented once in the
/// reference's "Global Options" table, and `help`/`version` are clap's own.
fn flag_json(arg: &Arg) -> Option<FlagJson> {
    if arg.is_global_set() {
        return None;
    }
    let id = arg.get_id().as_str();
    if id == "help" || id == "version" {
        return None;
    }
    if arg.is_hide_set() {
        return None;
    }

    let long = arg.get_long().map(|l| format!("--{l}"));
    let short = arg.get_short().map(|s| format!("-{s}"));
    // A positional has neither; name it by its value placeholder.
    let name = long
        .clone()
        .or_else(|| short.clone())
        .unwrap_or_else(|| id.to_uppercase());

    let value_name = arg
        .get_value_names()
        .and_then(|names| names.first().map(|n| n.to_string()));

    let default = {
        let defaults = arg.get_default_values();
        if defaults.is_empty() {
            None
        } else {
            Some(
                defaults
                    .iter()
                    .map(|v| v.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(", "),
            )
        }
    };

    Some(FlagJson {
        // When both forms exist the long one is the name and the short is the alias,
        // which is the order the renderer prints them in.
        alias: if long.is_some() { short } else { None },
        name,
        description: arg
            .get_help()
            .map(|h| h.to_string())
            .unwrap_or_default()
            .replace('\n', " "),
        value_name,
        required: arg.is_required_set(),
        env: arg
            .get_env()
            .map(|e| e.to_string_lossy().into_owned())
            .filter(|e| !e.is_empty()),
        default,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::Cli;
    use clap::CommandFactory;

    fn tree() -> HelpJson {
        build(Cli::command())
    }

    fn find<'a>(commands: &'a [CommandJson], path: &[&str]) -> &'a CommandJson {
        let (head, rest) = path.split_first().expect("non-empty path");
        let found = commands
            .iter()
            .find(|c| c.name == *head)
            .unwrap_or_else(|| panic!("no command {head}"));
        if rest.is_empty() {
            found
        } else {
            find(&found.subcommands, rest)
        }
    }

    #[test]
    fn the_tree_carries_the_crate_version_and_binary_name() {
        let tree = tree();
        assert_eq!(tree.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(tree.binary, "teams");
        assert!(!tree.description.is_empty());
        assert!(tree.commands.len() > 10, "{}", tree.commands.len());
    }

    #[test]
    fn nested_commands_carry_their_full_path() {
        let tree = tree();
        let login = find(&tree.commands, &["auth", "login"]);
        assert_eq!(login.path, vec!["auth", "login"]);
        assert!(
            login.usage.starts_with("teams auth login"),
            "{}",
            login.usage
        );
    }

    /// The renderer prints globals once in its own table, so a command's flag
    /// list must not repeat them — otherwise every one of ~190 pages grows a
    /// duplicate `--output`/`--profile` block.
    #[test]
    fn global_options_are_not_repeated_on_every_command() {
        let tree = tree();
        let send = find(&tree.commands, &["message", "send"]);
        let names: Vec<&str> = send.flags.iter().map(|f| f.name.as_str()).collect();
        for global in ["--output", "--profile", "--quiet", "--help", "--version"] {
            assert!(!names.contains(&global), "{global} leaked into {names:?}");
        }
        assert!(names.contains(&"--body"), "{names:?}");
    }

    /// These are the fields this release actually added; if the emitter ever
    /// stops reporting them the docs silently lose the new surface.
    #[test]
    fn newly_added_flags_reach_the_dump() {
        let commands = tree().commands;
        let send = find(&commands, &["message", "send"]);
        let mention = send
            .flags
            .iter()
            .find(|f| f.name == "--mention")
            .expect("--mention");
        assert_eq!(mention.value_name.as_deref(), Some("USER"));
        assert!(!mention.required);

        let set = find(&commands, &["presence", "set"]);
        let availability = set
            .flags
            .iter()
            .find(|f| f.name == "--availability")
            .expect("--availability");
        assert!(availability.required, "availability is a required option");
        assert!(
            availability.description.contains("Available"),
            "{}",
            availability.description
        );
    }

    #[test]
    fn a_flag_with_an_env_var_reports_it() {
        // --profile is global, so reach for a command-level env-backed flag via the
        // root's own argument list instead.
        let mut root = Cli::command();
        root.build();
        let profile = root
            .get_arguments()
            .find(|a| a.get_id() == "profile")
            .expect("profile arg");
        assert_eq!(
            profile
                .get_env()
                .map(|e| e.to_string_lossy().into_owned())
                .as_deref(),
            Some("TEAMS_CLI_PROFILE")
        );
    }

    /// clap synthesises a `help` subcommand under every parent. Including them
    /// tripled the command count and would have produced ~280 junk pages.
    #[test]
    fn synthesised_help_subcommands_are_excluded() {
        fn walk(commands: &[CommandJson], found: &mut Vec<String>) {
            for c in commands {
                if c.name == "help" {
                    found.push(c.path.join(" "));
                }
                walk(&c.subcommands, found);
            }
        }
        let tree = tree();
        let mut found = Vec::new();
        walk(&tree.commands, &mut found);
        assert!(found.is_empty(), "help subcommands leaked: {found:?}");
    }

    /// clap renders the full path already; splicing it in again produced
    /// "teams message send message send [OPTIONS]".
    #[test]
    fn usage_states_the_command_path_exactly_once() {
        let tree = tree();
        let send = find(&tree.commands, &["message", "send"]);
        assert_eq!(
            send.usage.matches("message send").count(),
            1,
            "{}",
            send.usage
        );
        assert!(
            send.usage.starts_with("teams message send"),
            "{}",
            send.usage
        );
    }

    /// The docs renderer curates these itself and merges them by path.
    #[test]
    fn examples_and_exit_codes_are_left_for_the_docs_to_supply() {
        let tree = tree();
        let login = find(&tree.commands, &["auth", "login"]);
        assert!(login.examples.is_empty());
        assert!(login.exit_codes.is_empty());
    }

    #[test]
    fn the_dump_serializes_to_the_shape_the_renderer_expects() {
        let value = serde_json::to_value(tree()).unwrap();
        for key in ["version", "binary", "description", "commands"] {
            assert!(value.get(key).is_some(), "missing {key}");
        }
        let first = &value["commands"][0];
        for key in [
            "name",
            "path",
            "summary",
            "description",
            "usage",
            "flags",
            "examples",
            "exit_codes",
            "subcommands",
        ] {
            assert!(first.get(key).is_some(), "command missing {key}");
        }
    }
}
