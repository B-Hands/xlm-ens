use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::Value;

fn bin() -> Command {
    Command::cargo_bin("xlm-ns-cli").expect("binary should build")
}

fn contract_id(fill: char) -> String {
    format!("C{}", fill.to_string().repeat(55))
}

fn account_address(fill: char) -> String {
    format!("G{}", fill.to_string().repeat(55))
}

fn base_args() -> Vec<String> {
    vec![
        "--network".into(),
        "testnet".into(),
        "--rpc-url".into(),
        "http://127.0.0.1:1".into(),
        "--network-passphrase".into(),
        "Test SDF Network ; September 2015".into(),
        "--registry-contract-id".into(),
        contract_id('A'),
        "--registrar-contract-id".into(),
        contract_id('B'),
        "--resolver-contract-id".into(),
        contract_id('C'),
        "--auction-contract-id".into(),
        contract_id('D'),
        "--bridge-contract-id".into(),
        contract_id('E'),
        "--subdomain-contract-id".into(),
        contract_id('F'),
        "--nft-contract-id".into(),
        contract_id('G'),
    ]
}

fn csv_rows(output: &[u8]) -> Vec<Vec<String>> {
    let mut reader = csv::Reader::from_reader(output);
    reader
        .records()
        .map(|record| {
            record
                .expect("csv row should parse")
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn json_output(output: &[u8]) -> Value {
    serde_json::from_slice(output).expect("stdout should be valid json")
}

#[test]
fn root_and_subcommand_help_are_available() {
    let commands = [
        vec!["--help"],
        vec!["register", "--help"],
        vec!["resolve", "--help"],
        vec!["reverse-resolve", "--help"],
        vec!["transfer", "--help"],
        vec!["renew", "--help"],
        vec!["whois", "--help"],
        vec!["portfolio", "--help"],
        vec!["text", "--help"],
        vec!["auction", "--help"],
        vec!["bridge", "--help"],
        vec!["subdomain", "--help"],
        vec!["nft", "--help"],
        vec!["config", "--help"],
        vec!["bulk", "--help"],
        vec!["watch", "--help"],
        vec!["completions", "--help"],
    ];

    for args in commands {
        let assert = bin()
            .args(args)
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:").or(predicate::str::contains("USAGE:")));

        if args == vec!["--help"] {
            assert.stdout(predicate::str::contains("XLM Name Service CLI"));
        }
    }
}

#[test]
fn register_emits_human_json_and_csv() {
    let mut args = base_args();
    args.extend([
        "register".to_string(),
        "alice".to_string(),
        account_address('H'),
    ]);

    let human = bin().args(&args).assert().success().get_output().stdout.clone();
    let human_text = String::from_utf8(human).expect("utf8");
    assert!(human_text.contains("Registration quote for alice.xlm:"));
    assert!(human_text.contains("SUCCESS: registered alice.xlm to"));

    let mut json_args = base_args();
    json_args.extend([
        "--output".into(),
        "json".into(),
        "register".into(),
        "alice".into(),
        account_address('H'),
    ]);
    let json = bin().args(&json_args).assert().success().get_output().stdout.clone();
    let json = json_output(&json);
    assert_eq!(json["name"], "alice.xlm");
    assert_eq!(json["owner"], account_address('H'));
    assert_eq!(json["duration_years"], 1);
    assert_eq!(json["submission_status"], "submitted");
    assert!(json["transaction_hash"].is_string());
    assert!(json["fee_total"].is_number());

    let mut csv_args = base_args();
    csv_args.extend([
        "--output".into(),
        "csv".into(),
        "register".into(),
        "alice".into(),
        account_address('H'),
    ]);
    let csv = bin().args(&csv_args).assert().success().get_output().stdout.clone();
    let rows = csv_rows(&csv);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].len() >= 5);
}

#[test]
fn resolve_emits_human_json_and_csv() {
    let mut human_args = base_args();
    human_args.extend(["resolve".into(), "alice.xlm".into()]);
    let human = bin().args(&human_args).assert().success().get_output().stdout.clone();
    let human_text = String::from_utf8(human).expect("utf8");
    assert!(human_text.contains("Name: alice.xlm"));
    assert!(human_text.contains("Address: GDRA"));

    let mut json_args = base_args();
    json_args.extend(["--output".into(), "json".into(), "resolve".into(), "alice.xlm".into()]);
    let json = bin().args(&json_args).assert().success().get_output().stdout.clone();
    let json = json_output(&json);
    assert_eq!(json["name"], "alice.xlm");
    assert!(json["address"].as_str().is_some());
    assert!(json["resolver"].as_str().is_some());

    let mut csv_args = base_args();
    csv_args.extend(["--output".into(), "csv".into(), "resolve".into(), "alice.xlm".into()]);
    let csv = bin().args(&csv_args).assert().success().get_output().stdout.clone();
    let rows = csv_rows(&csv);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 4);
}

