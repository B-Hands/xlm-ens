#![allow(
    dead_code,
    unused_variables,
    clippy::module_inception,
    clippy::single_match,
    clippy::duplicated_attributes
)]
#![allow(
    dead_code,
    unused_variables,
    clippy::module_inception,
    clippy::single_match,
    clippy::duplicated_attributes
)]
#![allow(
    dead_code,
    unused_variables,
    clippy::module_inception,
    clippy::single_match
)]
#![allow(
    dead_code,
    unused_variables,
    clippy::module_inception,
    clippy::single_match
)]
#[cfg(test)]
mod tests {
    use crate::client::XlmNsClient;
    use crate::errors::SdkError;
    use crate::network;
    use crate::types::{
        RegistrationRequest, RenewalRequest, SubmissionStatus, TextRecordUpdate, TextRecordsUpdate,
        TransferRequest,
    };
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use stellar_rpc_client::Client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

    /// A wiremock responder that echoes the JSON-RPC request ID
    /// from the incoming request, so jsonrpsee can match it.
    struct JsonRpcResponder {
        result: serde_json::Value,
    }
    impl JsonRpcResponder {
        fn new(result: serde_json::Value) -> Self {
            Self { result }
        }
    }
    impl Respond for JsonRpcResponder {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap_or_default();
            let id = body.get("id").cloned().unwrap_or(serde_json::json!(1));
            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": self.result,
            }))
        }
    }

    /// Returns HTTP errors for the first N requests, then a JSON-RPC success body.
    struct FailThenSucceed {
        failures_before_success: Arc<AtomicUsize>,
        success_body: serde_json::Value,
    }

    impl Respond for FailThenSucceed {
        fn respond(&self, request: &Request) -> ResponseTemplate {
            let remaining = self.failures_before_success.fetch_sub(1, Ordering::SeqCst);
            if remaining > 0 {
                ResponseTemplate::new(503)
            } else {
                JsonRpcResponder::new(self.success_body.clone()).respond(request)
            }
        }
    }

    fn retry_test_client(
        rpc_url: impl Into<String>,
        config: crate::config::ClientConfig,
    ) -> XlmNsClient {
        XlmNsClient::builder(rpc_url)
            .network_passphrase("Test SDF Network ; September 2015")
            .registry(REGISTRY_ID)
            .registrar(REGISTRAR_ID)
            .config(config)
            .build()
    }

    fn network_success_body() -> serde_json::Value {
        serde_json::json!({
            "passphrase": "Test SDF Network ; September 2015",
            "protocolVersion": 21
        })
    }

    // Valid 56-char Stellar contract IDs (C-prefix, all alphanumeric).
    const REGISTRY_ID: &str = "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const REGISTRAR_ID: &str = "CBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    const RESOLVER_ID: &str = "CCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";
    const AUCTION_ID: &str = "CDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDDD";
    const BRIDGE_ID: &str = "CEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE";
    const SUBDOMAIN_ID: &str = "CFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF";
    const NFT_ID: &str = "CGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG";
    // Valid 56-char Stellar account addresses (G-prefix, all alphanumeric).
    const OWNER_ADDR: &str = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    const NEW_OWNER_ADDR: &str = "GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB";
    const LOOKUP_ADDR: &str = "GCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCCC";

    fn client() -> XlmNsClient {
        XlmNsClient::builder("http://localhost")
            .network_passphrase("Test SDF Network ; September 2015")
            .registry(REGISTRY_ID)
            .subdomain(SUBDOMAIN_ID)
            .bridge(BRIDGE_ID)
            .auction(AUCTION_ID)
            .registrar(REGISTRAR_ID)
            .resolver(RESOLVER_ID)
            .build()
    }

    #[tokio::test]
    async fn renewal_returns_rich_receipt() {
        let receipt = client()
            .renew(RenewalRequest {
                name: "test.xlm".into(),
                additional_years: 2,
                signer: Some("alice".into()),
            })
            .await
            .unwrap();

        assert_eq!(receipt.fee_paid, 21);
        assert_eq!(receipt.additional_years, 2);
        assert_eq!(receipt.submission.status, SubmissionStatus::Submitted);
        assert_eq!(receipt.submission.signer.as_deref(), Some("alice"));
        assert!(receipt.new_expiry > 1_682_200_000);
    }

    #[tokio::test]
    async fn registration_quote_exposes_breakdown() {
        // "alpha" = 5 chars → 250_000_000 stroops/year (contract tier: 4–6 chars)
        let quote = client().quote_registration("alpha", 3).await.unwrap();
        assert_eq!(quote.label, "alpha");
        assert_eq!(quote.duration_years, 3);
        assert_eq!(quote.fee_breakdown.base_fee, 750_000_000); // 250_000_000 × 3
        assert_eq!(quote.fee_breakdown.premium_fee, 0);
        assert_eq!(quote.fee_breakdown.network_fee, 0);
        assert_eq!(quote.total_fee, 750_000_000);
        assert_eq!(quote.fee_currency, "XLM");
        assert!(quote.contract_id.is_some());
        assert!(quote.expires_at > quote.quoted_at);
        assert!(quote.grace_period_ends_at > quote.expires_at);
    }

    #[tokio::test]
    async fn registration_receipt_carries_submission_metadata() {
        // "beta" = 4 chars → 250_000_000 stroops/year (contract tier: 4–6 chars)
        let receipt = client()
            .register(RegistrationRequest {
                label: "beta".into(),
                owner: OWNER_ADDR.into(),
                duration_years: 1,
                signer: Some("treasury".into()),
            })
            .await
            .unwrap();

        assert_eq!(receipt.name, "beta.xlm");
        assert_eq!(receipt.duration_years, 1);
        assert_eq!(receipt.fee_paid, 250_000_000); // 250_000_000 × 1
        assert_eq!(receipt.submission.signer.as_deref(), Some("treasury"));
        assert!(receipt.submission.network_passphrase.is_some());
    }

    #[tokio::test]
    async fn reverse_resolution_rejects_empty_address() {
        assert!(client().reverse_resolve("").await.is_err());
    }

    #[tokio::test]
    async fn text_record_round_trip() {
        let client = client();
        let record = client.get_text_record("foo.xlm", "url").await.unwrap();
        assert_eq!(record.name, "foo.xlm");
        assert_eq!(record.key, "url");

        let submission = client
            .set_text_record(TextRecordUpdate {
                name: "foo.xlm".into(),
                key: "url".into(),
                value: Some("https://example.xyz".into()),
                signer: Some("owner".into()),
            })
            .await
            .unwrap();
        assert_eq!(submission.status, SubmissionStatus::Submitted);
        assert_eq!(submission.signer.as_deref(), Some("owner"));
    }

    #[tokio::test]
    async fn text_records_batch_update() {
        let client = client();
        let mut records = HashMap::new();
        records.insert("url".to_string(), Some("https://example.xyz".to_string()));
        records.insert("avatar".to_string(), None);

        let submission = client
            .set_text_records(TextRecordsUpdate {
                name: "foo.xlm".into(),
                records,
                signer: Some("owner".into()),
            })
            .await
            .unwrap();
        assert_eq!(submission.status, SubmissionStatus::Submitted);
        assert_eq!(submission.signer.as_deref(), Some("owner"));
    }

    #[tokio::test]
    async fn transfer_returns_submission() {
        let submission = client()
            .transfer(TransferRequest {
                name: "foo.xlm".into(),
                new_owner: NEW_OWNER_ADDR.into(),
                signer: Some("alice".into()),
            })
            .await
            .unwrap();
        assert_eq!(submission.status, SubmissionStatus::Submitted);
        assert_eq!(submission.signer.as_deref(), Some("alice"));
    }

    #[tokio::test]
    async fn registry_metadata_returns_typed_record() {
        let metadata = client().get_registry_metadata("alice.xlm").await.unwrap();
        assert_eq!(metadata.owner, "GDRA...OWNER");
        assert!(metadata.expires_at > 0);
        assert!(metadata.resolver.is_some());
    }

    #[tokio::test]
    async fn owner_portfolio_returns_vec() {
        let portfolio = client().get_owner_portfolio(OWNER_ADDR).await.unwrap();
        assert!(!portfolio.is_empty());
        assert_eq!(portfolio[0].owner, OWNER_ADDR);
    }

    #[test]
    fn owner_portfolio_page_returns_cursor_and_total() {
        let first = client()
            .list_registrations_by_owner_page(OWNER_ADDR, None, 1)
            .unwrap();
        assert_eq!(first.items.len(), 1);
        assert_eq!(first.total, 2);
        assert_eq!(first.next_cursor, Some(1));

        let second = client()
            .list_registrations_by_owner_page(OWNER_ADDR, first.next_cursor, 1)
            .unwrap();
        assert_eq!(second.items.len(), 1);
        assert_eq!(second.total, 2);
        assert_eq!(second.next_cursor, None);
    }

    #[tokio::test]
    async fn auction_state_returns_typed_data() {
        let state = client().get_auction_state("active.xlm").await.unwrap();
        assert_eq!(state.highest_bid, 150);
        assert!(state.end_time > 0);
    }

    #[tokio::test]
    async fn auction_state_handles_not_found() {
        use crate::errors::ContractErrorCode;
        use crate::errors::SdkError;
        let result = client().get_auction_state("missing.xlm").await;
        match result {
            Err(SdkError::ContractError(ContractErrorCode::NameNotFound)) => {}
            _ => panic!("Expected NameNotFound error"),
        }
    }

    #[tokio::test]
    async fn resolver_primary_name_returns_option() {
        let name = client().get_primary_name(LOOKUP_ADDR).await.unwrap();
        assert_eq!(name, Some("primary.xlm".to_string()));
    }

    #[tokio::test]
    async fn resolver_text_records_returns_hashmap() {
        let records = client().get_text_records("alice.xlm").await.unwrap();
        assert!(records.contains_key("url"));
        assert_eq!(records.get("url").unwrap(), "https://alice.xlm");
    }

    #[tokio::test]
    async fn builder_default_config_is_applied() {
        let client = client();
        assert_eq!(client.config.timeout, crate::config::DEFAULT_TIMEOUT);
        assert!(client.config.user_agent.starts_with("xlm-ns-sdk/"));
    }

    #[tokio::test]
    async fn builder_accepts_custom_config() {
        use crate::config::ClientConfig;
        use std::time::Duration;

        let client = XlmNsClient::builder("http://localhost")
            .registry(REGISTRY_ID)
            .config(
                ClientConfig::default()
                    .with_timeout(Duration::from_secs(2))
                    .with_max_retries(0)
                    .with_user_agent("integration-test/1.0"),
            )
            .build();

        assert_eq!(client.config.timeout, Duration::from_secs(2));
        assert_eq!(client.config.retry.max_retries, 0);
        assert_eq!(client.config.user_agent, "integration-test/1.0");
    }

    #[test]
    fn error_decoding_works() {
        use crate::errors::decode_error_generic;
        use crate::errors::ContractErrorCode;
        assert_eq!(decode_error_generic(1), ContractErrorCode::NameNotFound);
        assert_eq!(decode_error_generic(2), ContractErrorCode::NotOwner);
        assert_eq!(decode_error_generic(99), ContractErrorCode::Other(99));
    }

    #[tokio::test]
    async fn test_verify_passphrase_happy_path() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(JsonRpcResponder::new(serde_json::json!({
                "passphrase": "Test SDF Network ; September 2015",
                "protocolVersion": 21
            })))
            .mount(&mock_server)
            .await;
        let http_client = Client::new(&mock_server.uri()).unwrap();

        let result = network::verify_network_passphrase(
            "Test SDF Network ; September 2015",
            &mock_server.uri(),
            &http_client,
        )
        .await;

        assert!(result.is_ok(), "expected Ok but got: {:?}", result);
    }

    #[tokio::test]
    async fn test_verify_passphrase_mismatch_returns_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(JsonRpcResponder::new(serde_json::json!({
                "passphrase": "Public Global Stellar Network ; September 2015",
                "protocolVersion": 21
            })))
            .mount(&mock_server)
            .await;
        let http_client = Client::new(&mock_server.uri()).unwrap();

        let result = network::verify_network_passphrase(
            "Test SDF Network ; September 2015",
            &mock_server.uri(),
            &http_client,
        )
        .await;

        let err = result.unwrap_err();
        match err {
            SdkError::NetworkPassphraseMismatch {
                configured,
                rpc_reported,
            } => {
                assert_eq!(configured, "Test SDF Network ; September 2015");
                assert_eq!(
                    rpc_reported,
                    "Public Global Stellar Network ; September 2015"
                );
            }
            _ => panic!("wrong error variant"),
        }
    }

    #[tokio::test]
    async fn register_builds_real_submission() {
        // "gamma" = 5 chars → 250_000_000 stroops/year (contract tier: 4–6 chars)
        let receipt = client()
            .register(RegistrationRequest {
                label: "gamma".into(),
                owner: OWNER_ADDR.into(),
                duration_years: 2,
                signer: Some("registrar".into()),
            })
            .await
            .unwrap();

        assert_eq!(receipt.name, "gamma.xlm");
        assert_eq!(receipt.owner, OWNER_ADDR);
        assert_eq!(receipt.duration_years, 2);
        assert_eq!(receipt.fee_paid, 500_000_000); // 250_000_000 × 2
        assert_eq!(receipt.submission.status, SubmissionStatus::Submitted);
        assert_eq!(receipt.submission.signer.as_deref(), Some("registrar"));
        assert!(!receipt.submission.tx_hash.is_empty());
        assert!(receipt.submission.contract_id.is_some());
        assert!(receipt.expires_at > 1_682_200_000);
    }

    #[tokio::test]
    async fn register_rejects_empty_label() {
        let result = client()
            .register(RegistrationRequest {
                label: "".into(),
                owner: "GDRA...OWNER".into(),
                duration_years: 1,
                signer: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("label") || msg.contains("empty"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[tokio::test]
    async fn register_rejects_empty_owner() {
        let result = client()
            .register(RegistrationRequest {
                label: "test".into(),
                owner: "".into(),
                duration_years: 1,
                signer: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("owner") || msg.contains("empty"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[tokio::test]
    async fn register_rejects_zero_duration() {
        let result = client()
            .register(RegistrationRequest {
                label: "test".into(),
                owner: "GDRA...OWNER".into(),
                duration_years: 0,
                signer: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("duration") || msg.contains("greater"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[tokio::test]
    async fn test_verify_passphrase_transport_failure() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&mock_server)
            .await;
        let http_client = Client::new(&mock_server.uri()).unwrap();

        let result = network::verify_network_passphrase(
            "Test SDF Network ; September 2015",
            &mock_server.uri(),
            &http_client,
        )
        .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            SdkError::NetworkPassphraseMismatch { .. } => {
                panic!("should be a transport error, not a mismatch")
            }
            _ => {}
        }
    }

    #[test]
    fn test_verify_transaction_passphrase_mismatch() {
        let result = network::verify_transaction_passphrase(
            "Test SDF Network ; September 2015",
            "Public Global Stellar Network ; September 2015",
        );

        let err = result.unwrap_err();
        match err {
            SdkError::TransactionPassphraseMismatch {
                configured,
                in_transaction,
            } => {
                assert_eq!(configured, "Test SDF Network ; September 2015");
                assert_eq!(
                    in_transaction,
                    "Public Global Stellar Network ; September 2015"
                );
            }
            _ => panic!("wrong error variant"),
        }
    }

    #[tokio::test]
    async fn renew_builds_real_submission() {
        let receipt = client()
            .renew(RenewalRequest {
                name: "delta.xlm".into(),
                additional_years: 3,
                signer: Some("owner".into()),
            })
            .await
            .unwrap();

        // Verify receipt structure carries tx metadata
        assert_eq!(receipt.name, "delta.xlm");
        assert_eq!(receipt.additional_years, 3);
        assert_eq!(receipt.fee_paid, 31); // 3 years * 10 base + 1 network
        assert_eq!(receipt.submission.status, SubmissionStatus::Submitted);
        assert_eq!(receipt.submission.signer.as_deref(), Some("owner"));
        assert!(!receipt.submission.tx_hash.is_empty());
        assert!(receipt.submission.contract_id.is_some());
        assert!(receipt.new_expiry > 1_682_200_000);
    }

    #[tokio::test]
    async fn renew_rejects_empty_name() {
        let result = client()
            .renew(RenewalRequest {
                name: "".into(),
                additional_years: 1,
                signer: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("name") || msg.contains("empty"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[tokio::test]
    async fn renew_rejects_zero_years() {
        let result = client()
            .renew(RenewalRequest {
                name: "test.xlm".into(),
                additional_years: 0,
                signer: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("additional_years") || msg.contains("greater"));
            }
            _ => panic!("Expected InvalidRequest error"),
        }
    }

    #[tokio::test]
    async fn quote_requires_registrar_contract() {
        let no_registrar = XlmNsClient::builder("http://localhost")
            .registry(REGISTRY_ID)
            .build();

        let result = no_registrar.quote_registration("alpha", 1).await;
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("registrar"));
            }
            _ => panic!("Expected InvalidRequest when registrar contract ID is missing"),
        }
    }

    #[tokio::test]
    async fn register_requires_registrar_contract() {
        let no_registrar_client = XlmNsClient::builder("http://localhost")
            .registry(REGISTRY_ID)
            .build();

        let result = no_registrar_client
            .register(RegistrationRequest {
                label: "test".into(),
                owner: "GDRA...OWNER".into(),
                duration_years: 1,
                signer: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("registrar"));
            }
            _ => panic!("Expected InvalidRequest error for missing registrar"),
        }
    }

    #[tokio::test]
    async fn renew_requires_registrar_contract() {
        let no_registrar_client = XlmNsClient::builder("http://localhost")
            .registry(REGISTRY_ID)
            .build();

        let result = no_registrar_client
            .renew(RenewalRequest {
                name: "test.xlm".into(),
                additional_years: 1,
                signer: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("registrar"));
            }
            _ => panic!("Expected InvalidRequest error for missing registrar"),
        }
    }

    #[tokio::test]
    async fn submission_includes_fee_breakdown() {
        // "epsilon" = 7 chars → 100_000_000 stroops/year (contract tier: 7+ chars)
        let quote = client().quote_registration("epsilon", 4).await.unwrap();

        assert_eq!(quote.fee_breakdown.base_fee, 400_000_000); // 100_000_000 × 4
        assert_eq!(quote.fee_breakdown.premium_fee, 0);
        assert_eq!(quote.fee_breakdown.network_fee, 0);
        assert_eq!(quote.total_fee, 400_000_000);
        assert!(quote.grace_period_ends_at > quote.expires_at);

        let receipt = client()
            .register(RegistrationRequest {
                label: "epsilon".into(),
                owner: OWNER_ADDR.into(),
                duration_years: 4,
                signer: None,
            })
            .await
            .unwrap();

        assert_eq!(receipt.fee_paid, 400_000_000);
        assert_eq!(
            receipt.submission.network_passphrase,
            Some("Test SDF Network ; September 2015".into())
        );
    }

    #[tokio::test]
    async fn load_reserved_manifest_returns_submission() {
        let submission = client()
            .load_reserved_manifest(
                vec!["admin".to_string(), "root".to_string()],
                Some("deployer".into()),
            )
            .await
            .unwrap();

        assert_eq!(submission.status, SubmissionStatus::Submitted);
        assert_eq!(submission.signer.as_deref(), Some("deployer"));
    }

    // Issue #167 — simulation-first transaction assembly

    #[tokio::test]
    async fn simulate_register_surfaces_fee_estimate() {
        // "alpha" = 5 chars → 250_000_000 stroops/year × 2 years
        let result = client()
            .simulate_register(&RegistrationRequest {
                label: "alpha".into(),
                owner: OWNER_ADDR.into(),
                duration_years: 2,
                signer: None,
            })
            .await
            .unwrap();

        assert!(result.success);
        assert_eq!(result.fee_estimate, 500_000_000);
        assert!(!result.auth_addresses.is_empty());
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn simulate_renew_surfaces_fee_estimate() {
        let result = client()
            .simulate_renew(&RenewalRequest {
                name: "test.xlm".into(),
                additional_years: 3,
                signer: None,
            })
            .await
            .unwrap();

        assert!(result.success);
        assert!(result.fee_estimate > 0);
        assert!(result.error.is_none());
    }

    #[tokio::test]
    async fn simulate_register_requires_registrar_contract() {
        let no_registrar = XlmNsClient::builder("http://localhost")
            .registry(REGISTRY_ID)
            .build();

        let result = no_registrar
            .simulate_register(&RegistrationRequest {
                label: "alpha".into(),
                owner: OWNER_ADDR.into(),
                duration_years: 1,
                signer: None,
            })
            .await;

        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("registrar"));
            }
            _ => panic!("Expected InvalidRequest when registrar contract ID is missing"),
        }
    }

    #[tokio::test]
    async fn simulate_renew_requires_registrar_contract() {
        let no_registrar = XlmNsClient::builder("http://localhost")
            .registry(REGISTRY_ID)
            .build();

        let result = no_registrar
            .simulate_renew(&RenewalRequest {
                name: "test.xlm".into(),
                additional_years: 1,
                signer: None,
            })
            .await;

        match result {
            Err(SdkError::InvalidRequest(msg)) => {
                assert!(msg.contains("registrar"));
            }
            _ => panic!("Expected InvalidRequest when registrar contract ID is missing"),
        }
    }

    // Issue #168 — SDK config expansion

    #[test]
    fn builder_from_preset_testnet_sets_rpc_and_passphrase() {
        use crate::config::NetworkPreset;
        let client = XlmNsClient::builder_from_preset(NetworkPreset::Testnet).build();
        assert!(client.rpc_url.contains("testnet"));
        assert_eq!(
            client.network_passphrase.as_deref(),
            Some("Test SDF Network ; September 2015")
        );
    }

    #[test]
    fn builder_from_preset_mainnet_sets_rpc_and_passphrase() {
        use crate::config::NetworkPreset;
        let client = XlmNsClient::builder_from_preset(NetworkPreset::Mainnet).build();
        assert!(client.rpc_url.contains("soroban.stellar.org"));
        assert_eq!(
            client.network_passphrase.as_deref(),
            Some("Public Global Stellar Network ; September 2015")
        );
    }

    #[test]
    fn missing_resolver_contract_id_is_none() {
        let c = XlmNsClient::builder("http://localhost")
            .registry("CDAD...REGISTRY")
            .build();
        assert!(c.resolver_contract_id.is_none());
    }

    #[test]
    fn missing_nft_contract_id_is_none() {
        let c = XlmNsClient::builder("http://localhost").build();
        assert!(c.nft_contract_id.is_none());
    }

    #[test]
    fn missing_bridge_contract_id_is_none() {
        let c = XlmNsClient::builder("http://localhost").build();
        assert!(c.bridge_contract_id.is_none());
    }

    #[test]
    fn missing_auction_contract_id_is_none() {
        let c = XlmNsClient::builder("http://localhost").build();
        assert!(c.auction_contract_id.is_none());
    }

    #[test]
    fn missing_subdomain_contract_id_is_none() {
        let c = XlmNsClient::builder("http://localhost").build();
        assert!(c.subdomain_contract_id.is_none());
    }

    #[test]
    fn fully_specified_builder_sets_all_contract_ids() {
        let c = XlmNsClient::builder("http://localhost")
            .registry("CDAD...REGISTRY")
            .registrar("CDAD...REGISTRAR")
            .resolver("CDAD...RESOLVER")
            .auction("CDAD...AUCTION")
            .bridge("CDAD...BRIDGE")
            .subdomain("CDAD...SUBDOMAIN")
            .nft("CDAD...NFT")
            .build();

        assert!(c.registry_contract_id.is_some());
        assert!(c.registrar_contract_id.is_some());
        assert!(c.resolver_contract_id.is_some());
        assert!(c.auction_contract_id.is_some());
        assert!(c.bridge_contract_id.is_some());
        assert!(c.subdomain_contract_id.is_some());
        assert!(c.nft_contract_id.is_some());
    }

    // Issue #486 — RPC retry with exponential backoff

    #[tokio::test]
    async fn retry_succeeds_after_transient_transport_failures() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(FailThenSucceed {
                failures_before_success: Arc::new(AtomicUsize::new(2)),
                success_body: network_success_body(),
            })
            .expect(3)
            .mount(&mock_server)
            .await;

        let client = retry_test_client(
            mock_server.uri(),
            crate::config::ClientConfig::default()
                .with_max_retries(3)
                .with_initial_backoff(Duration::from_millis(1))
                .with_jitter(false)
                .with_poll_final_status(false),
        );

        let receipt = client
            .renew(RenewalRequest {
                name: "retry.xlm".into(),
                additional_years: 1,
                signer: None,
            })
            .await
            .unwrap();

        assert_eq!(receipt.name, "retry.xlm");
    }

    #[tokio::test]
    async fn retry_does_not_retry_non_retryable_passphrase_mismatch() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(JsonRpcResponder::new(serde_json::json!({
                "passphrase": "Public Global Stellar Network ; September 2015",
                "protocolVersion": 21
            })))
            .expect(1)
            .mount(&mock_server)
            .await;

        let client = retry_test_client(
            mock_server.uri(),
            crate::config::ClientConfig::default()
                .with_max_retries(3)
                .with_initial_backoff(Duration::from_millis(1))
                .with_jitter(false)
                .with_poll_final_status(false),
        );

        let err = client
            .renew(RenewalRequest {
                name: "retry.xlm".into(),
                additional_years: 1,
                signer: None,
            })
            .await
            .unwrap_err();

        match err {
            SdkError::NetworkPassphraseMismatch { .. } => {}
            other => panic!("expected passphrase mismatch, got {other:?}"),
        }
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn retry_honors_exponential_backoff_delays() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(FailThenSucceed {
                failures_before_success: Arc::new(AtomicUsize::new(2)),
                success_body: network_success_body(),
            })
            .mount(&mock_server)
            .await;

        let client = retry_test_client(
            mock_server.uri(),
            crate::config::ClientConfig::default()
                .with_max_retries(3)
                .with_initial_backoff(Duration::from_millis(100))
                .with_jitter(false)
                .with_poll_final_status(false),
        );

        let renew = client.renew(RenewalRequest {
            name: "retry.xlm".into(),
            additional_years: 1,
            signer: None,
        });
        tokio::pin!(renew);

        tokio::select! {
            _ = &mut renew => panic!("renew finished before first backoff elapsed"),
            _ = tokio::time::sleep(Duration::from_millis(99)) => {}
        }

        tokio::select! {
            _ = &mut renew => panic!("renew finished before second backoff elapsed"),
            _ = tokio::time::sleep(Duration::from_millis(201)) => {}
        }

        let result = renew.await.unwrap();
        assert_eq!(result.name, "retry.xlm");
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn retry_surfaces_rate_limit_error_after_exhausting_retries() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/"))
            .respond_with(ResponseTemplate::new(429))
            .mount(&mock_server)
            .await;

        let client = retry_test_client(
            mock_server.uri(),
            crate::config::ClientConfig::default()
                .with_max_retries(2)
                .with_initial_backoff(Duration::from_millis(10))
                .with_jitter(false)
                .with_poll_final_status(false),
        );

        // Test execute_with_retry directly — calling through renew() would
        // route via verify_write_network() which intentionally swallows
        // transport errors (the preflight network check is non-fatal).
        let rpc_url = mock_server.uri();
        let err = client
            .execute_with_retry("test_rate_limit", |http_client| {
                let configured = "Test SDF Network ; September 2015".to_owned();
                let rpc_url = rpc_url.clone();
                async move {
                    network::verify_network_passphrase(&configured, &rpc_url, &http_client).await
                }
            })
            .await
            .unwrap_err();

        match err {
            SdkError::RateLimitExceeded(details) => {
                assert_eq!(details.retries, 2);
                assert_eq!(details.total_wait_ms, 30); // 10ms + 20ms
            }
            other => panic!("expected rate limit error, got {other:?}"),
        }
    }

    // ── batch_resolve ─────────────────────────────────────────────────────
    //
    // Mock-backed name conventions used below (see `mock_batch_resolve`):
    //   `notfound*`  — resolver holds no record
    //   `expired*`   — record exists but is past its grace period
    //   `noaddress*` — record exists with no address
    //   `rpcfail*`   — the chunk's invocation fails with a retryable error

    mod batch_resolve {
        use super::*;
        use crate::client::take_chunk_invocations;
        use crate::config::ClientConfig;
        use crate::types::BatchResolveError;

        fn batch_client(chunk_size: usize) -> XlmNsClient {
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

        fn names(prefix: &str, count: usize) -> Vec<String> {
            (0..count).map(|i| format!("{prefix}{i}.xlm")).collect()
        }

        #[tokio::test]
        async fn resolves_every_name_and_preserves_input_order() {
            let input = vec![
                "alice.xlm".to_string(),
                "bob.xlm".to_string(),
                "carol.xlm".to_string(),
            ];

            let results = batch_client(50).batch_resolve(input.clone()).await.unwrap();

            assert_eq!(results.len(), input.len());
            let returned: Vec<&str> = results.iter().map(|r| r.name.as_str()).collect();
            assert_eq!(returned, ["alice.xlm", "bob.xlm", "carol.xlm"]);

            for result in &results {
                assert!(result.is_ok(), "{} should resolve: {result:?}", result.name);
                assert!(result.address.is_some());
                // TTL and expiry ride along so callers can cache the answer.
                assert_eq!(result.ttl_seconds, Some(3600));
                assert!(result.expires_at.is_some());
                assert!(result.error.is_none());
            }
        }

        #[tokio::test]
        async fn empty_input_returns_empty_without_invoking_the_contract() {
            take_chunk_invocations();

            let results = batch_client(50).batch_resolve(Vec::new()).await.unwrap();

            assert!(results.is_empty());
            assert_eq!(take_chunk_invocations(), 0);
        }

        #[tokio::test]
        async fn missing_resolver_contract_id_fails_the_whole_call() {
            let client = XlmNsClient::builder("http://localhost")
                .registry(REGISTRY_ID)
                .build();

            let err = client
                .batch_resolve(vec!["alice.xlm".to_string()])
                .await
                .unwrap_err();

            match err {
                SdkError::InvalidRequest(message) => {
                    assert!(
                        message.contains("resolver contract ID"),
                        "unexpected message: {message}"
                    );
                }
                other => panic!("expected InvalidRequest, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn partial_failures_are_reported_per_name() {
            let input = vec![
                "alice.xlm".to_string(),
                "notfound.xlm".to_string(),
                "expired.xlm".to_string(),
                "bob.xlm".to_string(),
                "noaddress.xlm".to_string(),
                "not-a-name".to_string(),
            ];

            let results = batch_client(50).batch_resolve(input).await.unwrap();
            assert_eq!(results.len(), 6);

            // The healthy names still resolve.
            assert!(results[0].is_ok());
            assert!(results[3].is_ok());
            assert_eq!(results[3].name, "bob.xlm");

            assert_eq!(results[1].error, Some(BatchResolveError::NotFound));
            assert!(results[1].address.is_none());

            match &results[2].error {
                Some(BatchResolveError::Expired { expired_at }) => {
                    assert!(*expired_at > 0, "expiry timestamp should be reported");
                }
                other => panic!("expected Expired, got {other:?}"),
            }

            assert_eq!(results[4].error, Some(BatchResolveError::NoAddress));

            match &results[5].error {
                Some(BatchResolveError::InvalidName { reason }) => {
                    assert!(reason.contains("TLD"), "unexpected reason: {reason}");
                }
                other => panic!("expected InvalidName, got {other:?}"),
            }
        }

        #[tokio::test]
        async fn invalid_names_are_rejected_locally_without_an_invocation() {
            take_chunk_invocations();

            let results = batch_client(50)
                .batch_resolve(vec![
                    String::new(),
                    "   ".to_string(),
                    " alice.xlm".to_string(),
                    "alice.eth".to_string(),
                    "-bad-.xlm".to_string(),
                ])
                .await
                .unwrap();

            assert_eq!(results.len(), 5);
            assert!(results
                .iter()
                .all(|r| matches!(r.error, Some(BatchResolveError::InvalidName { .. }))));
            // Nothing was worth sending, so no chunk was invoked at all.
            assert_eq!(take_chunk_invocations(), 0);
        }

        #[tokio::test]
        async fn subdomains_are_accepted_by_client_side_validation() {
            let results = batch_client(50)
                .batch_resolve(vec!["team.alice.xlm".to_string()])
                .await
                .unwrap();

            assert!(results[0].is_ok(), "unexpected: {:?}", results[0]);
        }

        #[tokio::test]
        async fn batches_larger_than_the_chunk_size_are_split() {
            take_chunk_invocations();

            // 10 names at a chunk size of 4 → chunks of 4, 4, 2.
            let results = batch_client(4)
                .batch_resolve(names("name", 10))
                .await
                .unwrap();

            assert_eq!(take_chunk_invocations(), 3);
            assert_eq!(results.len(), 10);
            // Order survives reassembly across chunk boundaries.
            for (index, result) in results.iter().enumerate() {
                assert_eq!(result.name, format!("name{index}.xlm"));
                assert!(result.is_ok());
            }
        }

        #[tokio::test]
        async fn a_batch_within_the_chunk_size_is_sent_as_one_invocation() {
            take_chunk_invocations();

            let results = batch_client(50)
                .batch_resolve(names("name", 50))
                .await
                .unwrap();

            assert_eq!(take_chunk_invocations(), 1);
            assert_eq!(results.len(), 50);
        }

        #[tokio::test]
        async fn locally_rejected_names_do_not_consume_chunk_capacity() {
            take_chunk_invocations();

            // 4 valid names around 2 invalid ones, chunked by 2: the invalid
            // names never enter a chunk, so 4 valid names → exactly 2 chunks.
            let results = batch_client(2)
                .batch_resolve(vec![
                    "alice.xlm".to_string(),
                    "bad".to_string(),
                    "bob.xlm".to_string(),
                    "also-bad".to_string(),
                    "carol.xlm".to_string(),
                    "dave.xlm".to_string(),
                ])
                .await
                .unwrap();

            assert_eq!(take_chunk_invocations(), 2);
            assert_eq!(results.len(), 6);
            assert!(results[0].is_ok());
            assert!(results[1].is_err());
            assert!(results[2].is_ok());
            assert!(results[3].is_err());
            assert!(results[4].is_ok());
            assert!(results[5].is_ok());
        }

        #[tokio::test]
        async fn a_failed_chunk_only_fails_its_own_names() {
            // Chunk size 2 puts the failing name with `bob.xlm` in chunk 2,
            // leaving chunks 1 and 3 to succeed.
            let results = batch_client(2)
                .batch_resolve(vec![
                    "alice.xlm".to_string(),
                    "carol.xlm".to_string(),
                    "rpcfail.xlm".to_string(),
                    "bob.xlm".to_string(),
                    "dave.xlm".to_string(),
                    "erin.xlm".to_string(),
                ])
                .await
                .unwrap();

            assert_eq!(results.len(), 6);
            assert!(results[0].is_ok());
            assert!(results[1].is_ok());
            assert!(results[4].is_ok());
            assert!(results[5].is_ok());

            // Both names sharing the failed chunk report the RPC failure.
            for index in [2, 3] {
                match &results[index].error {
                    Some(BatchResolveError::Rpc { reason }) => {
                        assert!(!reason.is_empty(), "reason should be populated");
                    }
                    other => panic!("expected Rpc error at {index}, got {other:?}"),
                }
            }
        }

        #[tokio::test]
        async fn a_failing_chunk_is_retried_before_being_reported() {
            take_chunk_invocations();

            let client = XlmNsClient::builder("http://localhost")
                .registry(REGISTRY_ID)
                .resolver(RESOLVER_ID)
                .config(
                    ClientConfig::default()
                        .with_batch_chunk_size(10)
                        .with_max_retries(2)
                        .with_initial_backoff(Duration::from_millis(1))
                        .with_jitter(false),
                )
                .build();

            let results = client
                .batch_resolve(vec!["rpcfail.xlm".to_string()])
                .await
                .unwrap();

            // One initial attempt plus two retries, all on the same chunk.
            assert_eq!(take_chunk_invocations(), 3);
            assert!(matches!(
                results[0].error,
                Some(BatchResolveError::Rpc { .. })
            ));
        }

        #[test]
        fn blocking_facade_exposes_batch_resolve() {
            let client =
                crate::blocking::XlmNsBlockingClient::from_async(batch_client(50)).unwrap();

            let results = client
                .batch_resolve(vec!["alice.xlm".to_string(), "notfound.xlm".to_string()])
                .unwrap();

            assert_eq!(results.len(), 2);
            assert!(results[0].is_ok());
            assert_eq!(results[1].error, Some(BatchResolveError::NotFound));
        }
    }
}
