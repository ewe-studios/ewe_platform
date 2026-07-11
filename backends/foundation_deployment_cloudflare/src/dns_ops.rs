//! High-level DNS record operations wrapping the auto-generated valtron functions.
//!
//! WHY: Typed API surface for DNS record CRUD. The regenerated `generated/zones/`
//! module provides properly typed response structs and request functions with
//! query-param serialization. We layer auth injection, body construction, and
//! result extraction on top.

use foundation_core::valtron::{sendables::sync_collect_one, Stream};

use crate::generated::shared::{ApiError, ApiPending, ApiResponse};
use crate::generated::zones::{
    dns_records_for_a_zone_create_dns_record_request,
    dns_records_for_a_zone_delete_dns_record_request,
    dns_records_for_a_zone_list_dns_records_request,
    dns_records_for_a_zone_update_dns_record_request,
    DnsRecordsDnsRecordResponse, DnsRecordsForAZoneCreateDnsRecordArgs,
    DnsRecordsForAZoneDeleteDnsRecordArgs, DnsRecordsForAZoneListDnsRecordsArgs,
    DnsRecordsForAZoneUpdateDnsRecordArgs,
};
use crate::types::{cf_err, CloudflareError, DnsRecord, DnsRecordInput, DnsRecordType};

/// Drive a valtron TaskIterator to completion, preserving HTTP status codes from
/// `ApiError::HttpStatus` for better error diagnostics.
fn drive_task<T: std::fmt::Debug>(
    task: impl Iterator<Item = Stream<Result<ApiResponse<T>, ApiError>, ApiPending>> + Send + 'static,
) -> Result<ApiResponse<T>, CloudflareError> {
    let result = sync_collect_one(task).ok_or_else(|| {
        cf_err(CloudflareError::Http("stream produced no result".into()))
    })?;
    result.map_err(|e| match e {
        ApiError::HttpStatus { code, body, .. } => {
            let detail = body
                .as_deref()
                .unwrap_or("(no body)");
            CloudflareError::Api {
                status: code,
                message: format!("HTTP {code}: {detail}"),
            }
        }
        other => CloudflareError::Api {
            status: 0,
            message: format!("{other:?}"),
        },
    })
}

/// Build an auth injection closure that adds `Authorization: Bearer <token>`.
fn auth_mod(
    token: &str,
) -> impl FnOnce(
    &mut foundation_netio::simple_http::client::ClientRequestBuilder<
        foundation_netio::simple_http::client::shared::SystemDnsResolver,
    >,
) + '_ {
    let t = token.to_string();
    move |b: &mut foundation_netio::simple_http::client::ClientRequestBuilder<_>| {
        b.header(SimpleHeader::AUTHORIZATION, format!("Bearer {t}"));
    }
}

/// Extract a `DnsRecord` from the auto-generated `DnsRecordsDnsRecordResponse`.
fn record_from_response(
    response: &DnsRecordsDnsRecordResponse,
) -> Result<DnsRecord, CloudflareError> {
    let value = serde_json::to_value(response)
        .map_err(|e| CloudflareError::Json(format!("serialize DnsRecordResponse: {e}")))?;
    serde_json::from_value(value)
        .map_err(|e| CloudflareError::Json(format!("deserialize DnsRecord: {e}")))
}

impl crate::client::CloudflareClient {
    /// List DNS records, optionally filtered by type and name.
    pub fn list_dns_records(
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

        let task = dns_records_for_a_zone_list_dns_records_request(
            self.http(),
            &args,
            Some(auth_mod(&self.token())),
        )
        .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

        let response = drive_task(task)?;
        let results = response.body.result.unwrap_or_default();
        let records: Result<Vec<_>, _> = results.iter().map(record_from_response).collect();
        records
    }

