//! High-level DNS record operations wrapping the auto-generated async
//! `*_request` functions.

use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::PreparedRequestBuilder;

use crate::generated::shared::{ApiError, ApiResponse};
use crate::generated::zones::{
    dns_records_for_a_zone_create_dns_record_request,
    dns_records_for_a_zone_delete_dns_record_request,
    dns_records_for_a_zone_list_dns_records_request,
    dns_records_for_a_zone_update_dns_record_request, DnsRecordsDnsRecordPost,
    DnsRecordsDnsResponseSingle, DnsRecordsForAZoneCreateDnsRecordArgs,
    DnsRecordsForAZoneDeleteDnsRecordArgs, DnsRecordsForAZoneListDnsRecordsArgs,
    DnsRecordsForAZoneUpdateDnsRecordArgs,
};
use crate::types::{CloudflareError, DnsRecord, DnsRecordInput, DnsRecordType};

fn map_api_error(e: ApiError) -> CloudflareError {
    match e {
        ApiError::HttpStatus { code, body, .. } => CloudflareError::Api {
            status: code,
            message: body.as_deref().unwrap_or("(no body)").to_string(),
        },
        other => CloudflareError::Api { status: 0, message: format!("{other:?}") },
    }
}

/// Convert a typed [`DnsRecordInput`] into the generated (flattened) request
/// body type via a serde round-trip.
fn dns_record_post_body(input: &DnsRecordInput) -> Result<DnsRecordsDnsRecordPost, CloudflareError> {
    let value = serde_json::to_value(input)
        .map_err(|e| CloudflareError::Json(format!("serialize DnsRecordInput: {e}")))?;
    serde_json::from_value(value)
        .map_err(|e| CloudflareError::Json(format!("build DNS record body: {e}")))
}

/// A `builder_mod` closure that injects the `Authorization: Bearer <token>` header.
fn auth_mod(token: &str) -> impl FnOnce(&mut PreparedRequestBuilder) + '_ {
    let t = token.to_string();
    move |b: &mut PreparedRequestBuilder| {
        b.set_header(SimpleHeader::AUTHORIZATION, format!("Bearer {t}"));
    }
}

impl crate::client::CloudflareClient {
    pub async fn list_dns_records(
        &self,
        r#type: Option<DnsRecordType>,
        name: Option<&str>,
    ) -> Result<Vec<DnsRecord>, CloudflareError> {
        let args = DnsRecordsForAZoneListDnsRecordsArgs {
            zone_id: self.zone_id().to_string(),
            r#type: r#type.map(|t| t.as_str().to_string()),
            name: name.map(|n| n.to_string()),
            ..Default::default()
        };
        let response = dns_records_for_a_zone_list_dns_records_request(
            self.http(),
            &args,
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        .map_err(map_api_error)?;
        let results = response.body.result.unwrap_or_default();
        results.iter().map(|entry| {
            let v = serde_json::to_value(entry).unwrap_or_default();
            serde_json::from_value(v)
                .map_err(|e| CloudflareError::Json(format!("deserialize DnsRecord: {e}")))
        }).collect()
    }

    pub async fn upsert_dns_record(&self, record: &DnsRecord) -> Result<String, CloudflareError> {
        let input = DnsRecordInput::from(record);
        // The generated body type is a flattened JSON object; build it from our
        // typed input via a serde round-trip so the request carries the DNS
        // record fields.
        let body = dns_record_post_body(&input)?;
        let existing = self.list_dns_records(Some(record.r#type), Some(&record.name)).await?;

        if let Some(existing_record) = existing.into_iter().next() {
            let args = DnsRecordsForAZoneUpdateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
                dns_record_id: existing_record.id.clone(),
                body,
            };
            dns_records_for_a_zone_update_dns_record_request(
                self.http(), &args, self.base_url(), Some(auth_mod(&self.token())),
            )
            .await
            .map_err(map_api_error)?;
            Ok(existing_record.id)
        } else {
            let args = DnsRecordsForAZoneCreateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
                body,
            };
            let response: ApiResponse<DnsRecordsDnsResponseSingle> =
                dns_records_for_a_zone_create_dns_record_request(
                    self.http(), &args, self.base_url(), Some(auth_mod(&self.token())),
                )
                .await
                .map_err(map_api_error)?;
            let entry = response.body.result.as_ref()
                .ok_or_else(|| CloudflareError::Json("response missing 'result' field".into()))?;
            let v = serde_json::to_value(entry).unwrap_or_default();
            let created: DnsRecord = serde_json::from_value(v)
                .map_err(|e| CloudflareError::Json(format!("deserialize DnsRecord: {e}")))?;
            Ok(created.id)
        }
    }

    pub async fn delete_dns_record(&self, record_id: &str) -> Result<(), CloudflareError> {
        let args = DnsRecordsForAZoneDeleteDnsRecordArgs {
            zone_id: self.zone_id().to_string(),
            dns_record_id: record_id.to_string(),
        };
        dns_records_for_a_zone_delete_dns_record_request(
            self.http(),
            &args,
            self.base_url(),
            Some(auth_mod(&self.token())),
        )
        .await
        .map_err(map_api_error)?;
        Ok(())
    }

    pub async fn delete_dns_records_by_name(
        &self, name: &str, record_type: DnsRecordType,
    ) -> Result<(), CloudflareError> {
        for record in self.list_dns_records(Some(record_type), Some(name)).await? {
            self.delete_dns_record(&record.id).await?;
        }
        Ok(())
    }

    pub async fn find_zone(&self, _domain: &str) -> Result<bool, CloudflareError> {
        use crate::generated::zones::{zones_0_get_request, Zones0GetArgs};
        let args = Zones0GetArgs { zone_id: self.zone_id().to_string() };
        match zones_0_get_request(self.http(), &args, self.base_url(), Some(auth_mod(&self.token()))).await {
            Ok(_) => Ok(true),
            Err(ApiError::HttpStatus { code: 404, .. }) => Ok(false),
            Err(e) => Err(map_api_error(e)),
        }
    }

    pub async fn bootstrap_domain(&self, public_ip: &str) -> Result<(), CloudflareError> {
        let wildcard = format!("*.{}", self.domain());
        let record = DnsRecord {
            id: String::new(), zone_id: self.zone_id().to_string(), name: wildcard,
            r#type: DnsRecordType::A, content: public_ip.to_string(), ttl: 60,
            proxied: false, comment: None, tags: vec![],
            created_on: chrono::Utc::now(), modified_on: chrono::Utc::now(),
        };
        self.upsert_dns_record(&record).await?;
        Ok(())
    }
}
