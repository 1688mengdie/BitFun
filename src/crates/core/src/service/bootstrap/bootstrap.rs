use crate::util::errors::*;
use log::{debug, warn};
use std::path::Path;
use tokio::fs;

const BOOTSTRAP_FILE_NAME: &str = "BOOTSTRAP.md";
const SOUL_FILE_NAME: &str = "SOUL.md";
const USER_FILE_NAME: &str = "USER.md";
const IDENTITY_FILE_NAME: &str = "IDENTITY.MD";
const BOOTSTRAP_TEMPLATE: &str = include_str!("templates/BOOTSTRAP.md");
const SOUL_TEMPLATE: &str = include_str!("templates/SOUL.md");
const USER_TEMPLATE: &str = include_str!("templates/USER.md");
const IDENTITY_TEMPLATE: &str = include_str!("templates/IDENTITY.MD");
const PERSONA_FILE_NAMES: [&str; 4] = [
    BOOTSTRAP_FILE_NAME,
    SOUL_FILE_NAME,
    USER_FILE_NAME,
    IDENTITY_FILE_NAME,
];

fn normalize_line_endings(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

async fn ensure_markdown_placeholder(path: &Path, content: &str) -> BitFunResult<bool> {
    if path.exists() {
        return Ok(false);
    }

    let normalized_content = normalize_line_endings(content);
    fs::write(path, normalized_content)
        .await
        .map_err(|e| BitFunError::service(format!("Failed to create {}: {}", path.display(), e)))?;

    Ok(true)
}

pub(crate) async fn ensure_workspace_persona_files_for_prompt(
    workspace_root: &Path,
) -> BitFunResult<()> {
    let bootstrap_path = workspace_root.join(BOOTSTRAP_FILE_NAME);
    let soul_path = workspace_root.join(SOUL_FILE_NAME);
    let user_path = workspace_root.join(USER_FILE_NAME);
    let identity_path = workspace_root.join(IDENTITY_FILE_NAME);

    let bootstrap_exists = bootstrap_path.exists();
    let user_exists = user_path.exists();
    let identity_exists = identity_path.exists();

    let (created_bootstrap, created_soul, created_user, created_identity) = if !bootstrap_exists {
        // Rule 1: when USER + IDENTITY already exist, do not create BOOTSTRAP.
        // Only ensure SOUL exists.
        if user_exists && identity_exists {
            (
                false,
                ensure_markdown_placeholder(&soul_path, SOUL_TEMPLATE).await?,
                false,
                false,
            )
        } else {
            // Rule 2: when USER or IDENTITY is missing, backfill all missing files.
            (
                ensure_markdown_placeholder(&bootstrap_path, BOOTSTRAP_TEMPLATE).await?,
                ensure_markdown_placeholder(&soul_path, SOUL_TEMPLATE).await?,
                ensure_markdown_placeholder(&user_path, USER_TEMPLATE).await?,
                ensure_markdown_placeholder(&identity_path, IDENTITY_TEMPLATE).await?,
            )
        }
    } else {
        // BOOTSTRAP already exists: keep persona set complete.
        (
            false,
            ensure_markdown_placeholder(&soul_path, SOUL_TEMPLATE).await?,
            ensure_markdown_placeholder(&user_path, USER_TEMPLATE).await?,
            ensure_markdown_placeholder(&identity_path, IDENTITY_TEMPLATE).await?,
        )
    };

    debug!(
        "Ensured workspace persona files for prompt: path={}, bootstrap_exists={}, user_exists={}, identity_exists={}, created_bootstrap={}, created_soul={}, created_user={}, created_identity={}",
        workspace_root.display(),
        bootstrap_exists,
        user_exists,
        identity_exists,
        created_bootstrap,
        created_soul,
        created_user,
        created_identity
    );

    Ok(())
}

pub(crate) async fn build_workspace_persona_prompt(
    workspace_root: &Path,
) -> BitFunResult<Option<String>> {
    ensure_workspace_persona_files_for_prompt(workspace_root).await?;

    let mut documents = Vec::new();
    for file_name in PERSONA_FILE_NAMES {
        let file_path = workspace_root.join(file_name);
        if !file_path.exists() {
            continue;
        }

        match fs::read_to_string(&file_path).await {
            Ok(content) => documents.push((file_name, normalize_line_endings(&content))),
            Err(e) => {
                warn!(
                    "Failed to read persona file: path={} error={}",
                    file_path.display(),
                    e
                );
            }
        }
    }

    if documents.is_empty() {
        return Ok(None);
    }

    let bootstrap_detected = documents
        .iter()
        .any(|(file_name, _)| *file_name == BOOTSTRAP_FILE_NAME);

    let mut prompt = String::from("<persona>\n");
    for (file_name, content) in documents {
        prompt.push_str(&format!(
            "<persona_file name=\"{}\" description=\"{}\">\n{}\n</persona_file>\n",
            file_name,
            persona_file_description(file_name),
            content
        ));
    }
    prompt.push_str("</persona>");

    let bootstrap_notice = if bootstrap_detected {
        "\n`BOOTSTRAP.md` has been detected. You MUST follow the instructions in that file to complete the setup. After the setup is complete, `BOOTSTRAP.md` should be deleted as soon as possible."
    } else {
        ""
    };

    Ok(Some(format!(
        r#"# Persona

The following files are located in the workspace root directory and define your role, conversational style, user profile, and related guidance.{}

{}
"#,
        bootstrap_notice, prompt
    )))
}

fn persona_file_description(file_name: &str) -> &'static str {
    match file_name {
        BOOTSTRAP_FILE_NAME => "Bootstrap guidance and initialization instructions",
        SOUL_FILE_NAME => "Core persona, values, and behavioral style",
        USER_FILE_NAME => "User profile, preferences, and collaboration expectations",
        IDENTITY_FILE_NAME => "Identity, role definition, and self-description",
        _ => "Additional persona file",
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_line_endings;

    #[test]
    fn normalize_line_endings_converts_crlf_and_cr_to_lf() {
        let input = "line1\r\nline2\rline3\nline4";
        let normalized = normalize_line_endings(input);

        assert_eq!(normalized, "line1\nline2\nline3\nline4");
    }
}
