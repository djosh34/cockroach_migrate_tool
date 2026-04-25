#[path = "support/tls_reference_surface.rs"]
mod tls_reference_surface_support;

use tls_reference_surface_support::TlsReferenceSurface;

#[test]
fn tls_reference_doc_exists_with_path_guidance_and_readme_link() {
    let surface = TlsReferenceSurface::load();
    let doc = surface.doc();
    let readme = surface.readme();

    assert!(
        doc.starts_with("# TLS Configuration Reference"),
        "TLS reference doc should start with the canonical title",
    );
    assert!(
        doc.contains("/config/certs/"),
        "TLS reference doc should recommend `/config/certs/...` paths for mounted PEM files",
    );
    assert!(
        readme.contains("docs/tls-configuration.md"),
        "README should link operators to the dedicated TLS reference doc",
    );
}

#[test]
fn tls_reference_doc_owns_the_mapping_table_and_mode_explanations() {
    let doc = TlsReferenceSurface::load().doc().to_owned();

    for required_snippet in [
        "## Component-to-Field Mapping",
        "| Component | TLS-relevant fields |",
        "| Runner webhook | `mode` (`http` or `https`), `tls.cert_path`, `tls.key_path` |",
        "| Runner destination | `tls.mode`, `tls.ca_cert_path`, `tls.client_cert_path`, `tls.client_key_path` |",
        "| Verify listener | `tls.cert_path`, `tls.key_path`, `tls.client_ca_path` (optional for mTLS) |",
        "| Verify source and destination | `url` with `sslmode`, `tls.ca_cert_path`, `tls.client_cert_path`, `tls.client_key_path` |",
        "## TLS Modes",
        "- `http` or no TLS: plain text. Use only for local development.",
        "- `https`: the server presents a certificate and clients verify it before sending data.",
        "- `mTLS`: both sides present certificates and both sides verify who is on the other end.",
        "- `require`: TLS is enabled, but the client does not verify the server certificate.",
        "- `verify-ca`: TLS is enabled and the client verifies the server certificate against a trusted CA.",
        "- `verify-full`: TLS is enabled and the client verifies both the server certificate and the hostname.",
    ] {
        assert!(
            doc.contains(required_snippet),
            "TLS reference doc should own the operator-facing TLS contract snippet `{required_snippet}`",
        );
    }

    for forbidden_snippet in [
        "openssl",
        "Let's Encrypt",
        "Kubernetes",
        "Ingress",
        "rustls",
        "Go TLS",
        "cipher suite",
        "TLS version",
    ] {
        assert!(
            !doc.contains(forbidden_snippet),
            "TLS reference doc must stay operator-facing and exclude `{forbidden_snippet}`",
        );
    }
}

#[test]
fn tls_reference_doc_covers_runner_tls_scenarios_and_payload_cross_reference() {
    let doc = TlsReferenceSurface::load().doc().to_owned();

    for required_snippet in [
        "## Common Scenarios",
        "### Runner webhook HTTP (local development)",
        "mode: http",
        "### Runner webhook HTTPS (production)",
        "mode: https",
        "cert_path: /config/certs/server.crt",
        "key_path: /config/certs/server.key",
        "### Runner destination with `verify-ca`",
        "sslmode=verify-ca",
        "sslrootcert=/config/certs/destination-ca.crt",
        "### Runner destination with `verify-full` and client certificates",
        "mode: verify-full",
        "client_cert_path: /config/certs/destination-client.crt",
        "client_key_path: /config/certs/destination-client.key",
        "For the webhook payload shape, see `README.md#webhook-payload-format`.",
    ] {
        assert!(
            doc.contains(required_snippet),
            "TLS reference doc should cover the runner-side snippet `{required_snippet}`",
        );
    }
}

#[test]
fn tls_reference_doc_covers_verify_tls_scenarios_and_openapi_cross_reference() {
    let doc = TlsReferenceSurface::load().doc().to_owned();

    for required_snippet in [
        "### Verify listener HTTPS",
        "listener:",
        "cert_path: /config/certs/server.crt",
        "key_path: /config/certs/server.key",
        "### Verify listener mTLS",
        "client_ca_path: /config/certs/client-ca.crt",
        "### Verify DB connection with `sslmode=verify-full`",
        "url: postgresql://verify_source@source.internal:5432/appdb?sslmode=verify-full",
        "ca_cert_path: /config/certs/source-ca.crt",
        "client_cert_path: /config/certs/source-client.crt",
        "client_key_path: /config/certs/source-client.key",
        "For verify API endpoints, see `openapi/verify-service.yaml`.",
    ] {
        assert!(
            doc.contains(required_snippet),
            "TLS reference doc should cover the verify-side snippet `{required_snippet}`",
        );
    }
}