#[test]
fn whois_and_portfolio_support_machine_readable_formats() {
    let mut whois_human_args = base_args();
    whois_human_args.extend(["whois".into(), "alice.xlm".into()]);
    let whois_human = bin()
        .args(&whois_human_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let whois_human = String::from_utf8(whois_human).expect("utf8");
    assert!(whois_human.contains("alice.xlm"));
    assert!(whois_human.contains("Owner:"));

    let mut whois_json_args = base_args();
    whois_json_args.extend(["--output".into(), "json".into(), "whois".into(), "alice.xlm".into()]);
    let whois_json = bin()
        .args(&whois_json_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let whois_json = json_output(&whois_json);
    assert_eq!(whois_json["name"], "alice.xlm");
    assert_eq!(whois_json["network"], "testnet");

    let owner = account_address('J');
    let mut portfolio_json_args = base_args();
    portfolio_json_args.extend([
        "--output".into(),
        "json".into(),
        "portfolio".into(),
        owner.clone(),
    ]);
    let portfolio_json = bin()
        .args(&portfolio_json_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let portfolio_json: Value = json_output(&portfolio_json);
    let items = portfolio_json.as_array().expect("portfolio json should be an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["owner"], owner);

    let mut portfolio_human_args = base_args();
    portfolio_human_args.extend(["portfolio".into(), owner.clone()]);
    let portfolio_human = bin()
        .args(&portfolio_human_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let portfolio_human = String::from_utf8(portfolio_human).expect("utf8");
    assert!(portfolio_human.contains("Portfolio for"));
    assert!(portfolio_human.contains("alice.xlm"));

    let mut portfolio_csv_args = base_args();
    portfolio_csv_args.extend([
        "--output".into(),
        "csv".into(),
        "portfolio".into(),
        owner,
    ]);
    let portfolio_csv = bin()
        .args(&portfolio_csv_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rows = csv_rows(&portfolio_csv);
    assert_eq!(rows.len(), 2);
}

#[test]
fn transfer_and_renew_emit_structured_output() {
    let mut transfer_json_args = base_args();
    transfer_json_args.extend([
        "--output".into(),
        "json".into(),
        "transfer".into(),
        "alice.xlm".into(),
        account_address('K'),
    ]);
    let transfer_json = bin()
        .args(&transfer_json_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let transfer_json = json_output(&transfer_json);
    assert_eq!(transfer_json["name"], "alice.xlm");
    assert_eq!(transfer_json["new_owner"], account_address('K'));
    assert_eq!(transfer_json["status"], "submitted");

    let mut renew_csv_args = base_args();
    renew_csv_args.extend(["--output".into(), "csv".into(), "renew".into(), "alice.xlm".into()]);
    let renew_csv = bin()
        .args(&renew_csv_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rows = csv_rows(&renew_csv);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].len() >= 5);
}

#[test]
fn missing_arguments_and_invalid_inputs_fail_cleanly() {
    bin()
        .args(["resolve"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Usage:").or(predicate::str::contains("USAGE:")));

    let mut args = base_args();
    args.extend([
        "transfer".into(),
        "alice.xlm".into(),
        "not-an-account".into(),
    ]);
    bin()
        .args(&args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to submit transfer"))
        .stderr(predicate::str::contains("new_owner is invalid"));

    let mut renew_args = base_args();
    renew_args.extend(["renew".into(), "notfound.xlm".into()]);
    bin()
        .args(&renew_args)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "is not registered and cannot be renewed",
        ));
}
