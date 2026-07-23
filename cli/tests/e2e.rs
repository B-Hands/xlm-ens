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

fn network_args() -> Vec<String> {
    vec![
        "--network".into(),
        "testnet".into(),
        "--rpc-url".into(),
        "http://127.0.0.1:1".into(),
        "--network-passphrase".into(),
        "Test SDF Network ; September 2015".into(),
    ]
}

#[allow(dead_code)]
fn base_args() -> Vec<String> {
    let mut args = network_args();
    args.extend([
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
    ]);
    args
}

fn args_for(contract_flags: &[(&str, char)]) -> Vec<String> {
    let mut args = network_args();
    for (flag, fill) in contract_flags {
        args.push(format!("--{flag}"));
        args.push(contract_id(*fill));
    }
    args
}

fn registrar_args() -> Vec<String> {
    args_for(&[("registrar-contract-id", 'B')])
}

fn resolver_args() -> Vec<String> {
    args_for(&[("resolver-contract-id", 'C'), ("registry-contract-id", 'A')])
}

fn registry_args() -> Vec<String> {
    args_for(&[("registry-contract-id", 'A')])
}

fn registry_resolver_args() -> Vec<String> {
    args_for(&[("registry-contract-id", 'A'), ("resolver-contract-id", 'C')])
}

fn registrar_registry_args() -> Vec<String> {
    args_for(&[
        ("registrar-contract-id", 'B'),
        ("registry-contract-id", 'A'),
    ])
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
        vec!["renewal-check", "--help"],
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
        let is_root_help = args == vec!["--help"];
        let assert = bin()
            .args(args)
            .assert()
            .success()
            .stdout(predicate::str::contains("Usage:").or(predicate::str::contains("USAGE:")));

        if is_root_help {
            assert.stdout(predicate::str::contains("XLM Name Service CLI"));
        }
    }
}

