use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discord_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub football_data_api_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_path: Option<String>,
}

impl FileConfig {
    pub fn apply_to_env_if_missing(&self) {
        for (key, value) in [
            ("DISCORD_TOKEN", &self.discord_token),
            ("FOOTBALL_DATA_API_TOKEN", &self.football_data_api_token),
            ("DATABASE_PATH", &self.database_path),
        ] {
            if let Some(value) = value {
                set_env_if_missing(key, value);
            }
        }
    }
}

fn set_env_if_missing(key: &str, value: &str) {
    if std::env::var(key).is_err() {
        // SAFETY: called during single-threaded startup before other threads read env.
        unsafe { std::env::set_var(key, value) };
    }
}

pub fn config_file_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "league-bot")
        .map(|dirs| dirs.config_dir().join("config.toml"))
        .unwrap_or_else(|| PathBuf::from("config.toml"))
}

pub fn load_file(path: &Path) -> Result<FileConfig, String> {
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    toml::from_str(&contents).map_err(|error| {
        format!(
            "Failed to parse {}: {error}",
            path.display()
        )
    })
}

pub fn load_into_env_if_missing(path: &Path) {
    match load_file(path) {
        Ok(config) => config.apply_to_env_if_missing(),
        Err(error) => eprintln!("Warning: {error}"),
    }
}

pub fn write_file(path: &Path, config: &FileConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true).mode(0o700);
            builder.create(parent).map_err(|error| {
                format!("Failed to create {}: {error}", parent.display())
            })?;
        }
        #[cfg(not(unix))]
        {
            fs::create_dir_all(parent).map_err(|error| {
                format!("Failed to create {}: {error}", parent.display())
            })?;
        }
    }

    let contents = toml::to_string_pretty(config)
        .map_err(|error| format!("Failed to serialize config: {error}"))?;

    #[cfg(unix)]
    {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
        file.write_all(contents.as_bytes())
            .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
    }

    #[cfg(not(unix))]
    {
        fs::write(path, contents)
            .map_err(|error| format!("Failed to write {}: {error}", path.display()))?;
    }

    Ok(())
}

pub fn run_setup() -> Result<(), String> {
    let path = config_file_path();

    if !io::stdin().is_terminal() {
        return Err(
            "Setup needs an interactive terminal. Set DISCORD_TOKEN and FOOTBALL_DATA_API_TOKEN in the environment (for example via a systemd EnvironmentFile) instead."
                .into(),
        );
    }

    if path.exists() {
        print!(
            "Config already exists at {}. Overwrite? [y/N] ",
            path.display()
        );
        io::stdout()
            .flush()
            .map_err(|error| error.to_string())?;
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .map_err(|error| error.to_string())?;
        let answer = answer.trim().to_ascii_lowercase();
        if answer != "y" && answer != "yes" {
            return Err("Setup cancelled.".into());
        }
    }

    print!("Discord bot token: ");
    io::stdout().flush().map_err(|error| error.to_string())?;
    let discord_token = rpassword::read_password().map_err(|error| error.to_string())?;
    if discord_token.trim().is_empty() {
        return Err("Discord bot token cannot be empty.".into());
    }

    print!("football-data.org API token (Enter to skip, e.g. if set in the environment): ");
    io::stdout().flush().map_err(|error| error.to_string())?;
    let football_data_api_token = rpassword::read_password().map_err(|error| error.to_string())?;
    let football_data_api_token = non_empty(&football_data_api_token);

    print!("SQLite database path (optional, Enter for default league_bot.db): ");
    io::stdout().flush().map_err(|error| error.to_string())?;
    let mut database_path_line = String::new();
    io::stdin()
        .read_line(&mut database_path_line)
        .map_err(|error| error.to_string())?;
    let database_path = non_empty(&database_path_line);

    let config = FileConfig {
        discord_token: Some(discord_token.trim().to_string()),
        football_data_api_token,
        database_path,
    };
    write_file(&path, &config)?;
    println!("Wrote config to {}.", path.display());
    Ok(())
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn require_env(name: &str) -> Result<String, String> {
    std::env::var(name).map_err(|_| {
        format!(
            "{name} is not set. Run `league-bot setup`, set {name} in the environment, or use a systemd EnvironmentFile."
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_config(contents: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, contents).unwrap();
        (dir, path)
    }

    #[test]
    fn load_parses_toml() {
        let (_dir, path) = temp_config(
            r#"
discord_token = "discord"
football_data_api_token = "football"
database_path = "/var/lib/league_bot.db"
"#,
        );
        let config = load_file(&path).unwrap();
        assert_eq!(config.discord_token.as_deref(), Some("discord"));
        assert_eq!(config.football_data_api_token.as_deref(), Some("football"));
        assert_eq!(
            config.database_path.as_deref(),
            Some("/var/lib/league_bot.db")
        );
    }

    #[test]
    fn load_accepts_partial_config() {
        let (_dir, path) = temp_config("discord_token = \"only-discord\"\n");
        let config = load_file(&path).unwrap();
        assert_eq!(config.discord_token.as_deref(), Some("only-discord"));
        assert_eq!(config.football_data_api_token, None);
        assert_eq!(config.database_path, None);
    }

    #[test]
    fn round_trip_toml() {
        let config = FileConfig {
            discord_token: Some("d".into()),
            football_data_api_token: None,
            database_path: None,
        };
        let text = toml::to_string_pretty(&config).unwrap();
        let parsed: FileConfig = toml::from_str(&text).unwrap();
        assert_eq!(config, parsed);
    }
}
