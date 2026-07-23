//! Integration coverage for [`XlmNsClient::batch_resolve`].
//!
//! These exercise the batch API through the crate's public surface only — the
//! types a downstream integrator actually imports — and run as part of the
//! default `cargo test`. The live-node counterpart is
//! `batch_resolve_against_local_node` in `local_soroban.rs`.

use xlm_ns_sdk::{BatchResolveError, BatchResult, ClientConfig, XlmNsBlockingClient, XlmNsClient};

const REGISTRY_ID: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
const RESOLVER_ID: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

fn client(chunk_size: usize) -> XlmNsClient {
    XlmNsClient::builder("http://localhost")
        .registry(REGISTRY_ID)
        .resolver(RESOLVER_ID)
        .config(
            ClientConfig::default()
                .with_batch_chunk_size(chunk_size)
                .with_max_retries(0),
        )
        .build()
}

fn names(count: usize) -> Vec<String> {
    (0..count).map(|i| format!("name{i}.xlm")).collect()
}

#[tokio::test]
async fn resolves_a_batch_and_returns_results_in_order() {
    let input = vec![
        "alice.xlm".to_string(),
        "bob.xlm".to_string(),
        "carol.xlm".to_string(),
    ];

    let results = client(50).batch_resolve(input.clone()).await.unwrap();

    assert_eq!(results.len(), input.len());
    for (result, expected) in results.iter().zip(&input) {
        assert_eq!(&result.name, expected);
        let address = result.as_result().expect("name should resolve");
        assert!(!address.is_empty());
        assert!(result.ttl_seconds.is_some(), "TTL should be reported");
    }
}

#[tokio::test]
async fn one_bad_name_does_not_fail_the_batch() {
    let results = client(50)
        .batch_resolve(vec![
            "alice.xlm".to_string(),
            "expired.xlm".to_string(),
            "bob.xlm".to_string(),
        ])
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    assert!(results[0].is_ok());
    assert!(results[2].is_ok());

    match results[1].as_result() {
        Err(BatchResolveError::Expired { expired_at }) => assert!(*expired_at > 0),
        other => panic!("expected the expired name to report Expired, got {other:?}"),
    }
}

#[tokio::test]
async fn a_large_batch_is_chunked_transparently() {
    // 250 names at the default chunk size, and the same batch at a chunk size
    // of 7, must return identical results — chunking is invisible to callers.
    let input = names(250);

    let default_chunking = client(usize::from(u8::MAX))
        .batch_resolve(input.clone())
        .await
        .unwrap();
    let small_chunking = client(7).batch_resolve(input.clone()).await.unwrap();

    assert_eq!(default_chunking.len(), input.len());
    assert_eq!(default_chunking, small_chunking);

    for (index, result) in small_chunking.iter().enumerate() {
        assert_eq!(result.name, format!("name{index}.xlm"));
        assert!(result.is_ok(), "chunked result should resolve: {result:?}");
    }
}

#[tokio::test]
async fn a_batch_mixing_every_failure_mode_reports_each_one() {
    let results = client(3)
        .batch_resolve(vec![
            "alice.xlm".to_string(),
            "notfound.xlm".to_string(),
            "expired.xlm".to_string(),
            "noaddress.xlm".to_string(),
            "not-a-name".to_string(),
            "bob.xlm".to_string(),
        ])
        .await
        .unwrap();

    let summary: Vec<bool> = results.iter().map(BatchResult::is_ok).collect();
    assert_eq!(summary, [true, false, false, false, false, true]);

    assert!(matches!(
        results[1].error,
        Some(BatchResolveError::NotFound)
    ));
    assert!(matches!(
        results[2].error,
        Some(BatchResolveError::Expired { .. })
    ));
    assert!(matches!(
        results[3].error,
        Some(BatchResolveError::NoAddress)
    ));
    assert!(matches!(
        results[4].error,
        Some(BatchResolveError::InvalidName { .. })
    ));

    // Errors render for logs without the caller matching on the variant.
    let rendered = results[1].error.as_ref().unwrap().to_string();
    assert!(!rendered.is_empty());
}

#[tokio::test]
async fn an_unconfigured_resolver_fails_the_request() {
    let client = XlmNsClient::builder("http://localhost")
        .registry(REGISTRY_ID)
        .build();

    assert!(client
        .batch_resolve(vec!["alice.xlm".to_string()])
        .await
        .is_err());
}

#[test]
fn the_blocking_client_exposes_the_same_batch_api() {
    let client = XlmNsBlockingClient::from_async(client(4)).unwrap();

    let results = client.batch_resolve(names(10)).unwrap();

    assert_eq!(results.len(), 10);
    assert!(results.iter().all(BatchResult::is_ok));
}