#[test]
fn register_emits_human_json_and_csv() {
    let mut args = registrar_args();
    args.extend([
        "register".to_string(),
        "alice".to_string(),
        account_address('H'),
    ]);

    let human = bin()
        .args(&args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human_text = String::from_utf8(human).expect("utf8");
    assert!(human_text.contains("Registration quote for alice.xlm:"));
    assert!(human_text.contains("SUCCESS: registered alice.xlm to"));

    let mut json_args = registrar_args();
    json_args.extend([
        "--output".into(),
        "json".into(),
        "register".into(),
        "alice".into(),
        account_address('H'),
    ]);
    let json = bin()
        .args(&json_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json = json_output(&json);
    assert_eq!(json["name"], "alice.xlm");
    assert_eq!(json["owner"], account_address('H'));
    assert_eq!(json["duration_years"], 1);
    assert_eq!(json["submission_status"], "submitted");
    assert!(json["transaction_hash"].is_string());
    assert!(json["fee_total"].is_number());

    let mut csv_args = registrar_args();
    csv_args.extend([
        "--output".into(),
        "csv".into(),
        "register".into(),
        "alice".into(),
        account_address('H'),
    ]);
    let csv = bin()
        .args(&csv_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rows = csv_rows(&csv);
    assert_eq!(rows.len(), 1);
    assert!(rows[0].len() >= 5);
}

#[test]
fn no_color_flag_disables_ansi_sequences() {
    let mut args = registrar_args();
    args.extend([
        "--no-color".into(),
        "register".into(),
        "alice".into(),
        account_address('H'),
    ]);

    let output = bin()
        .args(&args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(
        !output.contains(&0x1b),
        "expected no ANSI escape sequences when --no-color is set"
    );
}

#[test]
fn no_color_env_var_disables_ansi_sequences() {
    let mut args = registrar_args();
    args.extend(["register".into(), "alice".into(), account_address('H')]);

    let output = bin()
        .env("NO_COLOR", "1")
        .args(&args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    assert!(
        !output.contains(&0x1b),
        "expected no ANSI escape sequences when NO_COLOR is set"
    );
}

#[test]
fn resolve_emits_human_json_and_csv() {
    let mut human_args = resolver_args();
    human_args.extend(["resolve".into(), "alice.xlm".into()]);
    let human = bin()
        .args(&human_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human_text = String::from_utf8(human).expect("utf8");
    assert!(human_text.contains("Name: alice.xlm"));
    assert!(human_text.contains("Address: GDRA"));

    let mut json_args = resolver_args();
    json_args.extend([
        "--output".into(),
        "json".into(),
        "resolve".into(),
        "alice.xlm".into(),
    ]);
    let json = bin()
        .args(&json_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json = json_output(&json);
    assert_eq!(json["name"], "alice.xlm");
    assert!(json["address"].as_str().is_some());
    assert!(json["resolver"].as_str().is_some());

    let mut csv_args = resolver_args();
    csv_args.extend([
        "--output".into(),
        "csv".into(),
        "resolve".into(),
        "alice.xlm".into(),
    ]);
    let csv = bin()
        .args(&csv_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rows = csv_rows(&csv);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].len(), 4);
}

#[test]
fn whois_and_portfolio_support_machine_readable_formats() {
    let mut whois_human_args = registry_resolver_args();
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

    let mut whois_json_args = registry_resolver_args();
    whois_json_args.extend([
        "--output".into(),
        "json".into(),
        "whois".into(),
        "alice.xlm".into(),
    ]);
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
    let mut portfolio_json_args = registry_resolver_args();
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
    let items = portfolio_json
        .as_array()
        .expect("portfolio json should be an array");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["owner"], owner);

    let mut portfolio_human_args = registry_resolver_args();
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

    let mut portfolio_csv_args = registry_resolver_args();
    portfolio_csv_args.extend(["--output".into(), "csv".into(), "portfolio".into(), owner]);
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
fn renewal_check_emits_human_json_and_csv() {
    let owner = account_address('M');

    let mut json_args = registry_resolver_args();
    json_args.extend([
        "--output".into(),
        "json".into(),
        "renewal-check".into(),
        owner.clone(),
    ]);
    let json = bin()
        .args(&json_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let json = json_output(&json);
    let items = json
        .as_array()
        .expect("renewal-check json should be an array");
    assert_eq!(items.len(), 2);

    // The SDK's mock portfolio/registry data uses a fixed reference timestamp
    // that is permanently in the past, so both names are always past their
    // grace period (claimable) and therefore have no renewal cost.
    for item in items {
        assert_eq!(item["status"], "claimable");
        assert!(item["renewal_cost"].is_null());
        assert!(item["auto_renew_status"].is_null());
        assert!(item["days_remaining"].as_i64().unwrap() < 0);
    }
    let names: Vec<&str> = items
        .iter()
        .map(|item| item["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["alice.xlm", "bob.xlm"]);

    let mut human_args = registry_resolver_args();
    human_args.extend(["renewal-check".into(), owner.clone()]);
    let human = bin()
        .args(&human_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let human_text = String::from_utf8(human).expect("utf8");
    assert!(human_text.contains(&format!("Renewal check for {owner}")));
    assert!(human_text.contains("[CLAIMABLE]"));
    assert!(human_text.contains("Summary: 2 claimable, 0 grace period, 0 warning, 0 ok"));

    let mut csv_args = registry_resolver_args();
    csv_args.extend([
        "--output".into(),
        "csv".into(),
        "renewal-check".into(),
        owner,
    ]);
    let csv = bin()
        .args(&csv_args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let rows = csv_rows(&csv);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].len(), 10);
}

#[test]
fn renewal_check_auto_renew_requires_registrar_contract() {
    let mut args = registry_resolver_args();
    args.extend([
        "renewal-check".into(),
        account_address('N'),
        "--auto-renew".into(),
    ]);

    bin()
        .args(&args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("Suggestion:"))
        .stderr(predicate::str::contains("--auto-renew"))
        .stderr(predicate::str::contains("registrar contract ID"));
}

#[test]
fn transfer_and_renew_emit_structured_output() {
    let mut transfer_json_args = registry_args();
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

    let mut renew_csv_args = registrar_registry_args();
    renew_csv_args.extend([
        "--output".into(),
        "csv".into(),
        "renew".into(),
        "alice.xlm".into(),
    ]);
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

    let mut args = registry_args();
    args.extend([
        "transfer".into(),
        "alice.xlm".into(),
        "not-an-account".into(),
    ]);
    bin()
        .args(&args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("Suggestion:"))
        .stderr(predicate::str::contains("new_owner is invalid"));

    let mut renew_args = registrar_registry_args();
    renew_args.extend(["renew".into(), "notfound.xlm".into()]);
    bin()
        .args(&renew_args)
        .assert()
        .failure()
        .stderr(predicate::str::contains("Error:"))
        .stderr(predicate::str::contains("Suggestion:"))
        .stderr(predicate::str::contains("not registered"));
}
