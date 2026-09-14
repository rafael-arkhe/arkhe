//! `OrcidClient` against a real local HTTP server (mockito), so the wire
//! contract — request path, `Accept` header, status mapping, body parsing —
//! is exercised rather than asserted about.

use arkhe_orcid::{OrcidClient, OrcidError, OrcidId};

/// The shape ORCID's `/person` endpoint returns, trimmed to the fields
/// this crate reads.
const PERSON_WITH_NAME: &str =
    r#"{"name":{"given-names":{"value":"Josiah"},"family-name":{"value":"Carberry"}}}"#;

fn carberry() -> OrcidId {
    OrcidId::parse("0000-0002-1825-0097").unwrap()
}

#[tokio::test]
async fn a_successful_response_is_parsed_into_a_verification() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .match_header("accept", "application/json")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(PERSON_WITH_NAME)
        .create_async()
        .await;

    let verification = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .expect("200 with a name verifies");

    assert_eq!(verification.display_name, "Josiah Carberry");
    assert_eq!(verification.id, carberry());
    assert_eq!(verification.given_names.as_deref(), Some("Josiah"));
    assert_eq!(verification.family_name.as_deref(), Some("Carberry"));

    // The mock only matches the exact path and Accept header, so reaching
    // the assertions above already proves the request shape; this pins it.
    mock.assert_async().await;
}

#[tokio::test]
async fn a_bare_digit_id_is_sent_in_canonical_hyphenated_form() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(200)
        .with_body(PERSON_WITH_NAME)
        .create_async()
        .await;

    // Parsed from the hyphen-less spelling, must travel hyphenated.
    let id = OrcidId::parse("0000000218250097").unwrap();
    OrcidClient::with_base_url(server.url())
        .verify(&id)
        .await
        .unwrap();

    mock.assert_async().await;
}

#[tokio::test]
async fn a_404_is_mapped_to_not_found() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(404)
        .create_async()
        .await;

    let err = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .unwrap_err();

    match err {
        OrcidError::NotFound { orcid } => assert_eq!(orcid, "0000-0002-1825-0097"),
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[tokio::test]
async fn a_429_is_mapped_to_rate_limited() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(429)
        .create_async()
        .await;

    let err = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .unwrap_err();

    assert!(matches!(err, OrcidError::RateLimited), "got {err:?}");
}

#[tokio::test]
async fn a_500_is_mapped_to_provider() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(500)
        .create_async()
        .await;

    let err = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .unwrap_err();

    match err {
        OrcidError::Provider { status } => assert_eq!(status, 500),
        other => panic!("expected Provider, got {other:?}"),
    }
}

#[tokio::test]
async fn a_403_is_mapped_to_unexpected_response() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(403)
        .create_async()
        .await;

    let err = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .unwrap_err();

    match err {
        OrcidError::UnexpectedResponse { reason } => assert!(reason.contains("403"), "{reason}"),
        other => panic!("expected UnexpectedResponse, got {other:?}"),
    }
}

#[tokio::test]
async fn malformed_json_is_an_unexpected_response() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body("not json")
        .create_async()
        .await;

    let err = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrcidError::UnexpectedResponse { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn a_record_without_a_name_is_an_unexpected_response() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name":null}"#)
        .create_async()
        .await;

    let err = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .unwrap_err();

    // A verification with a blank display name would be worse than an
    // error: it would attest turns to nobody while looking successful.
    assert!(
        matches!(err, OrcidError::UnexpectedResponse { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn a_record_with_a_null_name_field_is_an_unexpected_response() {
    let mut server = mockito::Server::new_async().await;
    let _mock = server
        .mock("GET", "/0000-0002-1825-0097/person")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"name":{"given-names":null,"family-name":null}}"#)
        .create_async()
        .await;

    let err = OrcidClient::with_base_url(server.url())
        .verify(&carberry())
        .await
        .unwrap_err();

    assert!(
        matches!(err, OrcidError::UnexpectedResponse { .. }),
        "got {err:?}"
    );
}

#[tokio::test]
async fn an_unreachable_server_is_a_transport_error() {
    // Port 1 on loopback: nothing listens there, so the request fails
    // before any HTTP response exists.
    let client = OrcidClient::with_base_url("http://127.0.0.1:1");
    let err = client.verify(&carberry()).await.unwrap_err();

    assert!(matches!(err, OrcidError::Transport(_)), "got {err:?}");
}

/// Not run by default: this hits the real ORCID API, so it needs network
/// access and is subject to ORCID's rate limits. Run explicitly with
/// `cargo test -p arkhe-orcid -- --ignored` when validating against the
/// live service.
#[tokio::test]
#[ignore = "hits the live ORCID public API"]
async fn live_verification_against_the_real_orcid_api() {
    let verification = OrcidClient::new()
        .verify(&carberry())
        .await
        .expect("the ORCID docs' example iD resolves");

    assert_eq!(verification.display_name, "Josiah Carberry");
    assert_eq!(verification.id, carberry());
}
