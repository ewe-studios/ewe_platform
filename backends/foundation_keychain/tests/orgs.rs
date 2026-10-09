//! Organizations vault vertical — integration tests (spec-57, F008 Stage 1).
//!
//! Org create (owner membership + default collection) → collections CRUD →
//! member invite/confirm/list → permission enforcement, over a real Turso store.

use std::sync::Arc;

use foundation_core::valtron::{collect_one, execute, from_future, valtron_test};
use foundation_db::core::storage_provider::AsyncQueryStore;
use foundation_db::{StorageBackend, StorageProvider};
use foundation_keychain::core::api::orgs;
use foundation_keychain::core::models::org::{
    CollectionCreateRequest, ConfirmMemberRequest, InviteMemberRequest, OrganizationCreateRequest,
    OrganizationUserType,
};
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::{AppError, KeychainContext};
use tempfile::TempDir;

const OWNER: &str = "user-owner";
const OWNER_EMAIL: &str = "owner@example.com";
const OUTSIDER: &str = "user-outsider";

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: std::future::Future<Output = T> + Send + 'static,
{
    let task = from_future(future);
    let stream = execute(task, None).expect("valtron execute");
    collect_one(stream).expect("future produced a result")
}

fn fresh_ctx() -> (KeychainContext, TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("keychain_test.db");
    let provider = StorageProvider::new(StorageBackend::Turso {
        url: db_path.to_str().unwrap().to_string(),
    })
    .expect("init turso");
    let db: Arc<dyn AsyncQueryStore> = Arc::new(provider);
    let db_for_schema = Arc::clone(&db);
    drive(async move { apply_schema(db_for_schema.as_ref()).await }).expect("apply schema");
    (KeychainContext::new(db), dir)
}

fn make_org(ctx: &KeychainContext) -> String {
    let c = ctx.clone();
    drive(async move {
        orgs::create(
            &c,
            OWNER,
            OWNER_EMAIL,
            OrganizationCreateRequest {
                name: "Acme".into(),
                billing_email: None,
                key: Some("2.orgkey|enc".into()),
                collection_name: Some("Shared".into()),
            },
        )
        .await
    })
    .expect("create org")
    .id
}

#[valtron_test]
fn create_sets_owner_membership_and_default_collection() {
    let (ctx, _dir) = fresh_ctx();
    let org_id = make_org(&ctx);

    // Owner can read the org.
    let c = ctx.clone();
    let oid = org_id.clone();
    let org = drive(async move { orgs::get(&c, OWNER, &oid).await }).expect("get");
    assert_eq!(org.name, "Acme");
    assert!(org.enabled);

    // Owner's profile lists the org as a confirmed owner.
    let c = ctx.clone();
    let mine = drive(async move { orgs::list_for_user(&c, OWNER).await }).expect("list_for_user");
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].member_type, OrganizationUserType::Owner);
    assert_eq!(mine[0].status, 2);

    // The default collection exists.
    let c = ctx.clone();
    let oid = org_id.clone();
    let cols = drive(async move { orgs::list_collections(&c, OWNER, &oid).await }).expect("cols");
    assert_eq!(cols.len(), 1);
    assert_eq!(cols[0].name, "Shared");
}

#[valtron_test]
fn collections_crud() {
    let (ctx, _dir) = fresh_ctx();
    let org_id = make_org(&ctx);

    let c = ctx.clone();
    let oid = org_id.clone();
    let col = drive(async move {
        orgs::create_collection(&c, OWNER, &oid, CollectionCreateRequest {
            name: "Engineering".into(),
            external_id: None,
        })
        .await
    })
    .expect("create collection");
    assert_eq!(col.name, "Engineering");

    let c = ctx.clone();
    let oid = org_id.clone();
    assert_eq!(drive(async move { orgs::list_collections(&c, OWNER, &oid).await }).expect("list").len(), 2);

    let c = ctx.clone();
    let oid = org_id.clone();
    let cid = col.id.clone();
    drive(async move { orgs::delete_collection(&c, OWNER, &oid, &cid).await }).expect("delete col");

    let c = ctx.clone();
    assert_eq!(drive(async move { orgs::list_collections(&c, OWNER, &org_id).await }).expect("list2").len(), 1);
}

#[valtron_test]
fn invite_confirm_and_list_members() {
    let (ctx, _dir) = fresh_ctx();
    let org_id = make_org(&ctx);

    let c = ctx.clone();
    let oid = org_id.clone();
    let invited = drive(async move {
        orgs::invite_member(&c, OWNER, &oid, InviteMemberRequest {
            email: "New@Example.com".into(),
            member_type: OrganizationUserType::User,
        })
        .await
    })
    .expect("invite");
    assert_eq!(invited.email, "new@example.com");
    assert_eq!(invited.status, 0); // invited

    // Owner + invited = 2 members.
    let c = ctx.clone();
    let oid = org_id.clone();
    let members = drive(async move { orgs::list_members(&c, OWNER, &oid).await }).expect("members");
    assert_eq!(members.len(), 2);

    // Confirm the invited member.
    let c = ctx.clone();
    let oid = org_id.clone();
    let mid = invited.id.clone();
    drive(async move {
        orgs::confirm_member(&c, OWNER, &oid, &mid, ConfirmMemberRequest { key: Some("2.mk|enc".into()) }).await
    })
    .expect("confirm");

    let c = ctx.clone();
    let members = drive(async move { orgs::list_members(&c, OWNER, &org_id).await }).expect("members2");
    let confirmed = members.iter().find(|m| m.id == invited.id).unwrap();
    assert_eq!(confirmed.status, 2);
}

#[valtron_test]
fn non_members_are_forbidden() {
    let (ctx, _dir) = fresh_ctx();
    let org_id = make_org(&ctx);

    let c = ctx.clone();
    let oid = org_id.clone();
    assert!(matches!(
        drive(async move { orgs::get(&c, OUTSIDER, &oid).await }),
        Err(AppError::Forbidden)
    ));

    let c = ctx.clone();
    let oid = org_id.clone();
    assert!(matches!(
        drive(async move {
            orgs::create_collection(&c, OUTSIDER, &oid, CollectionCreateRequest {
                name: "x".into(),
                external_id: None,
            })
            .await
        }),
        Err(AppError::Forbidden)
    ));

    // Owner deletes the org; afterwards even the owner is no longer a member.
    let c = ctx.clone();
    let oid = org_id.clone();
    drive(async move { orgs::delete(&c, OWNER, &oid).await }).expect("delete org");
    let c = ctx.clone();
    assert!(drive(async move { orgs::get(&c, OWNER, &org_id).await }).is_err());
}
