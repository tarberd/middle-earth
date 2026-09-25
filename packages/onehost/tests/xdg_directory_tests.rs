use std::collections::HashMap;
use std::path::PathBuf;
use onehost::xdg::{XdgBaseDirectories, XdgDirectoryResolutionError};

#[test]
fn test_explicit_xdg_environment_variables_are_honored() {
    let environment_variables: HashMap<String, String> = [
        ("XDG_CONFIG_HOME", "/custom/config"),
        ("XDG_CACHE_HOME", "/custom/cache"),
        ("XDG_DATA_HOME", "/custom/data"),
        ("XDG_STATE_HOME", "/custom/state"),
        ("XDG_RUNTIME_DIR", "/custom/runtime"),
    ]
    .into_iter()
    .map(|(env_key, env_value)| (env_key.to_string(), env_value.to_string()))
    .collect();

    let resolver = XdgBaseDirectories::from_environment_lookup(move |variable_name| {
        environment_variables.get(variable_name).cloned()
    });

    assert_eq!(
        resolver.configuration_home().unwrap(),
        PathBuf::from("/custom/config")
    );
    assert_eq!(
        resolver.cache_home().unwrap(),
        PathBuf::from("/custom/cache")
    );
    assert_eq!(
        resolver.data_home().unwrap(),
        PathBuf::from("/custom/data")
    );
    assert_eq!(
        resolver.state_home().unwrap(),
        PathBuf::from("/custom/state")
    );
    assert_eq!(
        resolver.runtime_directory().unwrap(),
        PathBuf::from("/custom/runtime")
    );
}

#[test]
fn test_fallback_to_home_directory_when_xdg_variables_are_unset() {
    let environment_variables: HashMap<String, String> = [
        ("HOME", "/home/testuser"),
        ("XDG_RUNTIME_DIR", "/run/user/1000"),
    ]
    .into_iter()
    .map(|(env_key, env_value)| (env_key.to_string(), env_value.to_string()))
    .collect();

    let resolver = XdgBaseDirectories::from_environment_lookup(move |variable_name| {
        environment_variables.get(variable_name).cloned()
    });

    assert_eq!(
        resolver.configuration_home().unwrap(),
        PathBuf::from("/home/testuser/.config")
    );
    assert_eq!(
        resolver.cache_home().unwrap(),
        PathBuf::from("/home/testuser/.cache")
    );
    assert_eq!(
        resolver.data_home().unwrap(),
        PathBuf::from("/home/testuser/.local/share")
    );
    assert_eq!(
        resolver.state_home().unwrap(),
        PathBuf::from("/home/testuser/.local/state")
    );
}

#[test]
fn test_missing_home_directory_returns_error_when_fallback_is_needed() {
    let environment_variables = HashMap::<String, String>::new();

    let resolver = XdgBaseDirectories::from_environment_lookup(move |variable_name| {
        environment_variables.get(variable_name).cloned()
    });

    match resolver.configuration_home() {
        Err(XdgDirectoryResolutionError::MissingHomeDirectory { fallback_target }) => {
            assert_eq!(fallback_target, ".config");
        }
        other => panic!("Expected MissingHomeDirectory error, got {:?}", other),
    }

    match resolver.cache_home() {
        Err(XdgDirectoryResolutionError::MissingHomeDirectory { fallback_target }) => {
            assert_eq!(fallback_target, ".cache");
        }
        other => panic!("Expected MissingHomeDirectory error, got {:?}", other),
    }

    match resolver.data_home() {
        Err(XdgDirectoryResolutionError::MissingHomeDirectory { fallback_target }) => {
            assert_eq!(fallback_target, ".local/share");
        }
        other => panic!("Expected MissingHomeDirectory error, got {:?}", other),
    }

    match resolver.state_home() {
        Err(XdgDirectoryResolutionError::MissingHomeDirectory { fallback_target }) => {
            assert_eq!(fallback_target, ".local/state");
        }
        other => panic!("Expected MissingHomeDirectory error, got {:?}", other),
    }
}

#[test]
fn test_missing_runtime_directory_returns_error() {
    let environment_variables: HashMap<String, String> = [("HOME", "/home/testuser")]
        .into_iter()
        .map(|(env_key, env_value)| (env_key.to_string(), env_value.to_string()))
        .collect();

    let resolver = XdgBaseDirectories::from_environment_lookup(move |variable_name| {
        environment_variables.get(variable_name).cloned()
    });

    match resolver.runtime_directory() {
        Err(XdgDirectoryResolutionError::MissingRuntimeDirectory) => (),
        other => panic!("Expected MissingRuntimeDirectory error, got {:?}", other),
    }
}

