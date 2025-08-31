use clap::{ArgMatches, Command, Parser};
use clap_serde::CommandWrap;
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
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
    #[arg(long, conflicts_with = "json_file")]
    json: Option<String>,

    #[arg(long, value_name = "FILE", conflicts_with = "json")]
    json_file: Option<PathBuf>,

    #[arg(last = true, help = "Arguments passed to the main command")]
    trailing: Vec<String>,
}

#[derive(Debug)]
struct Config {
    clap_config: Command,
    command_config: CommandConfig,
}

#[derive(Debug, Deserialize)]
struct CommandConfig {
    executable: Option<PathBuf>,

    #[serde(default)]
    args: Vec<HashMap<String, ArgConfig>>,

    #[serde(default)]
    subcommands: Vec<HashMap<String, CommandConfig>>,
}

#[derive(Debug, Deserialize)]
struct ArgConfig {
    env_var: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let json_data = if let Some(json) = cli.json {
        json
    } else if let Some(file) = cli.json_file {
        fs::read_to_string(file).expect("Failed to read JSON file")
    } else {
        anyhow::bail!("You must provide either --json or --json-file")
    };

    let config: Config = serde_json::from_str(&json_data)?;

    let app = config.clap_config;
    let command_config = config.command_config;

    let mut args = cli.trailing;
    args.insert(0, app.get_name().to_string());

    let matches = app.clone().get_matches_from(args);

    run(&app, &matches, &command_config, BTreeMap::new())
}

fn run(
    command: &Command,
    args: &ArgMatches,
    config: &CommandConfig,
    mut env: BTreeMap<String, String>,
) -> anyhow::Result<()> {
    let env_vars = create_env_vars(command, args, config);
    env.extend(env_vars);

    if let Some((name, subargs)) = args.subcommand() {
        let subconfig = get_subcommand_config(config, name);
        let subcommand = get_subcommand(command, name);

        run(subcommand, subargs, subconfig, env)
    } else {
        if let Some(executable) = &config.executable {
            let error = ProcCommand::new(executable).envs(env).exec();
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

fn get_subcommand_config<'a>(config: &'a CommandConfig, name: &str) -> &'a CommandConfig {
    for subcommand in config.subcommands.iter() {
        for (subcommand_name, config) in subcommand.iter() {
            if subcommand_name == name {
                return config;
            }
        }
    }
    todo!()
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

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut full_json = Value::deserialize(deserializer)?;
        let runtime_json = extract_runtime(&mut full_json);

        let clap_config =
            serde_json::to_string_pretty(&full_json).map_err(serde::de::Error::custom)?;
        let clap_config: CommandWrap = serde_json::from_str(&clap_config).unwrap();

        let command_config: CommandConfig =
            serde_json::from_value(runtime_json).map_err(serde::de::Error::custom)?;

        Ok(Config {
            clap_config: clap_config.into(),
            command_config,
        })
    }
}

fn extract_runtime(v: &mut Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut runtime_map = serde_json::Map::new();

            for key in ["executable", "env_var"] {
                if let Some(val) = map.remove(key) {
                    runtime_map.insert(key.to_string(), val);
                }
            }

            if let Some(Value::Array(args)) = map.get_mut("args") {
                let runtime_args: Vec<Value> = args
                    .iter_mut()
                    .map(|arg| match arg {
                        Value::Object(object) => {
                            let (name, value) = object.iter_mut().next().unwrap();
                            let value = extract_runtime(value);
                            let mut map = Map::new();
                            map.insert(name.to_string(), value);
                            Value::Object(map)
                        }
                        _ => todo!(),
                    })
                    .collect();
                if !runtime_args.is_empty() {
                    runtime_map.insert("args".to_string(), Value::Array(runtime_args));
                }
            }

            if let Some(subs) = map.get_mut("subcommands") {
                match subs {
                    Value::Array(arr) => {
                        let runtime_subs: Vec<Value> = arr
                            .iter_mut()
                            .map(|sub| match sub {
                                Value::Object(object) => {
                                    let (name, value) = object.iter_mut().next().unwrap();
                                    let value = extract_runtime(value);
                                    let mut map = Map::new();
                                    map.insert(name.to_string(), value);
                                    Value::Object(map)
                                }
                                _ => todo!(),
                            })
                            .collect();
                        runtime_map.insert("subcommands".to_string(), Value::Array(runtime_subs));
                    }
                    _ => {
                        todo!()
                    }
                }
            }

            Value::Object(runtime_map)
        }
        Value::Array(arr) => {
            Value::Array(arr.iter_mut().map(|item| extract_runtime(item)).collect())
        }
        _ => Value::Null,
    }
}
