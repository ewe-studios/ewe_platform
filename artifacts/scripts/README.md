# Artifacts Scripts

This directory contains utility scripts used for development, testing, and debugging.

## Scripts

### capture_gcp_raw_response.py

Captures raw HTTP/1.1 responses from GCP Discovery API with chunked transfer encoding preserved.

**Purpose:** Generate test fixtures for HTTP chunked parsing regression tests.

**Usage:**
```bash
# Capture from default endpoint (GCP Compute API)
python3 artifacts/scripts/capture_gcp_raw_response.py

# Capture from custom endpoint
python3 artifacts/scripts/capture_gcp_raw_response.py custom_prefix

# Capture and save to specific location
python3 artifacts/scripts/capture_gcp_raw_response.py /path/to/output
```

**Output files:**
- `<prefix>.bin` - Full raw HTTP response (headers + chunked body)
- `<prefix>_body.bin` - Body only (chunked encoding preserved)
- `<prefix>.txt` - Human-readable analysis with hex dumps

**Why this exists:**

GCP Discovery API sends different API spec versions to different clients. The version sent to our client contains stray CR (`\r`, 0x0D) bytes embedded in JSON string values, which break JSON parsing.

Standard tools like `curl` automatically decode chunked transfer encoding, hiding the raw chunk structure. This script captures the exact bytes over the wire for:
- Regression testing of chunked HTTP parsing
- Debugging CR byte stripping logic
- Verifying protocol-level correctness

See `specifications/11-foundation-deployment/features/05-gcp-cloud-run-provider/CR_BYTE_INVESTIGATION.md` for full background.
