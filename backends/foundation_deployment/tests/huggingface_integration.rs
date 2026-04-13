//! `HuggingFace` Hub API integration tests.
//!
//! These tests are ignored by default and require a valid `HuggingFace` token.
//! Set the `HF_TOKEN` environment variable or ensure you have a valid token file.
//!
//! Run with: `cargo test --package foundation_deployment --features huggingface huggingface -- --ignored --nocapture`

#[cfg(feature = "huggingface")]
mod tests {
    use foundation_deployment::providers::huggingface::{HFClientBuilder, HuggingFaceError, client, repository, types};
    use foundation_core::valtron;
    use tracing_test::traced_test;

    fn get_token() -> Option<String> {
        let token = std::env::var("HF_TOKEN").ok();
        if token.is_none() {
            eprintln!("WARNING: HF_TOKEN not set in environment!");
        } else {
            let t = token.as_ref().unwrap();
            eprintln!("HF_TOKEN found, length: {}, starts with: {}", t.len(), &t[..10.min(t.len())]);
        }
        token
    }

    fn init_valtron() -> valtron::PoolGuard {
        valtron::initialize_pool(42, Some(4))
    }

    #[test]
    #[ignore = "requires HF_TOKEN environment variable"]
    #[traced_test]
    fn test_whoami() -> Result<(), HuggingFaceError> {
        let _guard = init_valtron();
        let token = get_token().expect("HF_TOKEN environment variable must be set for integration tests");

        let client = HFClientBuilder::new()
            .token(token)
            .build()?;

        let user = client::whoami(&client)?;

        tracing::info!("Authenticated as: {}", user.username);
        if let Some(fullname) = user.fullname {
            tracing::info!("Full name: {}", fullname);
        }
        if let Some(avatar) = user.avatar_url {
            tracing::info!("Avatar URL: {}", avatar);
        }
        tracing::info!("User type: {:?}", user.user_type);
        tracing::info!("Is pro: {:?}", user.is_pro);

        assert!(!user.username.is_empty());
        Ok(())
    }

    #[test]
    #[ignore = "requires HF_TOKEN environment variable"]
    #[traced_test]
    fn test_auth_check() -> Result<(), HuggingFaceError> {
        let _guard = init_valtron();
        let token = get_token().expect("HF_TOKEN environment variable must be set for integration tests");

        let client = HFClientBuilder::new()
            .token(token)
            .build()?;

        client::auth_check(&client)?;

        tracing::info!("Authentication successful!");
        Ok(())
    }

    #[test]
    #[ignore = "requires HF_TOKEN environment variable"]
    #[traced_test]
    fn test_list_models() -> Result<(), Box<dyn std::error::Error>> {
        let _guard = init_valtron();
        let token = get_token().expect("HF_TOKEN environment variable must be set for integration tests");

        let client = HFClientBuilder::new()
            .token(token)
            .build()?;

        let params = types::ListModelsParams {
            limit: Some(5),
            ..Default::default()
        };

        let stream = client::list_models(&client, &params)?;

        let models: Vec<_> = foundation_core::valtron::collect_result(stream);

        tracing::info!("Fetched {} models:", models.len());
        for (i, result) in models.iter().enumerate() {
            match result {
                Ok(model) => tracing::info!("  {}. {} (author: {})", i + 1, model.id, model.author.as_deref().unwrap_or("unknown")),
                Err(e) => tracing::error!("  {}. Error: {}", i + 1, e),
            }
        }

        assert!(!models.is_empty());
        Ok(())
    }

    #[test]
    #[ignore = "requires HF_TOKEN environment variable"]
    #[traced_test]
    fn test_list_datasets() -> Result<(), Box<dyn std::error::Error>> {
        let _guard = init_valtron();
        let token = get_token().expect("HF_TOKEN environment variable must be set for integration tests");

        let client = HFClientBuilder::new()
            .token(token)
            .build()?;

        let params = types::ListDatasetsParams {
            limit: Some(5),
            ..Default::default()
        };

        let stream = client::list_datasets(&client, &params)?;

        let datasets: Vec<_> = foundation_core::valtron::collect_result(stream);

        tracing::info!("Fetched {} datasets:", datasets.len());
        for (i, result) in datasets.iter().enumerate() {
            match result {
                Ok(dataset) => tracing::info!("  {}. {} (author: {})", i + 1, dataset.id, dataset.author.as_deref().unwrap_or("unknown")),
                Err(e) => tracing::error!("  {}. Error: {}", i + 1, e),
            }
        }

        assert!(!datasets.is_empty());
        Ok(())
    }

    #[test]
    #[ignore = "requires HF_TOKEN environment variable"]
    #[traced_test]
    fn test_repository_info() -> Result<(), Box<dyn std::error::Error>> {
        let _guard = init_valtron();
        let token = get_token().expect("HF_TOKEN environment variable must be set for integration tests");

        let client = HFClientBuilder::new()
            .token(token)
            .build()?;

        // Test with a well-known public model
        let repo = client.model("bert-base-uncased".to_string(), "");

        // This should work even without auth for public repos
        match repository::repo_info(&repo, &types::RepoInfoParams::default()) {
            Ok(info) => {
                tracing::info!("Repository info retrieved successfully");
                match info {
                    types::RepoInfo::Model(model) => {
                        tracing::info!("  Model ID: {}", model.id);
                        tracing::info!("  Downloads: {:?}", model.downloads);
                        tracing::info!("  Likes: {:?}", model.likes);
                    }
                    _ => tracing::warn!("Unexpected repo type"),
                }
            }
            Err(e) => {
                tracing::warn!("Could not fetch repo info (may need auth): {}", e);
            }
        }

        Ok(())
    }
}
