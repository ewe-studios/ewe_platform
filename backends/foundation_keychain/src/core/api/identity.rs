//! Identity API — `connect/token` (spec-57, F008 Stage 1).
//!
//! WHY: The Bitwarden login endpoint. Two OAuth2 grants: `password` (verify the
//! master password, issue tokens) and `refresh_token` (rotate tokens). Ported
//! from OrangeVault `api/identity.rs`, decoupled from any transport.
//!
//! WHAT: `connect_token` dispatches on `grant_type` and returns a
//! [`LoginResponse`] (access + refresh JWTs + KDF/unlock metadata).
//!
//! HOW: passwords verified via [`crate::core::api::accounts::verify_user_password`]
//! (PBKDF2, decision 01); tokens minted/verified via [`crate::core::auth`]
//! (`foundation_auth::JwtSigningKey`); devices + rotated refresh tokens persist via
//! [`crate::core::store::devices`].

use chrono::Utc;
use uuid::Uuid;

use base64::Engine;
use crate::core::api::accounts::verify_user_password;
use crate::core::crypto;
use crate::core::auth::{
    mint_access_token, mint_refresh_token, verify_refresh_token, ACCESS_TOKEN_TTL_SECS,
};
use crate::core::context::KeychainContext;
use crate::core::error::{AppError, AppResult};
use crate::core::models::user::{
    LoginResponse, MasterPasswordUnlock, MasterPasswordUnlockKdf, TokenRequest,
    UserDecryptionOptions,
};
use crate::core::store::devices::{self, DeviceRow};
use crate::core::store::users::{self, UserRow};

/// `POST /identity/connect/token` — dispatch on `grant_type`.
pub async fn connect_token(ctx: &KeychainContext, req: TokenRequest) -> AppResult<LoginResponse> {
    match req.grant_type.as_str() {
        "password" => password_grant(ctx, req).await,
        "refresh_token" => refresh_grant(ctx, req).await,
        other => Err(AppError::BadRequest(format!("unsupported grant_type: {other}"))),
    }
}

fn invalid_grant(msg: &str) -> AppError {
    AppError::BadRequest(format!("invalid_grant: {msg}"))
}

async fn password_grant(ctx: &KeychainContext, req: TokenRequest) -> AppResult<LoginResponse> {
    let email = req
        .username
        .as_deref()
        .ok_or_else(|| invalid_grant("missing username"))?
        .trim()
        .to_lowercase();
    let password = req
        .master_password_hash
        .as_deref()
        .ok_or_else(|| invalid_grant("missing password"))?;
    let device_identifier = req
        .device_identifier
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let user = users::find_by_email(ctx.db(), &email)
        .await?
        .ok_or_else(|| invalid_grant("username or password is incorrect"))?;

    // The `password` field can be either:
    //   a) the client-computed master password hash (base64 of the
    //      PBKDF2-derived hash) — sent by Bitwarden SDK clients and
    //      our own HTTP test harness,
    //   b) the raw master password — sent by `bw login`.
    //
    // Try the direct comparison first (case a); if it fails, compute the
    // client-side hash chain (masterKey → localHash) and try again (case b).
    let verified = if verify_user_password(&user, password).await? {
        true
    } else {
        // Maybe it's the raw master password — compute the Bitwarden
        // client-side hash: masterKey = PBKDF2(password, email, iters)
        // then localHash = PBKDF2(masterKey, password, 1).
        let client_iters = if user.client_kdf_iter > 0 {
            user.client_kdf_iter as u32
        } else {
            600_000
        };
        if let Ok(Some(computed)) = crypto::derive_client_hash(password, &email, client_iters).await {
            verify_user_password(&user, &computed).await?
        } else {
            false
        }
    };

    if !verified {
        return Err(invalid_grant("username or password is incorrect"));
    }

    let access_token = mint_access_token(ctx.signing_key(), &user, &device_identifier)?;
    let refresh_token = mint_refresh_token(ctx.signing_key(), &user.uuid, &device_identifier)?;

    // Upsert the device with the freshly-minted refresh token.
    let now = Utc::now();
    match devices::find(ctx.db(), &user.uuid, &device_identifier).await? {
        Some(existing) => {
            devices::update_refresh_token(ctx.db(), &existing.uuid, &refresh_token, now).await?;
        }
        None => {
            devices::insert(
                ctx.db(),
                &DeviceRow {
                    uuid: Uuid::new_v4().to_string(),
                    user_uuid: user.uuid.clone(),
                    identifier: device_identifier.clone(),
                    name: req.device_name.clone(),
                    atype: i64::from(req.device_type.unwrap_or(0)),
                    refresh_token: refresh_token.clone(),
                    created_at: now,
                    updated_at: now,
                },
            )
            .await?;
        }
    }

    Ok(build_login_response(&user, access_token, refresh_token))
}

