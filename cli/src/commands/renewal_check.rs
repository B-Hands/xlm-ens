use crate::commands::portfolio::{is_timeout_error, PortfolioOptions};
use crate::config::NetworkConfig;
use crate::export;
use crate::output::{print_human, print_human_err, with_spinner, OutputFormat};
use colored::Colorize;
use serde::Serialize;
use xlm_ns_common::time::{is_active_at, is_claimable_at};
use xlm_ns_sdk::client::XlmNsClient;
use xlm_ns_sdk::types::{RenewalRequest, ResolutionResult};

const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Urgency {
    Claimable,
    GracePeriod,
    Warning,
    Ok,
}

impl Urgency {
    fn classify(expires_at: u64, grace_period_ends_at: u64, now_unix: u64, warn_days: u32) -> Self {
        // Use the record's own `grace_period_ends_at` as the source of truth rather than
        // `xlm_ns_common::time::within_grace_period`, which recomputes the boundary from
        // `expires_at` using the default grace duration and would disagree with the
        // authoritative per-record value if a non-default duration was ever used.
        if is_claimable_at(grace_period_ends_at, now_unix) {
            Self::Claimable
        } else if !is_active_at(expires_at, now_unix) {
            Self::GracePeriod
        } else {
            let warn_seconds = i64::from(warn_days) * SECONDS_PER_DAY;
            let remaining = expires_at as i64 - now_unix as i64;
            if remaining <= warn_seconds {
                Self::Warning
            } else {
                Self::Ok
            }
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Claimable => "claimable",
            Self::GracePeriod => "grace_period",
            Self::Warning => "warning",
            Self::Ok => "ok",
        }
    }

    fn human_tag(self) -> &'static str {
        match self {
            Self::Claimable => "CLAIMABLE",
            Self::GracePeriod => "GRACE PERIOD",
            Self::Warning => "WARNING",
            Self::Ok => "OK",
        }
    }

    fn color(self) -> &'static str {
        match self {
            Self::Claimable | Self::GracePeriod => "red",
            Self::Warning => "yellow",
            Self::Ok => "green",
        }
    }

    fn auto_renew_eligible(self) -> bool {
        matches!(self, Self::Warning | Self::GracePeriod)
    }
}

