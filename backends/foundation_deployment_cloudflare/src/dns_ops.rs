//! High-level DNS record operations wrapping the auto-generated valtron functions.

use foundation_core::valtron::{sendables::sync_collect_one, Stream};
use foundation_netio::simple_http::shared::{SimpleHeader, SimpleHeaders};

use crate::shared::{ApiError, ApiPending, ApiResponse};
use crate::types::{cf_err, CloudflareError, DnsRecord, DnsRecordType};
use crate::zones::{
    dns_records_for_a_zone_create_dns_record_request,
    dns_records_for_a_zone_delete_dns_record_request,
    dns_records_for_a_zone_list_dns_records_request,
    dns_records_for_a_zone_patch_dns_record_request,
    dns_records_for_a_zone_update_dns_record_request,
    DnsRecordsDnsRecordPost, DnsRecordsDnsResponseCollection, DnsRecordsDnsResponseSingle,
    DnsRecordsForAZoneCreateDnsRecordArgs, DnsRecordsForAZoneDeleteDnsRecordArgs,
    DnsRecordsForAZoneListDnsRecordsArgs, DnsRecordsForAZonePatchDnsRecordArgs,
    DnsRecordsForAZoneUpdateDnsRecordArgs,
};

/// Drive a valtron TaskIterator to completion and extract the typed value.
fn drive_task<T: std::fmt::Debug>(
    task: impl Iterator<Item = Stream<Result<ApiResponse<T>, ApiError>, ApiPending>> + Send + 'static,
) -> Result<ApiResponse<T>, CloudflareError> {
    let result = sync_collect_one(task).ok_or_else(|| {
        cf_err(CloudflareError::Http("stream produced no result".into()))
    })?;
    result.map_err(|e| cf_err(CloudflareError::Api {
        status: 0,
        message: format!("{e:?}"),
    }))
}

/// Build an auth injection closure for the auto-generated request builders.
fn auth_mod(token: &str) -> impl FnOnce(&mut foundation_netio::simple_http::client::ClientRequestBuilder<foundation_netio::simple_http::client::shared::SystemDnsResolver>) + '_ {
    let token = token.to_string();
    move |b: &mut foundation_netio::simple_http::client::ClientRequestBuilder<_>| {
        b.header(SimpleHeader::AUTHORIZATION, format!("Bearer {token}"));
    }
}

impl crate::client::CloudflareClient {
    /// List DNS records, optionally filtered by type and name.
    pub fn list_dns_records(
        &self,
        r#type: Option<DnsRecordType>,
        name: Option<&str>,
    ) -> Result<Vec<DnsRecord>, CloudflareError> {
        // Query params are now serialised automatically by the regenerated
        // request function — no manual URL building needed.
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
        // Cloudflare API wraps everything in { success, errors, messages, result }.
        // The auto-generated type flattens this into a HashMap.  Extract "result"
        // and deserialize as Vec<DnsRecord>.
        let result_value = response.body.data.get("result").cloned().ok_or_else(|| {
            cf_err(CloudflareError::Json("response missing 'result' field".into()))
        })?;
        let records: Vec<DnsRecord> = serde_json::from_value(result_value)
            .map_err(|e| cf_err(CloudflareError::Json(format!("deserialize DnsRecord list: {e}"))))?;
        Ok(records)
    }

    /// Upsert a DNS record — find by name+type, update if exists, create if not.
    pub fn upsert_dns_record(
        &self,
        record: &DnsRecord,
    ) -> Result<String, CloudflareError> {
        // 1. Look for existing record with same name + type
        let existing = self.list_dns_records(Some(record.r#type), Some(&record.name))?;

        let body = record_to_body(record);

        if let Some(existing_record) = existing.into_iter().next() {
            // Update (PUT)
            let args = DnsRecordsForAZoneUpdateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
                dns_record_id: existing_record.id.clone(),
                body,
            };

            let task = dns_records_for_a_zone_update_dns_record_request(
                self.http(),
                &args,
                Some(auth_mod(&self.token())),
            )
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

            let _response = drive_task(task)?;
            Ok(existing_record.id)
        } else {
            // Create (POST)
            let args = DnsRecordsForAZoneCreateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
                body,
            };

            let task = dns_records_for_a_zone_create_dns_record_request(
                self.http(),
                &args,
                Some(auth_mod(&self.token())),
            )
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

            let response = drive_task(task)?;
            // Extract the created record from `result`
            let result_value = response.body.data.get("result").cloned().ok_or_else(|| {
                cf_err(CloudflareError::Json("response missing 'result' field".into()))
            })?;
            let created: DnsRecord = serde_json::from_value(result_value)
                .map_err(|e| cf_err(CloudflareError::Json(format!("deserialize created record: {e}"))))?;
            Ok(created.id)
        }
    }
}

/// Build a body HashMap from a DnsRecord.  Uses serde serialization so the
/// body automatically matches whatever fields `DnsRecord` declares — no
/// manual field-by-field construction.
fn record_to_body(record: &DnsRecord) -> DnsRecordsDnsRecordPost {
    let value = serde_json::to_value(record).unwrap_or_default();
    let data = match value {
        serde_json::Value::Object(map) => map.into_iter().collect(),
        _ => std::collections::HashMap::new(),
    };
    DnsRecordsDnsRecordPost { data }
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
