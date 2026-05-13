#![cfg(feature = "remote-ssh")]

use bitfun_services_integrations::remote_ssh::{
    local_workspace_stable_storage_id, normalize_remote_workspace_path,
    remote_root_to_mirror_subpath, remote_workspace_stable_id,
    sanitize_remote_mirror_path_component, sanitize_ssh_connection_id_for_local_dir,
    sanitize_ssh_hostname_for_mirror, unresolved_remote_session_storage_key, workspace_logical_key,
    RemoteWorkspace, RemoteWorkspaceRegistry, SSHAuthMethod, SSHConnectionConfig, SavedAuthType,
    SavedConnection, LOCAL_WORKSPACE_SSH_HOST,
};

#[test]
fn remote_ssh_legacy_agent_auth_maps_to_default_private_key() {
    let config: SSHConnectionConfig = serde_json::from_value(serde_json::json!({
        "id": "conn-1",
        "name": "dev",
        "host": "example.com",
        "port": 22,
        "username": "alice",
        "auth": { "type": "Agent" },
        "defaultWorkspace": "/repo"
    }))
    .unwrap();

    match config.auth {
        SSHAuthMethod::PrivateKey {
            key_path,
            passphrase,
        } => {
            assert_eq!(key_path, "~/.ssh/id_rsa");
            assert_eq!(passphrase, None);
        }
        SSHAuthMethod::Password { .. } => panic!("legacy agent auth must map to private key"),
    }

    let saved: SavedConnection = serde_json::from_value(serde_json::json!({
        "id": "conn-1",
        "name": "dev",
        "host": "example.com",
        "port": 22,
        "username": "alice",
        "authType": { "type": "Agent" },
        "defaultWorkspace": "/repo",
        "lastConnected": 1
    }))
    .unwrap();

    match saved.auth_type {
        SavedAuthType::PrivateKey { key_path } => assert_eq!(key_path, "~/.ssh/id_rsa"),
        SavedAuthType::Password => panic!("legacy agent auth type must map to private key"),
    }
}

#[test]
fn remote_workspace_defaults_keep_older_files_loadable() {
    let workspace: RemoteWorkspace = serde_json::from_value(serde_json::json!({
        "connectionId": "conn-1"
    }))
    .unwrap();

    assert_eq!(workspace.connection_id, "conn-1");
    assert_eq!(workspace.remote_path, "");
    assert_eq!(workspace.connection_name, "");
    assert_eq!(workspace.ssh_host, "");
}

#[test]
fn remote_workspace_path_helpers_preserve_current_identity_contract() {
    assert_eq!(
        normalize_remote_workspace_path(r"\\home\\user\\repo//src"),
        "/home/user/repo/src"
    );
    assert_eq!(normalize_remote_workspace_path("///"), "/");
    assert_eq!(
        normalize_remote_workspace_path("/home/user/repo/"),
        "/home/user/repo"
    );

    #[cfg(windows)]
    assert_eq!(
        sanitize_ssh_connection_id_for_local_dir("ssh-root@1.95.50.146:22"),
        "ssh-root@1.95.50.146-22"
    );
    #[cfg(not(windows))]
    assert_eq!(
        sanitize_ssh_connection_id_for_local_dir("ssh-root@1.95.50.146:22"),
        "ssh-root@1.95.50.146:22"
    );

    assert_eq!(sanitize_remote_mirror_path_component(""), "_");
    assert_eq!(
        sanitize_ssh_hostname_for_mirror(" Example.COM "),
        "example.com"
    );
    assert_eq!(
        remote_root_to_mirror_subpath("/home/user/repo"),
        std::path::PathBuf::from("home").join("user").join("repo")
    );
    assert_eq!(
        remote_root_to_mirror_subpath("/"),
        std::path::PathBuf::from("_root")
    );

    assert_eq!(
        workspace_logical_key(LOCAL_WORKSPACE_SSH_HOST, "/Users/p/w"),
        "localhost:/Users/p/w"
    );

    let local_id = local_workspace_stable_storage_id("/Users/foo/BitFun");
    assert_eq!(local_id, "local_1d9bbee7a88cb84fc9500423130a3e99");

    let remote_id = remote_workspace_stable_id("myhost", "/root/proj");
    assert_eq!(remote_id, "remote_0b6e9c54b3e51fd56bf721ed35c1ce88");

    let unresolved_key = unresolved_remote_session_storage_key(" conn-1 ", "/home/u/p");
    assert_eq!(unresolved_key, "d1c72f60fc1b7cb99599cf21");
}

#[tokio::test]
async fn remote_workspace_registry_preserves_ambiguous_root_resolution_contract() {
    let registry = RemoteWorkspaceRegistry::new();
    registry
        .register_remote_workspace(
            "/".to_string(),
            "conn-a".to_string(),
            "Server A".to_string(),
            "host-a".to_string(),
        )
        .await;
    registry
        .register_remote_workspace(
            "/".to_string(),
            "conn-b".to_string(),
            "Server B".to_string(),
            "host-b".to_string(),
        )
        .await;

    assert!(registry.lookup_connection("/tmp", None).await.is_none());

    registry
        .set_active_connection_hint(Some("conn-a".to_string()))
        .await;
    let hinted = registry.lookup_connection("/tmp", None).await.unwrap();
    assert_eq!(hinted.connection_id, "conn-a");
    assert_eq!(hinted.ssh_host, "host-a");

    let preferred = registry
        .lookup_connection("/tmp", Some("conn-b"))
        .await
        .unwrap();
    assert_eq!(preferred.connection_id, "conn-b");
    assert_eq!(preferred.ssh_host, "host-b");
}

#[tokio::test]
async fn remote_workspace_registry_preserves_legacy_state_and_clear_contract() {
    let registry = RemoteWorkspaceRegistry::new();
    assert!(!registry.has_any().await);
    assert!(!registry.get_state().await.is_active);

    registry
        .register_remote_workspace(
            "/repo".to_string(),
            "conn-1".to_string(),
            "Dev Server".to_string(),
            "dev.example.com".to_string(),
        )
        .await;

    let state = registry.get_state().await;
    assert!(state.is_active);
    assert_eq!(state.connection_id.as_deref(), Some("conn-1"));
    assert_eq!(state.remote_path.as_deref(), Some("/repo"));
    assert_eq!(state.connection_name.as_deref(), Some("Dev Server"));

    registry
        .unregister_remote_workspace("conn-1", "/repo")
        .await;
    assert!(!registry.has_any().await);
    assert!(!registry.get_state().await.is_active);
}