#[test]
fn test_relative_path_in_xdg_environment_variable_is_rejected() {
    let environment_variables: HashMap<String, String> = [("XDG_CONFIG_HOME", "relative/config")]
        .into_iter()
        .map(|(env_key, env_value)| (env_key.to_string(), env_value.to_string()))
        .collect();

    let resolver = XdgBaseDirectories::from_environment_lookup(move |variable_name| {
        environment_variables.get(variable_name).cloned()
    });

    match resolver.configuration_home() {
        Err(XdgDirectoryResolutionError::RelativePathNotAllowed {
            variable_name,
            provided_path,
        }) => {
            assert_eq!(variable_name, "XDG_CONFIG_HOME");
            assert_eq!(provided_path, PathBuf::from("relative/config"));
        }
        other => panic!("Expected RelativePathNotAllowed error, got {:?}", other),
    }
}

#[test]
fn test_application_specific_directories_append_application_name() {
    let environment_variables: HashMap<String, String> = [
        ("XDG_CONFIG_HOME", "/custom/config"),
        ("XDG_CACHE_HOME", "/custom/cache"),
        ("XDG_DATA_HOME", "/custom/data"),
        ("XDG_STATE_HOME", "/custom/state"),
        ("XDG_RUNTIME_DIR", "/custom/runtime"),
    ]
    .into_iter()
    .map(|(env_key, env_value)| (env_key.to_string(), env_value.to_string()))
    .collect();

    let resolver = XdgBaseDirectories::from_environment_lookup(move |variable_name| {
        environment_variables.get(variable_name).cloned()
    });

    assert_eq!(
        resolver.onehost_configuration_directory().unwrap(),
        PathBuf::from("/custom/config/onehost")
    );
    assert_eq!(
        resolver.onehost_cache_directory().unwrap(),
        PathBuf::from("/custom/cache/onehost")
    );
    assert_eq!(
        resolver.onehost_data_directory().unwrap(),
        PathBuf::from("/custom/data/onehost")
    );
    assert_eq!(
        resolver.onehost_state_directory().unwrap(),
        PathBuf::from("/custom/state/onehost")
    );
    assert_eq!(
        resolver.onehost_runtime_directory().unwrap(),
        PathBuf::from("/custom/runtime/onehost")
    );
}

#[test]
fn test_ensure_application_directories_exist_creates_directories_on_filesystem() {
    let temporary_directory = tempfile::tempdir().unwrap();
    let base_path = temporary_directory.path();

    let configuration_directory = base_path.join("config");
    let cache_directory = base_path.join("cache");
    let data_directory = base_path.join("data");
    let state_directory = base_path.join("state");
    let runtime_directory = base_path.join("runtime");

    let environment_variables: HashMap<String, String> = [
        ("XDG_CONFIG_HOME", configuration_directory.to_str().unwrap().to_string()),
        ("XDG_CACHE_HOME", cache_directory.to_str().unwrap().to_string()),
        ("XDG_DATA_HOME", data_directory.to_str().unwrap().to_string()),
        ("XDG_STATE_HOME", state_directory.to_str().unwrap().to_string()),
        ("XDG_RUNTIME_DIR", runtime_directory.to_str().unwrap().to_string()),
    ]
    .into_iter()
    .map(|(env_key, env_value)| (env_key.to_string(), env_value))
    .collect();

    let resolver = XdgBaseDirectories::from_environment_lookup(move |variable_name| {
        environment_variables.get(variable_name).cloned()
    });

    assert!(!configuration_directory.join("onehost").exists());
    assert!(!cache_directory.join("onehost").exists());
    assert!(!data_directory.join("onehost").exists());
    assert!(!state_directory.join("onehost").exists());
    assert!(!runtime_directory.join("onehost").exists());

    resolver.ensure_application_directories_exist().unwrap();

    assert!(configuration_directory.join("onehost").is_dir());
    assert!(cache_directory.join("onehost").is_dir());
    assert!(data_directory.join("onehost").is_dir());
    assert!(state_directory.join("onehost").is_dir());
    assert!(runtime_directory.join("onehost").is_dir());
}
