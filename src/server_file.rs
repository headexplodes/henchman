use std::fs;
use std::io::{Error as IoError};
use std::path::{Path};

use toml::de::Error as TomlError;

use serde::{Deserialize};

use crate::task_file::ConfigFileError;

#[derive(Debug, Deserialize)]
pub struct ServerToml {
    pub server: Option<ServerServerToml>,
    pub auth: Option<ServerAuthToml>,
}

#[derive(Debug, Deserialize)]
pub struct ServerServerToml {
    pub listen: Option<String>,
    pub dir: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ServerAuthToml {
    #[serde(default)]
    pub users: Vec<ServerAuthUserToml>
}

#[derive(Debug, Deserialize)]
pub struct ServerAuthUserToml {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub roles: Vec<String>,
}

pub fn load_toml<T>(path: &Path) -> Result<T, ConfigFileError> where T: serde::de::DeserializeOwned {
    let from_io_err = |err: IoError| -> ConfigFileError {
        ConfigFileError::Io(err, Some(path.into()))
    };

    let from_toml_err = |err: TomlError| -> ConfigFileError {
        ConfigFileError::Toml(err, path.into())
    };

    let file_str = fs::read_to_string(path).map_err(from_io_err)?;

    let model = toml::from_str::<T>(&file_str).map_err(from_toml_err)?;

    Ok(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::{PathBuf};

    const CARGO_MANIFEST_DIR: &'static str = env!("CARGO_MANIFEST_DIR");

    //noinspection SpellCheckingInspection
    const EXPECTED_PASSWORD_HASH: &'static str =
        "0100002710053615732b4de713b68cf98b3405e06ac373182d28c9932f19569177addbb63889db74bc6ecdb6ab54a5d5f395356c1e";

    #[test]
    fn test_parse_server_toml() {
        let path: PathBuf = format!("{}/tests/resources/server.toml", CARGO_MANIFEST_DIR).into();

        let server_toml = load_toml::<ServerToml>(&path).unwrap();

        assert!(server_toml.server.is_some());
        assert_eq!(server_toml.server.as_ref().unwrap().listen, Some("127.0.0.1:0".to_owned()));
        assert_eq!(server_toml.server.as_ref().unwrap().dir, Some("tasks".to_owned()));

        assert!(server_toml.auth.is_some());
        assert_eq!(server_toml.auth.as_ref().unwrap().users.len(), 1);
        assert_eq!(server_toml.auth.as_ref().unwrap().users[0].username, "admin");
        assert_eq!(server_toml.auth.as_ref().unwrap().users[0].password, EXPECTED_PASSWORD_HASH);
        assert_eq!(server_toml.auth.as_ref().unwrap().users[0].roles, vec!["ADMIN"]);
    }
}