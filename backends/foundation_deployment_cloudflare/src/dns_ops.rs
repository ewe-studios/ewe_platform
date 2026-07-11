//! High-level DNS record operations wrapping the auto-generated valtron functions.

use foundation_netio::simple_http::shared::SimpleHeader;

use crate::generated::shared::{ApiError, ApiPending, ApiResponse};
use crate::generated::zones::{
    dns_records_for_a_zone_create_dns_record_request,
    dns_records_for_a_zone_delete_dns_record_request,
    dns_records_for_a_zone_list_dns_records_request,
    dns_records_for_a_zone_update_dns_record_request, DnsRecordsDnsResponseSingle,
    DnsRecordsForAZoneCreateDnsRecordArgs, DnsRecordsForAZoneDeleteDnsRecordArgs,
    DnsRecordsForAZoneListDnsRecordsArgs, DnsRecordsForAZoneUpdateDnsRecordArgs,
};
use crate::types::{cf_err, CloudflareError, DnsRecord, DnsRecordInput, DnsRecordType};

fn map_api_error(e: ApiError) -> CloudflareError {
    match e {
        ApiError::HttpStatus { code, body, .. } => CloudflareError::Api {
            status: code,
            message: body.as_deref().unwrap_or("(no body)").to_string(),
        },
        other => CloudflareError::Api { status: 0, message: format!("{other:?}") },
    }
}

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

impl crate::client::CloudflareClient {
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
        let task = dns_records_for_a_zone_list_dns_records_request(self.http(), &args, Some(auth_mod(&self.token())))
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;
        let response = task.collect_one()
            .ok_or_else(|| CloudflareError::Http("no result".into()))?
            .map_err(map_api_error)?;
        let results = response.body.result.unwrap_or_default();
        results.iter().map(|entry| {
            let v = serde_json::to_value(entry).unwrap_or_default();
            serde_json::from_value(v)
                .map_err(|e| CloudflareError::Json(format!("deserialize DnsRecord: {e}")))
        }).collect()
    }

    pub fn upsert_dns_record(&self, record: &DnsRecord) -> Result<String, CloudflareError> {
        let input = DnsRecordInput::from(record);
        let existing = self.list_dns_records(Some(record.r#type), Some(&record.name))?;

        if let Some(existing_record) = existing.into_iter().next() {
            let args = DnsRecordsForAZoneUpdateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
                dns_record_id: existing_record.id.clone(),
            };
            let task = dns_records_for_a_zone_update_dns_record_request(
                self.http(), &args,
                Some(|b: &mut foundation_netio::simple_http::client::ClientRequestBuilder<_>| {
                    b.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", self.token()));
                    let _ = b.body_json(&input);
                }),
            )
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;
            task.collect_one()
                .ok_or_else(|| CloudflareError::Http("no result".into()))?
                .map_err(map_api_error)?;
            Ok(existing_record.id)
        } else {
            let args = DnsRecordsForAZoneCreateDnsRecordArgs {
                zone_id: self.zone_id().to_string(),
            };
            let task = dns_records_for_a_zone_create_dns_record_request(
                self.http(), &args,
                Some(|b: &mut foundation_netio::simple_http::client::ClientRequestBuilder<_>| {
                    b.header(SimpleHeader::AUTHORIZATION, format!("Bearer {}", self.token()));
                    let _ = b.body_json(&input);
                }),
            )
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;
            let response: ApiResponse<DnsRecordsDnsResponseSingle> = task.collect_one()
                .ok_or_else(|| CloudflareError::Http("no result".into()))?
                .map_err(map_api_error)?;
            let entry = response.body.result.as_ref()
                .ok_or_else(|| CloudflareError::Json("response missing 'result' field".into()))?;
            let v = serde_json::to_value(entry).unwrap_or_default();
            let created: DnsRecord = serde_json::from_value(v)
                .map_err(|e| CloudflareError::Json(format!("deserialize DnsRecord: {e}")))?;
            Ok(created.id)
        }
    }

    pub fn delete_dns_record(&self, record_id: &str) -> Result<(), CloudflareError> {
        let args = DnsRecordsForAZoneDeleteDnsRecordArgs {
            zone_id: self.zone_id().to_string(),
            dns_record_id: record_id.to_string(),
        };
        let task = dns_records_for_a_zone_delete_dns_record_request(self.http(), &args, Some(auth_mod(&self.token())))
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;
        task.collect_one()
            .ok_or_else(|| CloudflareError::Http("no result".into()))?
            .map_err(map_api_error)?;
        Ok(())
    }

    pub fn delete_dns_records_by_name(
        &self, name: &str, record_type: DnsRecordType,
    ) -> Result<(), CloudflareError> {
        for record in self.list_dns_records(Some(record_type), Some(name))? {
            self.delete_dns_record(&record.id)?;
        }
        Ok(())
    }

    pub fn find_zone(&self, _domain: &str) -> Result<bool, CloudflareError> {
        use crate::generated::zones::{zones_0_get_request, Zones0GetArgs};
        let args = Zones0GetArgs { zone_id: self.zone_id().to_string() };
        let task = zones_0_get_request(self.http(), &args, Some(auth_mod(&self.token())))
            .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;
        match task.collect_one() {
            Some(Ok(_)) => Ok(true),
            Some(Err(ApiError::HttpStatus { code: 404, .. })) => Ok(false),
            Some(Err(e)) => Err(map_api_error(e)),
            None => Err(CloudflareError::Http("no result".into())),
        }
    }

    pub fn bootstrap_domain(&self, public_ip: &str) -> Result<(), CloudflareError> {
        let wildcard = format!("*.{}", self.domain());
        let record = DnsRecord {
            id: String::new(), zone_id: self.zone_id().to_string(), name: wildcard,
            r#type: DnsRecordType::A, content: public_ip.to_string(), ttl: 60,
            proxied: false, comment: None, tags: vec![],
            created_on: chrono::Utc::now(), modified_on: chrono::Utc::now(),
        };
        self.upsert_dns_record(&record)?;
        Ok(())
    }
}
