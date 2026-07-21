use assert_cmd::Command;
use predicates::prelude::*;
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_bulk_register_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("names.csv");
    let mut file = File::create(&file_path)?;
    writeln!(file, "name,duration,owner")?;
    writeln!(file, "name1,1,G...")?;
    writeln!(file, "name2,2,G...")?;

    let mut cmd = Command::cargo_bin("xlm-ns-cli")?;
    cmd.arg("bulk")
        .arg("register")
        .arg("--file")
        .arg(&file_path)
        .arg("--dry-run");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Dry run: The following names would be registered:",
        ))
        .stdout(predicate::str::contains(
            "  - Name: name1, Duration: 1, Owner: G...",
        ))
        .stdout(predicate::str::contains(
            "  - Name: name2, Duration: 2, Owner: G...",
        ));

    Ok(())
}

#[test]
fn test_bulk_renew_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempdir()?;
    let file_path = dir.path().join("names.csv");
    let mut file = File::create(&file_path)?;
    writeln!(file, "name,duration")?;
    writeln!(file, "name1,1")?;
    writeln!(file, "name2,2")?;

    let mut cmd = Command::cargo_bin("xlm-ns-cli")?;
    cmd.arg("bulk")
        .arg("renew")
        .arg("--file")
        .arg(&file_path)
        .arg("--dry-run");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Dry run: The following names would be renewed:",
        ))
        .stdout(predicate::str::contains("  - Name: name1, Duration: 1"))
        .stdout(predicate::str::contains("  - Name: name2, Duration: 2"));

    Ok(())
}