    /// Upsert a DNS record — find by name+type, update if exists, create if not.
    /// Returns the record ID.
    pub fn upsert_dns_record(
        &self,
        record: &DnsRecord,
    ) -> Result<String, CloudflareError> {
        let input = DnsRecordInput::from(record);

        // 1. Look for existing record with same name + type
        let existing = self.list_dns_records(Some(record.r#type), Some(&record.name))?;

        if let Some(existing_record) = existing.into_iter().next() {
            // Update (PUT)
            let args = DnsRecordsForAZoneUpdateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
                dns_record_id: existing_record.id.clone(),
            };

            let task = dns_records_for_a_zone_update_dns_record_request(
                self.http(),
                &args,
                // Inject auth + JSON body via builder_mod
                Some(|b: &mut foundation_netio::simple_http::client::ClientRequestBuilder<
                    foundation_netio::simple_http::client::shared::SystemDnsResolver,
                >| {
                    b.header(
                        SimpleHeader::AUTHORIZATION,
                        format!("Bearer {}", self.token()),
                    );
                    let _ = b.body_json(&input);
                }),
            )
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

            let _response = drive_task(task)?;
            Ok(existing_record.id)
        } else {
            // Create (POST)
            let args = DnsRecordsForAZoneCreateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
            };

            let task = dns_records_for_a_zone_create_dns_record_request(
                self.http(),
                &args,
                Some(|b: &mut foundation_netio::simple_http::client::ClientRequestBuilder<
                    foundation_netio::simple_http::client::shared::SystemDnsResolver,
                >| {
                    b.header(
                        SimpleHeader::AUTHORIZATION,
                        format!("Bearer {}", self.token()),
                    );
                    let _ = b.body_json(&input);
                }),
            )
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

            let response = drive_task(task)?;
            let result = response
                .body
                .result
                .as_ref()
                .ok_or_else(|| CloudflareError::Json("response missing 'result' field".into()))?;
            let created = record_from_response(result)?;
            Ok(created.id)
        }
    }

    /// Delete a DNS record by ID.
    pub fn delete_dns_record(&self, record_id: &str) -> Result<(), CloudflareError> {
        let args = DnsRecordsForAZoneDeleteDnsRecordArgs {
            zone_id: self.zone_id().to_string(),
            dns_record_id: record_id.to_string(),
        };

        let task = dns_records_for_a_zone_delete_dns_record_request(
            self.http(),
            &args,
            Some(auth_mod(&self.token())),
        )
        .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

        let _response = drive_task(task)?;
        Ok(())
    }

    /// Delete all DNS records matching name + type (used for ACME cleanup).
    pub fn delete_dns_records_by_name(
        &self,
        name: &str,
        record_type: DnsRecordType,
    ) -> Result<(), CloudflareError> {
        let records = self.list_dns_records(Some(record_type), Some(name))?;
        for record in records {
            self.delete_dns_record(&record.id)?;
        }
        Ok(())
    }

    /// Check whether a zone exists by probing the zone details endpoint.
    ///
    /// The generated zone response type is `()` (the schema wasn't resolved by
    /// the generator), so we can only check existence (200 vs 404), not extract
    /// typed `Zone` data. A full implementation would either fix the generator
    /// to resolve the zone response schema, or use the raw response body.
    pub fn find_zone(&self, _domain: &str) -> Result<bool, CloudflareError> {
        use crate::generated::zones::{zones_0_get_request, Zones0GetArgs};

        let args = Zones0GetArgs {
            zone_id: self.zone_id().to_string(),
        };
        let task = zones_0_get_request(
            self.http(),
            &args,
            Some(auth_mod(&self.token())),
        )
        .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

        match drive_task::<()>(task) {
            Ok(_) => Ok(true),
            Err(CloudflareError::Api { status: 404, .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Bootstrap domain — ensure wildcard A record exists.
    pub fn bootstrap_domain(&self, public_ip: &str) -> Result<(), CloudflareError> {
        let wildcard = format!("*.{}", self.domain());
        let record = DnsRecord {
            id: String::new(),
            zone_id: self.zone_id().to_string(),
            name: wildcard,
            r#type: DnsRecordType::A,
            content: public_ip.to_string(),
            ttl: 60,
            proxied: false,
            comment: None,
            tags: Vec::new(),
            created_on: chrono::Utc::now(),
            modified_on: chrono::Utc::now(),
        };
        self.upsert_dns_record(&record)?;
        Ok(())
    }
}