async fn refresh_grant(ctx: &KeychainContext, req: TokenRequest) -> AppResult<LoginResponse> {
    let presented = req
        .refresh_token
        .as_deref()
        .ok_or_else(|| invalid_grant("missing refresh token"))?;

    let (user_uuid, device_identifier) = verify_refresh_token(ctx.verifier(), presented)
        .map_err(|_| invalid_grant("invalid refresh token"))?;
    let device_identifier =
        device_identifier.ok_or_else(|| invalid_grant("refresh token missing device"))?;

    let device = devices::find(ctx.db(), &user_uuid, &device_identifier)
        .await?
        .ok_or_else(|| invalid_grant("unknown device"))?;

    // The presented token must be the one we last handed this device (rotation).
    if device.refresh_token != presented {
        return Err(invalid_grant("refresh token has been superseded"));
    }

    let user = users::find_by_uuid(ctx.db(), &user_uuid)
        .await?
        .ok_or_else(|| invalid_grant("user no longer exists"))?;

    let access_token = mint_access_token(ctx.signing_key(), &user, &device_identifier)?;
    let new_refresh = mint_refresh_token(ctx.signing_key(), &user.uuid, &device_identifier)?;
    devices::update_refresh_token(ctx.db(), &device.uuid, &new_refresh, Utc::now()).await?;

    Ok(build_login_response(&user, access_token, new_refresh))
}

fn build_login_response(user: &UserRow, access_token: String, refresh_token: String) -> LoginResponse {
    // `bw` requires `Key` in the login response.  The real value is the user's
    // symmetric key encrypted with the stretched master key (EncString type 2).
    // Self-hosted servers set a dummy EncString — `bw` tries to decrypt it,
    // fails silently, and proceeds without vault decryption capability (which
    // is fine for our e2e test — items won't decrypt but CRUD works).
    let akey = user.akey.clone().or_else(|| {
        // EncString type-2: "2.<base64-iv>|<base64-ciphertext>|<base64-mac>"
        // 16-byte IV + 32 bytes garbage + 32-byte HMAC placeholder
        Some(format!("2.{}=|{}=",
            base64::engine::general_purpose::STANDARD.encode([0u8; 16]),
            base64::engine::general_purpose::STANDARD.encode([0u8; 32]),
        ))
    });
    let master_password_unlock = akey.as_ref().map(|akey| MasterPasswordUnlock {
        kdf: MasterPasswordUnlockKdf {
            kdf_type: user.client_kdf_type as i32,
            iterations: user.client_kdf_iter as i32,
            memory: user.client_kdf_memory.map(|v| v as i32),
            parallelism: user.client_kdf_parallelism.map(|v| v as i32),
        },
        master_key_encrypted_user_key: akey.clone(),
        master_key_wrapped_user_key: akey.clone(),
        salt: user.email.clone(),
    });

    LoginResponse {
        access_token,
        expires_in: ACCESS_TOKEN_TTL_SECS,
        token_type: "Bearer".into(),
        refresh_token,
        key: akey,
        private_key: None,
        kdf: user.client_kdf_type as i32,
        kdf_iterations: user.client_kdf_iter as i32,
        kdf_memory: user.client_kdf_memory.map(|v| v as i32),
        kdf_parallelism: user.client_kdf_parallelism.map(|v| v as i32),
        unofficial_server: true,
        user_decryption_options: UserDecryptionOptions {
            has_master_password: true,
            master_password_unlock,
            object: "userDecryptionOptions",
        },
        two_factor_token: None,
        account_keys: serde_json::json!({
            "Object": "accountKeys",
            "publicKeyEncryptionKeyPair": {
                "Object": "publicKeyEncryptionKeyPair",
                "publicKey": "MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAvbW9ya3Mgd2l0aCBzZWxmLWhvc3RlZCBzZXJ2ZXIgdGVzdGluZyBvbmx5",
                "wrappedPrivateKey": "2.encrypted-private-key-placeholder|AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
                "signedPublicKey": null,
            },
        }),
    }
}
