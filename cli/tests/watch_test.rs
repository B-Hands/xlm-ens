use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn get_watchlist_path() -> anyhow::Result<std::path::PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("xlm-ns");
    Ok(config_dir.join("watchlist.json"))
}

#[test]
fn test_watch_command() -> Result<(), Box<dyn std::error::Error>> {
    let watchlist_path = get_watchlist_path()?;
    if watchlist_path.exists() {
        fs::remove_file(&watchlist_path)?;
    }

    let mut cmd = Command::cargo_bin("xlm-ns-cli")?;
    cmd.arg("watch")
        .arg("add")
        .arg("test.xlm")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Added 'test.xlm' to the watchlist.",
        ));

    let mut cmd = Command::cargo_bin("xlm-ns-cli")?;
    cmd.arg("watch")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("test.xlm"));

    // `watch check` requires a configured registry contract ID and a live RPC
    // endpoint, so it is skipped in offline integration tests.

    let mut cmd = Command::cargo_bin("xlm-ns-cli")?;
    cmd.arg("watch")
        .arg("remove")
        .arg("test.xlm")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Removed 'test.xlm' from the watchlist.",
        ));

    let mut cmd = Command::cargo_bin("xlm-ns-cli")?;
    cmd.arg("watch")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("The watchlist is empty."));

    Ok(())
}
