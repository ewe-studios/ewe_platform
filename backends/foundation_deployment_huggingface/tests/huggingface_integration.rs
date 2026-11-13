//! `HuggingFace` Hub API integration tests.
//!
//! These tests are ignored by default and require a valid `HuggingFace` token.
//! Set the `HF_TOKEN` environment variable or ensure you have a valid token file.
//!
//! Run with: `cargo test --package foundation_deployment_huggingface -- --ignored --nocapture`

use foundation_core::valtron::valtron_test;
use foundation_deployment_huggingface::{
    client, repository, types, HFClientBuilder, HuggingFaceError,
};

fn get_token() -> Option<String> {
    let token = std::env::var("HF_TOKEN").ok();
    if let Some(t) = &token {
        // Never log token contents — only its presence and length.
        tracing::info!("HF_TOKEN found, length: {}", t.len());
    } else {
        tracing::warn!("HF_TOKEN not set in environment!");
    }
    token
}

#[valtron_test]
#[ignore = "requires HF_TOKEN environment variable"]
async fn test_whoami() -> Result<(), HuggingFaceError> {
    let token =
        get_token().expect("HF_TOKEN environment variable must be set for integration tests");

    let client = HFClientBuilder::new().token(token).build()?;

    let user = client::whoami(&client).await?;

    tracing::info!("Authenticated as: {}", user.username);
    if let Some(fullname) = user.fullname {
        tracing::info!("Full name: {}", fullname);
    }
    if let Some(avatar) = user.avatar_url {
        tracing::info!("Avatar URL: {}", avatar);
    }
    tracing::info!("User type: {:?}", user.user_type);
    tracing::info!("Is pro: {:?}", user.is_pro);

    assert_ne!(user.username, "");
    Ok(())
}

#[valtron_test]
#[ignore = "requires HF_TOKEN environment variable"]
async fn test_auth_check() -> Result<(), HuggingFaceError> {
    let token =
        get_token().expect("HF_TOKEN environment variable must be set for integration tests");

    let client = HFClientBuilder::new().token(token).build()?;

    client::auth_check(&client).await?;

    tracing::info!("Authentication successful!");
    Ok(())
}

#[valtron_test]
#[ignore = "requires HF_TOKEN environment variable"]
async fn test_list_models() -> Result<(), HuggingFaceError> {
    let token =
        get_token().expect("HF_TOKEN environment variable must be set for integration tests");

    let client = HFClientBuilder::new().token(token).build()?;

    let params = types::ListModelsParams {
        limit: Some(5),
        ..Default::default()
    };

    let stream = client::list_models(&client, &params).await?;

    let models: Vec<_> = foundation_core::valtron::collect_result(stream);

    tracing::info!("Fetched {} models:", models.len());
    for (i, result) in models.iter().enumerate() {
        match result {
            Ok(model) => tracing::info!(
                "  {}. {} (author: {})",
                i + 1,
                model.id,
                model.author.as_deref().unwrap_or("unknown")
            ),
            Err(e) => tracing::error!("  {}. Error: {}", i + 1, e),
        }
    }

    assert!(!models.is_empty());
    Ok(())
}

#[valtron_test]
#[ignore = "requires HF_TOKEN environment variable"]
async fn test_list_datasets() -> Result<(), HuggingFaceError> {
    let token =
        get_token().expect("HF_TOKEN environment variable must be set for integration tests");

    let client = HFClientBuilder::new().token(token).build()?;

    let params = types::ListDatasetsParams {
        limit: Some(5),
        ..Default::default()
    };

    let stream = client::list_datasets(&client, &params).await?;

    let datasets: Vec<_> = foundation_core::valtron::collect_result(stream);

    tracing::info!("Fetched {} datasets:", datasets.len());
    for (i, result) in datasets.iter().enumerate() {
        match result {
            Ok(dataset) => tracing::info!(
                "  {}. {} (author: {})",
                i + 1,
                dataset.id,
                dataset.author.as_deref().unwrap_or("unknown")
            ),
            Err(e) => tracing::error!("  {}. Error: {}", i + 1, e),
        }
    }

    assert!(!datasets.is_empty());
    Ok(())
}

#[valtron_test]
#[ignore = "requires HF_TOKEN environment variable"]
async fn test_repository_info() -> Result<(), HuggingFaceError> {
    let token =
        get_token().expect("HF_TOKEN environment variable must be set for integration tests");

    let client = HFClientBuilder::new().token(token).build()?;

    // Test with a well-known public model (namespaced owner/name).
    let repo = client.model("google-bert".to_string(), "bert-base-uncased".to_string());

    // This should work even without auth for public repos
    match repository::repo_info(&repo, &types::RepoInfoParams::default()).await {
        Ok(info) => {
            tracing::info!("Repository info retrieved successfully");
            if let types::RepoInfo::Model(model) = info {
                tracing::info!("  Model ID: {}", model.id);
                tracing::info!("  Downloads: {:?}", model.downloads);
                tracing::info!("  Likes: {:?}", model.likes);
            } else {
                tracing::warn!("Unexpected repo type");
            }
        }
        Err(e) => {
            tracing::warn!("Could not fetch repo info (may need auth): {}", e);
        }
    }

    Ok(())
}
