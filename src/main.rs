use clap::{ArgMatches, Command, Parser};
use clap_serde::CommandWrap;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command as ProcCommand;

#[derive(Parser, Debug)]
#[command(
    name = "clap-bash",
    version = "1.0.0",
    author = "FlorianNAdam",
    about = "A simple clap based arg parser for bash scripts"
)]
struct Cli {
    #[arg(long, conflicts_with = "clap_json_file")]
    clap_json: Option<String>,

    #[arg(long, value_name = "FILE", conflicts_with = "clap_json")]
    clap_json_file: Option<PathBuf>,

    #[arg(long, conflicts_with = "run_json_file")]
    run_json: Option<String>,

    #[arg(long, value_name = "FILE", conflicts_with = "run_json")]
    run_json_file: Option<PathBuf>,

    #[arg(last = true, help = "Arguments passed to the main command")]
    trailing: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(flatten)]
    clap_config: CommandWrap,
    #[serde(flatten)]
    command_config: CommandConfig,
}

#[derive(Debug, Deserialize)]
struct CommandConfig {
    executable: Option<PathBuf>,

    #[serde(default)]
    args: Vec<HashMap<String, ArgConfig>>,

    #[serde(default)]
    subcommands: HashMap<String, CommandConfig>,
}

#[derive(Debug, Deserialize)]
struct ArgConfig {
    env_var: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let clap_json_data = if let Some(json) = cli.clap_json {
        json
    } else if let Some(file) = cli.clap_json_file {
        fs::read_to_string(file).expect("Failed to read JSON file")
    } else {
        eprintln!("You must provide either --clap-json or --clap-json-file");
        std::process::exit(1);
    };

    let app: clap::Command = serde_json::from_str::<clap_serde::CommandWrap>(&clap_json_data)
        .expect("Failed to parse JSON into clap::Command")
        .into();

    let run_json_data = if let Some(json) = cli.run_json {
        json
    } else if let Some(file) = cli.run_json_file {
        fs::read_to_string(file).expect("Failed to read JSON file")
    } else {
        eprintln!("You must provide either --run-json or --run-json-file");
        std::process::exit(1);
    };

    let config: CommandConfig = serde_json::from_str(&run_json_data).expect("Failed to parse JSON");

    let mut args = cli.trailing;
    args.insert(0, app.get_name().to_string());

    let matches = app.clone().get_matches_from(args);

    run(&app, &matches, &config)
}

fn run(command: &Command, args: &ArgMatches, config: &CommandConfig) -> anyhow::Result<()> {
    if let Some((name, subargs)) = args.subcommand() {
        let subconfig = config
            .subcommands
            .get(name)
            .expect("Missing subcommand in config");

        let subcommand = get_subcommand(command, name);

        run(subcommand, subargs, subconfig)
    } else {
        if let Some(executable) = &config.executable {
            println!("run root");

            let env_vars = create_env_vars(command, args, config);

            let error = ProcCommand::new(executable).envs(env_vars).exec();
            Err(error.into())
        } else {
            anyhow::bail!("Missing executable")
        }
    }
}

fn get_subcommand<'a>(command: &'a Command, name: &str) -> &'a Command {
    command
        .get_subcommands()
        .find(|cmd| cmd.get_name() == name)
        .expect("Missing subcommand")
}

fn get_arg_config<'a>(config: &'a CommandConfig, name: &str) -> &'a ArgConfig {
    for arg in config.args.iter() {
        for (arg_name, config) in arg.iter() {
            if arg_name == name {
                return config;
            }
        }
    }
    todo!()
}

fn create_env_vars(
    command: &Command,
    args: &ArgMatches,
    config: &CommandConfig,
) -> BTreeMap<String, String> {
    let mut mapping = BTreeMap::new();
    for arg in command.get_arguments() {
        let arg_name = arg.get_id().as_str();
        let Some(raw_arg_value) = args.get_raw(&arg_name) else {
            continue;
        };

        let arg_value = raw_arg_value
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(",");

        let arg_config = get_arg_config(config, arg_name);

        let env_var_name = arg_config
            .env_var
            .clone()
            .unwrap_or_else(|| to_env_var_name(arg_name));

        mapping.insert(env_var_name, arg_value);
    }
    mapping
}

fn to_env_var_name(input: &str) -> String {
    input
        .chars()
        .enumerate()
        .map(|(i, c)| {
            let c = if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            };
            if i == 0 && !c.is_ascii_alphabetic() && c != '_' {
                '_'
            } else {
                c
            }
        })
        .map(|c| c.to_ascii_uppercase())
        .collect()
}