fn days_between(now_unix: u64, expires_at: u64) -> i64 {
    let diff = expires_at as i64 - now_unix as i64;
    if diff >= 0 {
        (diff + SECONDS_PER_DAY - 1) / SECONDS_PER_DAY
    } else {
        -((-diff + SECONDS_PER_DAY - 1) / SECONDS_PER_DAY)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
struct RenewalCheckRecord {
    name: String,
    owner: String,
    status: String,
    days_remaining: i64,
    expires_at: i64,
    grace_period_ends_at: i64,
    renewal_cost: Option<u64>,
    renewal_currency: Option<String>,
    auto_renew_status: Option<String>,
    auto_renew_tx_hash: Option<String>,
}

fn fetch_owned_names(
    client: &XlmNsClient,
    owner: &str,
    options: PortfolioOptions,
    output: OutputFormat,
) -> anyhow::Result<Vec<ResolutionResult>> {
    let mut cursor = options
        .page
        .map(|page| page.saturating_sub(1) * options.batch_size);
    let mut page_size = options.batch_size;
    let mut fetched = 0usize;
    let mut names = Vec::new();

    loop {
        let remaining_limit = options.limit.map(|limit| limit.saturating_sub(fetched));
        if remaining_limit == Some(0) {
            break;
        }
        let page = loop {
            let requested = remaining_limit.map_or(page_size, |remaining| remaining.min(page_size));
            match client.list_registrations_by_owner_page(owner, cursor, requested) {
                Ok(page) => break page,
                Err(err) if is_timeout_error(&err) && page_size > 1 => {
                    page_size = (page_size / 2).max(1);
                    print_human_err(&format!(
                        "RPC timed out while fetching portfolio; retrying with batch size {page_size}"
                    ));
                    continue;
                }
                Err(err) => {
                    return Err(anyhow::anyhow!(
                        "Failed to fetch portfolio for {owner}: {err}"
                    ));
                }
            }
        };

        fetched += page.items.len();
        if output == OutputFormat::Human {
            print_human_err(&format!("Fetched {fetched}/{} names...", page.total));
        }
        names.extend(page.items);

        if options.page.is_some() || cursor == page.next_cursor || page.next_cursor.is_none() {
            break;
        }
        cursor = page.next_cursor;
    }

    Ok(names)
}

pub async fn run_renewal_check(
    config: NetworkConfig,
    output: OutputFormat,
    dry_run: bool,
    owner: &str,
    warn_days: u32,
    auto_renew: bool,
    options: PortfolioOptions,
) -> anyhow::Result<()> {
    let options = options.normalized();

    if auto_renew && config.registrar_contract_id.is_none() {
        return Err(anyhow::anyhow!(
            "`--auto-renew` requires a registrar contract ID. Set `--registrar-contract-id`, the config file value, or the network default."
        ));
    }

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut client = XlmNsClient::new(
        config.rpc_url.clone(),
        Some(config.network_passphrase.clone()),
        config.registry_contract_id.clone(),
        config.subdomain_contract_id.clone(),
        config.bridge_contract_id.clone(),
        config.auction_contract_id.clone(),
    )
    .with_resolver(
        config
            .resolver_contract_id
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
    );
    let has_registrar = config.registrar_contract_id.is_some();
    if let Some(registrar_id) = config.registrar_contract_id.clone() {
        client = client.with_registrar(registrar_id);
    }

    let names = fetch_owned_names(&client, owner, options, output)?;

    if names.is_empty() {
        match output {
            OutputFormat::Human => {
                print_human(&format!("Renewal check for {owner}:\n  [no names found]"));
            }
            OutputFormat::Json => {
                export::write_json(&Vec::<RenewalCheckRecord>::new(), &mut std::io::stdout())
                    .map_err(anyhow::Error::msg)?;
            }
            OutputFormat::Csv => {
                export::write_csv(&Vec::<RenewalCheckRecord>::new(), &mut std::io::stdout())
                    .map_err(anyhow::Error::msg)?;
            }
        }
        return Ok(());
    }

    let mut ranked: Vec<(Urgency, RenewalCheckRecord)> = Vec::with_capacity(names.len());

    for entry in &names {
        let metadata = client.get_registry_metadata(&entry.name).await?;
        let expires_at = metadata.expires_at;
        let grace_period_ends_at = metadata.grace_period_ends_at;
        let urgency = Urgency::classify(expires_at, grace_period_ends_at, now_unix, warn_days);
        let days_remaining = days_between(now_unix, expires_at);

        // Claimable names are past their grace period and can no longer be
        // renewed by the current owner, so a renewal cost doesn't apply.
        let renewal_cost = if has_registrar && urgency != Urgency::Claimable {
            client
                .simulate_renew(&RenewalRequest {
                    name: entry.name.clone(),
                    additional_years: 1,
                    signer: None,
                })
                .await
                .ok()
                .map(|sim| sim.fee_estimate)
        } else {
            None
        };

        let mut record = RenewalCheckRecord {
            name: entry.name.clone(),
            owner: entry
                .address
                .clone()
                .unwrap_or_else(|| metadata.owner.clone()),
            status: urgency.label().to_string(),
            days_remaining,
            expires_at: expires_at as i64,
            grace_period_ends_at: grace_period_ends_at as i64,
            renewal_cost,
            renewal_currency: renewal_cost.map(|_| "XLM".to_string()),
            auto_renew_status: None,
            auto_renew_tx_hash: None,
        };

        if auto_renew && urgency.auto_renew_eligible() {
            let request = RenewalRequest {
                name: entry.name.clone(),
                additional_years: 1,
                signer: None,
            };
            if dry_run {
                match client.simulate_renew(&request).await {
                    Ok(sim) => {
                        record.auto_renew_status = Some(format!(
                            "dry-run: would renew, estimated fee {} XLM",
                            sim.fee_estimate
                        ));
                    }
                    Err(err) => {
                        record.auto_renew_status =
                            Some(format!("dry-run simulation failed: {err}"));
                    }
                }
            } else {
                match with_spinner(
                    format!("Submitting renewal for {}", entry.name),
                    output,
                    client.renew(request),
                )
                .await
                {
                    Ok(receipt) => {
                        record.auto_renew_status = Some(receipt.submission.status.to_string());
                        record.auto_renew_tx_hash = Some(receipt.submission.tx_hash);
                    }
                    Err(err) => {
                        record.auto_renew_status = Some(format!("failed: {err}"));
                    }
                }
            }
        }

        ranked.push((urgency, record));
    }

    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.days_remaining.cmp(&b.1.days_remaining))
    });
    let records: Vec<RenewalCheckRecord> = ranked.into_iter().map(|(_, record)| record).collect();

    match output {
        OutputFormat::Human => print_human_report(owner, warn_days, &records),
        OutputFormat::Json => {
            export::write_json(&records, &mut std::io::stdout()).map_err(anyhow::Error::msg)?;
        }
        OutputFormat::Csv => {
            export::write_csv(&records, &mut std::io::stdout()).map_err(anyhow::Error::msg)?;
        }
    }

    Ok(())
}

