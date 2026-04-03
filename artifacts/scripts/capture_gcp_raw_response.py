#!/usr/bin/env python3
"""
Capture raw HTTP response from GCP Discovery API with chunked encoding preserved.

This script captures the exact bytes sent by GCP over the wire, including:
- HTTP response headers
- Chunked transfer encoding markers (chunk sizes, CRLFs)
- Chunk data with any embedded CR bytes

Usage:
    python capture_gcp_raw_response.py [output_file]

Output files created:
- <output_file>.bin - Raw HTTP response (headers + body)
- <output_file>_body.bin - Body only (chunked data, no headers)
- <output_file>.txt - Human-readable analysis

The captured data can be used for regression testing of HTTP chunked parsing.
"""

import socket
import ssl
import sys
from pathlib import Path


def capture_raw_http_response(
    host: str = "www.googleapis.com",
    port: int = 443,
    path: str = "/discovery/v1/apis/compute/v1/rest",
    output_prefix: str = "gcp_raw_http_response",
) -> bytes:
    """
    Capture raw HTTP/1.1 response with chunked transfer encoding preserved.

    Args:
        host: Target hostname
        port: Target port (443 for HTTPS)
        path: URL path to fetch
        output_prefix: Prefix for output files

    Returns:
        Raw response bytes
    """
    # Create TLS connection
    context = ssl.create_default_context()
    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    ssock = context.wrap_socket(sock, server_hostname=host)

    print(f"Connecting to {host}:{port}...")
    ssock.connect((host, port))

    # Build raw HTTP/1.1 request
    request = b"GET " + path.encode() + b" HTTP/1.1\r\n"
    request += b"Host: " + host.encode() + b"\r\n"
    request += b"Accept: */*\r\n"
    request += b"Connection: close\r\n"
    request += b"\r\n"

    print(f"Sending request...")
    ssock.sendall(request)

    # Read raw response until connection closes
    print("Reading response...")
    response = b""
    total_bytes = 0

    while True:
        chunk = ssock.recv(8192)
        if not chunk:
            break
        response += chunk
        total_bytes += len(chunk)
        if total_bytes % (1024 * 1024) == 0:
            print(f"  Received {total_bytes // (1024 * 1024)} MB...")

    ssock.close()
    print(f"Capture complete: {len(response)} bytes")

    return response


def analyze_response(response: bytes) -> str:
    """Generate human-readable analysis of the captured response."""
    lines = []
    lines.append("=== TCP/HTTP Capture Analysis ===")
    lines.append(f"Total bytes: {len(response)}")
    lines.append("")

    # Find end of headers
    header_end = response.find(b"\r\n\r\n")
    if header_end > 0:
        headers = response[:header_end].decode("utf-8", errors="replace")
        body = response[header_end + 4:]

        lines.append("=== HTTP HEADERS ===")
        for line in headers.split("\r\n"):
            lines.append(line)
        lines.append("")

        # Check for chunked encoding
        is_chunked = "chunked" in headers.lower()
        lines.append(f"Transfer-Encoding: {'chunked' if is_chunked else 'identity'}")
        lines.append(f"Body size: {len(body)} bytes")
        lines.append("")

        # Byte analysis
        cr_count = sum(1 for b in body if b == 0x0D)
        lf_count = sum(1 for b in body if b == 0x0A)
        lines.append("=== BYTE ANALYSIS ===")
        lines.append(f"CR (0x0D) count: {cr_count}")
        lines.append(f"LF (0x0A) count: {lf_count}")
        lines.append("")

        # Show first chunk info
        if is_chunked and len(body) > 20:
            first_line_end = body.find(b"\r\n")
            if first_line_end > 0:
                chunk_size_line = body[:first_line_end]
                try:
                    chunk_size = int(chunk_size_line.decode(), 16)
                    lines.append("=== FIRST CHUNK ===")
                    lines.append(f"Chunk size: {chunk_size} bytes (0x{chunk_size:x})")
                    lines.append(f"First 100 bytes of chunk data:")
                    chunk_data = body[first_line_end + 2:first_line_end + 102]
                    lines.append("  " + " ".join(f"{b:02x}" for b in chunk_data))
                except (ValueError, UnicodeDecodeError):
                    pass

        # Find CR byte positions (first 10)
        if cr_count > 0:
            lines.append("")
            lines.append("=== CR BYTE POSITIONS (first 10) ===")
            cr_positions = [i for i, b in enumerate(body) if b == 0x0D][:10]
            for pos in cr_positions:
                context_start = max(0, pos - 20)
                context_end = min(len(body), pos + 20)
                context = body[context_start:context_end]
                lines.append(f"  CR at body offset {pos}: ...{context!r}...")

    else:
        lines.append("ERROR: Could not find end of HTTP headers")

    return "\n".join(lines)


def main():
    output_prefix = sys.argv[1] if len(sys.argv) > 1 else "gcp_raw_http_response"
    output_path = Path(output_prefix)

    # Capture raw response
    response = capture_raw_http_response(output_prefix=str(output_path))

    # Write raw response
    bin_path = output_path.with_suffix(".bin")
    with open(bin_path, "wb") as f:
        f.write(response)
    print(f"Raw response written to: {bin_path}")

    # Extract and write body only
    header_end = response.find(b"\r\n\r\n")
    if header_end > 0:
        body = response[header_end + 4:]
        body_path = output_path.with_name(output_path.stem + "_body.bin")
        with open(body_path, "wb") as f:
            f.write(body)
        print(f"Body only written to: {body_path} ({len(body)} bytes)")

    # Write analysis
    analysis = analyze_response(response)
    analysis_path = output_path.with_suffix(".txt")
    with open(analysis_path, "w") as f:
        f.write(analysis)
    print(f"Analysis written to: {analysis_path}")

    print("\nDone!")


if __name__ == "__main__":
    main()
