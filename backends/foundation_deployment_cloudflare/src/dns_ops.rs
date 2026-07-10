//! High-level DNS record operations wrapping the auto-generated valtron functions.

use foundation_core::valtron::{sendables::sync_collect_one, Stream};
use foundation_netio::simple_http::client::native::SimpleHttpClient;
use foundation_netio::simple_http::client::ClientRequestBuilder;
use foundation_netio::simple_http::shared::DnsResolver;

use crate::shared::{ApiError, ApiPending};
use crate::types::{cf_err, CloudflareError, DnsRecord, DnsRecordType};

/// Drive a valtron TaskIterator to completion, extract the Ready value.
fn drive_task<T: std::fmt::Debug>(
    task: impl Iterator<Item = Stream<Result<T, ApiError>, ApiPending>> + Send + 'static,
) -> Result<T, CloudflareError> {
    let result = sync_collect_one(task).ok_or_else(|| {
        cf_err(CloudflareError::Http("stream produced no result".into()))
    })?;

    result.map_err(|e| cf_err(CloudflareError::Api {
        status: 0,
        message: format!("{e:?}"),
    }))
}

/// Auth injection closure for the auto-generated request builders.
struct AuthInjector<'a>(&'a str);

impl<'a, R: DnsResolver + Clone + Default + 'static> FnOnce(&mut ClientRequestBuilder<R>) for AuthInjector<'a> {
    type Output = ();
    extern "rust-call" fn call_once(self, builder: &mut ClientRequestBuilder<R>) {
        // builder.header("Authorization", format!("Bearer {}", self.0));
        let _ = (builder, self.0);
    }
}

impl crate::client::CloudflareClient {
    /// List DNS records, optionally filtered by type and name.
    pub fn list_dns_records(
        &self,
        r#type: Option<DnsRecordType>,
        name: Option<&str>,
    ) -> Result<Vec<DnsRecord>, CloudflareError> {
        use crate::zones::{
            dns_records_for_a_zone_list_dns_records_request,
            DnsRecordsForAZoneListDnsRecordsArgs,
        };

        let args = DnsRecordsForAZoneListDnsRecordsArgs {
            zone_id: self.zone_id().to_string(),
            r#type: r#type.map(|t| t.as_str().to_string()),
            name: name.map(|n| n.to_string()),
            ..Default::default()
        };

        let http = self.http().clone();
        let token = std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default();

        let task = dns_records_for_a_zone_list_dns_records_request(
            &http,
            &args,
            Some(|b: &mut ClientRequestBuilder<_>| {
                // Auth header
                let _ = (b, token);
            }),
        )
        .map_err(|e| cf_err(CloudflareError::Http(format!("{e:?}"))))?;

        let _response = drive_task(task)?;
        // TODO: convert HashMap response to Vec<DnsRecord>
        Ok(Vec::new())
    }

    /// Upsert a DNS record — create if not exists, update if it does.
    pub fn upsert_dns_record(
        &self,
        _record: &DnsRecord,
    ) -> Result<String, CloudflareError> {
        // 1. List: find existing record by name + type
        // 2. If exists → update (PUT). If not → create (POST)
        // 3. Return record ID
        Err(cf_err(CloudflareError::Api {
            status: 0,
            message: "upsert_dns_record not yet implemented".into(),
        }))
    }
}