fn print_human_report(owner: &str, warn_days: u32, records: &[RenewalCheckRecord]) {
    print_human(&format!(
        "Renewal check for {owner} (warn threshold: {warn_days} day(s)):"
    ));

    let mut claimable = 0usize;
    let mut grace = 0usize;
    let mut warning = 0usize;
    let mut ok = 0usize;

    for record in records {
        let (tag, color) = match record.status.as_str() {
            "claimable" => {
                claimable += 1;
                (Urgency::Claimable.human_tag(), Urgency::Claimable.color())
            }
            "grace_period" => {
                grace += 1;
                (
                    Urgency::GracePeriod.human_tag(),
                    Urgency::GracePeriod.color(),
                )
            }
            "warning" => {
                warning += 1;
                (Urgency::Warning.human_tag(), Urgency::Warning.color())
            }
            _ => {
                ok += 1;
                (Urgency::Ok.human_tag(), Urgency::Ok.color())
            }
        };

        let timing = if record.days_remaining >= 0 {
            format!("expires in {} day(s)", record.days_remaining)
        } else {
            format!("expired {} day(s) ago", -record.days_remaining)
        };

        let cost = match record.renewal_cost {
            Some(cost) => format!(
                ", renewal cost: {cost} {}",
                record.renewal_currency.as_deref().unwrap_or("XLM")
            ),
            None => String::new(),
        };

        let mut line = format!("  [{tag}] {} - {timing}{cost}", record.name);
        if let Some(status) = &record.auto_renew_status {
            line.push_str(&format!(" | auto-renew: {status}"));
            if let Some(hash) = &record.auto_renew_tx_hash {
                line.push_str(&format!(" ({hash})"));
            }
        }

        println!("{}", line.color(color));
    }

    print_human(&format!(
        "\nSummary: {claimable} claimable, {grace} grace period, {warning} warning, {ok} ok"
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: u64 = 86_400;

    #[test]
    fn classify_ok_when_well_beyond_warn_threshold() {
        let now = 1_000_000_000;
        let expires_at = now + 60 * DAY;
        let grace_ends = expires_at + 30 * DAY;
        assert_eq!(
            Urgency::classify(expires_at, grace_ends, now, 30),
            Urgency::Ok
        );
    }

    #[test]
    fn classify_warning_at_threshold_boundary() {
        let now = 1_000_000_000;
        let warn_days = 30;
        // Exactly at the threshold (inclusive) should already warn.
        let expires_at = now + u64::from(warn_days) * DAY;
        let grace_ends = expires_at + 30 * DAY;
        assert_eq!(
            Urgency::classify(expires_at, grace_ends, now, warn_days),
            Urgency::Warning
        );

        // One second beyond the threshold should still be OK.
        let expires_at_ok = expires_at + 1;
        assert_eq!(
            Urgency::classify(expires_at_ok, expires_at_ok + 30 * DAY, now, warn_days),
            Urgency::Ok
        );
    }

    #[test]
    fn classify_grace_period_after_expiry_before_grace_end() {
        let now = 1_000_000_000;
        let expires_at = now - DAY;
        let grace_ends = now + 10 * DAY;
        assert_eq!(
            Urgency::classify(expires_at, grace_ends, now, 30),
            Urgency::GracePeriod
        );
    }

    #[test]
    fn classify_claimable_after_grace_period_ends() {
        let now = 1_000_000_000;
        let expires_at = now - 60 * DAY;
        let grace_ends = now - DAY;
        assert_eq!(
            Urgency::classify(expires_at, grace_ends, now, 30),
            Urgency::Claimable
        );
    }

    #[test]
    fn urgency_orders_most_critical_first() {
        let mut urgencies = vec![
            Urgency::Ok,
            Urgency::Claimable,
            Urgency::Warning,
            Urgency::GracePeriod,
        ];
        urgencies.sort();
        assert_eq!(
            urgencies,
            vec![
                Urgency::Claimable,
                Urgency::GracePeriod,
                Urgency::Warning,
                Urgency::Ok,
            ]
        );
    }

    #[test]
    fn only_warning_and_grace_period_are_auto_renew_eligible() {
        assert!(!Urgency::Ok.auto_renew_eligible());
        assert!(Urgency::Warning.auto_renew_eligible());
        assert!(Urgency::GracePeriod.auto_renew_eligible());
        assert!(!Urgency::Claimable.auto_renew_eligible());
    }

    #[test]
    fn days_between_rounds_up_for_future_and_past_timestamps() {
        let now = 1_000_000_000;
        assert_eq!(days_between(now, now), 0);
        assert_eq!(days_between(now, now + 1), 1);
        assert_eq!(days_between(now, now + DAY), 1);
        assert_eq!(days_between(now, now + DAY + 1), 2);
        assert_eq!(days_between(now, now - 1), -1);
        assert_eq!(days_between(now, now - DAY), -1);
        assert_eq!(days_between(now, now - DAY - 1), -2);
    }
}
