//! Exact-second boundary tests for name expiry.
//!
//! A name moves through three lifecycle phases, and both transitions are
//! decided by a single comparison against `now_unix`. An off-by-one in either
//! comparison would let a name be used one second too long, or strand it one
//! second early, so each transition is pinned here at `t - 1`, `t`, and
//! `t + 1`.
//!
//! The comparisons under test live in `xlm_ns_common::time` and are
//! **inclusive of the boundary second**:
//!
//! ```text
//! is_active_at(expires_at, now)          == now <= expires_at
//! is_claimable_at(grace_period_end, now) == now >  grace_period_end
//! ```
//!
//! So the second named by `expires_at` is the last active second, and the
//! second named by `grace_period_ends_at` is the last grace second. Both
//! transitions land on `boundary + 1`. The tests below assert that shape
//! directly rather than restating it, so a change to either comparison fails
//! here first.
//!
//! Phase-by-phase, the operations permitted by the registry are:
//!
//! | Operation  | Active | Grace | Claimable |
//! |------------|--------|-------|-----------|
//! | `resolve`  | yes    | no    | no        |
//! | `transfer` | yes    | no    | no        |
//! | `renew`    | yes    | yes   | no        |
//! | `register` | no (taken) | no (`NotYetClaimable`) | yes |

#[cfg(test)]
mod tests {
    extern crate std;

    use soroban_sdk::{testutils::Address as _, Address, Env, String};

    use crate::{NameState, RegistryContract, RegistryContractClient, RegistryError};
    use xlm_ns_common::{GRACE_PERIOD_SECONDS, MAX_REGISTRATION_YEARS, YEAR_SECONDS};

    /// Registration start used by every test below. Any value works; a
    /// non-zero one keeps `boundary - 1` from underflowing.
    const REGISTERED_AT: u64 = 1_700_000_000;

    struct Fixture {
        env: Env,
        contract_id: Address,
        owner: Address,
        name: String,
        expires_at: u64,
        grace_period_ends_at: u64,
    }

    impl Fixture {
        /// Registers `boundary.xlm` for `duration_seconds` with the standard
        /// grace period, starting at [`REGISTERED_AT`].
        fn new(duration_seconds: u64) -> Self {
            Self::with_grace(duration_seconds, GRACE_PERIOD_SECONDS)
        }

        fn with_grace(duration_seconds: u64, grace_seconds: u64) -> Self {
            let env = Env::default();
            env.mock_all_auths();
            let contract_id = env.register(RegistryContract, ());

            let owner = Address::generate(&env);
            let name = String::from_str(&env, "boundary.xlm");
            let expires_at = REGISTERED_AT + duration_seconds;
            let grace_period_ends_at = expires_at + grace_seconds;

            let fixture = Self {
                env,
                contract_id,
                owner,
                name,
                expires_at,
                grace_period_ends_at,
            };

            fixture.client().register(
                &fixture.name,
                &fixture.owner,
                &None::<String>,
                &None::<String>,
                &REGISTERED_AT,
                &expires_at,
                &grace_period_ends_at,
            );

            fixture
        }

        fn client(&self) -> RegistryContractClient<'_> {
            RegistryContractClient::new(&self.env, &self.contract_id)
        }
    }

    // ── Active → grace period ─────────────────────────────────────────────

    #[test]
    fn expiry_second_is_the_last_active_second() {
        let f = Fixture::new(YEAR_SECONDS);
        let client = f.client();

        // One second before expiry: unambiguously active.
        assert_eq!(
            client.name_state(&f.name, &(f.expires_at - 1)),
            NameState::Active,
        );
        // At the expiry second itself the name is still active — the
        // comparison is `now <= expires_at`.
        assert_eq!(client.name_state(&f.name, &f.expires_at), NameState::Active,);
        // The transition lands on the following second.
        assert_eq!(
            client.name_state(&f.name, &(f.expires_at + 1)),
            NameState::GracePeriod,
        );
    }

    #[test]
    fn resolve_stops_working_one_second_after_expiry() {
        let f = Fixture::new(YEAR_SECONDS);
        let client = f.client();

        assert_eq!(
            client.resolve(&f.name, &(f.expires_at - 1)).owner,
            f.owner,
            "resolve should succeed a second before expiry",
        );
        assert_eq!(
            client.resolve(&f.name, &f.expires_at).owner,
            f.owner,
            "resolve should still succeed at the expiry second",
        );
        assert!(
            matches!(
                client.try_resolve(&f.name, &(f.expires_at + 1)),
                Err(Ok(RegistryError::NotActive)),
            ),
            "resolve should fail once the name enters the grace period",
        );
    }

    #[test]
    fn transfer_stops_working_one_second_after_expiry() {
        let f = Fixture::new(YEAR_SECONDS);
        let client = f.client();
        let recipient = Address::generate(&f.env);

        // Rejected a second past expiry...
        assert!(matches!(
            client.try_transfer(&f.name, &f.owner, &recipient, &(f.expires_at + 1)),
            Err(Ok(RegistryError::NotActive)),
        ));
        // ...but allowed at the expiry second itself. Asserted last so the
        // successful transfer does not change the owner for the check above.
        client.transfer(&f.name, &f.owner, &recipient, &f.expires_at);
        assert_eq!(client.resolve(&f.name, &f.expires_at).owner, recipient);
    }

    #[test]
    fn renew_is_allowed_on_both_sides_of_the_expiry_boundary() {
        // Renewal is the operation that must survive the active → grace
        // transition, so it is checked at each of the three seconds.
        for offset in [-1i64, 0, 1] {
            let f = Fixture::new(YEAR_SECONDS);
            let client = f.client();
            let now = (f.expires_at as i64 + offset) as u64;

            let new_expiry = f.expires_at + YEAR_SECONDS;
            client.renew(
                &f.name,
                &f.owner,
                &new_expiry,
                &(new_expiry + GRACE_PERIOD_SECONDS),
                &now,
            );

            assert_eq!(
                client.resolve(&f.name, &now).expires_at,
                new_expiry,
                "renew should be permitted at expiry offset {offset}",
            );
        }
    }

    // ── Grace period → claimable ──────────────────────────────────────────

    #[test]
    fn grace_end_second_is_the_last_grace_second() {
        let f = Fixture::new(YEAR_SECONDS);
        let client = f.client();

        assert_eq!(
            client.name_state(&f.name, &(f.grace_period_ends_at - 1)),
            NameState::GracePeriod,
        );
        // At the grace-end second the name is still in grace — the comparison
        // is `now > grace_period_ends_at`.
        assert_eq!(
            client.name_state(&f.name, &f.grace_period_ends_at),
            NameState::GracePeriod,
        );
        assert_eq!(
            client.name_state(&f.name, &(f.grace_period_ends_at + 1)),
            NameState::Claimable,
        );
    }

    #[test]
    fn renew_stops_working_one_second_after_the_grace_period_ends() {
        let f = Fixture::new(YEAR_SECONDS);

        let renew_at = |now: u64| {
            let new_expiry = now + YEAR_SECONDS;
            f.client().try_renew(
                &f.name,
                &f.owner,
                &new_expiry,
                &(new_expiry + GRACE_PERIOD_SECONDS),
                &now,
            )
        };

        assert!(
            renew_at(f.grace_period_ends_at - 1).is_ok(),
            "renew should work a second before the grace period ends",
        );
        assert!(
            renew_at(f.grace_period_ends_at).is_ok(),
            "renew should work at the grace-end second itself",
        );

        // A fresh fixture: the renewals above moved this one's boundaries.
        let g = Fixture::new(YEAR_SECONDS);
        let past_grace = g.grace_period_ends_at + 1;
        let new_expiry = past_grace + YEAR_SECONDS;
        assert!(
            matches!(
                g.client().try_renew(
                    &g.name,
                    &g.owner,
                    &new_expiry,
                    &(new_expiry + GRACE_PERIOD_SECONDS),
                    &past_grace,
                ),
                Err(Ok(RegistryError::NotActive)),
            ),
            "renew should fail once the name is claimable",
        );
    }

    #[test]
    fn re_registration_becomes_possible_one_second_after_the_grace_period() {
        let f = Fixture::new(YEAR_SECONDS);
        let client = f.client();
        let claimant = Address::generate(&f.env);

        let register_at = |now: u64| {
            f.client().try_register(
                &f.name,
                &claimant,
                &None::<String>,
                &None::<String>,
                &now,
                &(now + YEAR_SECONDS),
                &(now + YEAR_SECONDS + GRACE_PERIOD_SECONDS),
            )
        };

        // Still in grace one second before the boundary, and at it.
        assert!(matches!(
            register_at(f.grace_period_ends_at - 1),
            Err(Ok(RegistryError::NotYetClaimable)),
        ));
        assert!(matches!(
            register_at(f.grace_period_ends_at),
            Err(Ok(RegistryError::NotYetClaimable)),
        ));

        // One second later a third party may take the name.
        let claim_at = f.grace_period_ends_at + 1;
        assert!(register_at(claim_at).is_ok());
        assert_eq!(client.resolve(&f.name, &claim_at).owner, claimant);
    }

    #[test]
    fn an_active_name_reports_already_registered_not_not_yet_claimable() {
        // Guards the ordering of the two checks in `register`: while the name
        // is active the caller must see `AlreadyRegistered`, and only after
        // expiry does the grace-period check take over.
        let f = Fixture::new(YEAR_SECONDS);
        let claimant = Address::generate(&f.env);

        let register_at = |now: u64| {
            f.client().try_register(
                &f.name,
                &claimant,
                &None::<String>,
                &None::<String>,
                &now,
                &(now + YEAR_SECONDS),
                &(now + YEAR_SECONDS + GRACE_PERIOD_SECONDS),
            )
        };

        assert!(matches!(
            register_at(f.expires_at),
            Err(Ok(RegistryError::AlreadyRegistered)),
        ));
        assert!(matches!(
            register_at(f.expires_at + 1),
            Err(Ok(RegistryError::NotYetClaimable)),
        ));
    }

    // ── Duration edge cases ───────────────────────────────────────────────

    #[test]
    fn minimum_duration_registration_honors_both_boundaries() {
        // Shortest supported registration: one year.
        let f = Fixture::new(YEAR_SECONDS);
        let client = f.client();

        assert_eq!(f.expires_at, REGISTERED_AT + YEAR_SECONDS);
        assert_eq!(client.name_state(&f.name, &f.expires_at), NameState::Active);
        assert_eq!(
            client.name_state(&f.name, &(f.expires_at + 1)),
            NameState::GracePeriod,
        );
        assert_eq!(
            client.name_state(&f.name, &f.grace_period_ends_at),
            NameState::GracePeriod,
        );
        assert_eq!(
            client.name_state(&f.name, &(f.grace_period_ends_at + 1)),
            NameState::Claimable,
        );
    }

    #[test]
    fn maximum_duration_registration_honors_both_boundaries() {
        // Longest supported registration: ten years. The boundaries must be
        // exact this far out, not merely approximately right.
        let f = Fixture::new(YEAR_SECONDS * MAX_REGISTRATION_YEARS);
        let client = f.client();

        assert_eq!(
            f.expires_at,
            REGISTERED_AT + YEAR_SECONDS * MAX_REGISTRATION_YEARS,
        );
        assert_eq!(client.name_state(&f.name, &f.expires_at), NameState::Active);
        assert_eq!(
            client.name_state(&f.name, &(f.expires_at + 1)),
            NameState::GracePeriod,
        );
        assert_eq!(
            client.name_state(&f.name, &f.grace_period_ends_at),
            NameState::GracePeriod,
        );
        assert_eq!(
            client.name_state(&f.name, &(f.grace_period_ends_at + 1)),
            NameState::Claimable,
        );
    }

    #[test]
    fn a_zero_length_grace_period_collapses_both_boundaries_onto_one_second() {
        // Degenerate but legal: `grace_period_ends_at == expires_at`. The name
        // must go straight from active to claimable with no grace second, and
        // neither comparison may swallow the other's boundary.
        let f = Fixture::with_grace(YEAR_SECONDS, 0);
        let client = f.client();

        assert_eq!(f.grace_period_ends_at, f.expires_at);
        assert_eq!(client.name_state(&f.name, &f.expires_at), NameState::Active);
        assert_eq!(
            client.name_state(&f.name, &(f.expires_at + 1)),
            NameState::Claimable,
            "with no grace period the name skips straight to claimable",
        );
    }

    #[test]
    fn boundaries_are_exact_for_a_one_second_grace_period() {
        // The tightest non-degenerate case: exactly one grace second exists,
        // and it is the second named by `grace_period_ends_at`.
        let f = Fixture::with_grace(YEAR_SECONDS, 1);
        let client = f.client();

        assert_eq!(client.name_state(&f.name, &f.expires_at), NameState::Active);
        assert_eq!(
            client.name_state(&f.name, &(f.expires_at + 1)),
            NameState::GracePeriod,
        );
        assert_eq!(f.grace_period_ends_at, f.expires_at + 1);
        assert_eq!(
            client.name_state(&f.name, &(f.grace_period_ends_at + 1)),
            NameState::Claimable,
        );
    }

    // ── Determinism ───────────────────────────────────────────────────────

    #[test]
    fn boundary_evaluation_is_deterministic_across_repeated_queries() {
        // Every lifecycle read takes `now_unix` as an argument rather than
        // reading the ledger clock, so the same timestamp must always yield
        // the same phase no matter how often it is asked.
        let f = Fixture::new(YEAR_SECONDS);
        let client = f.client();

        let checkpoints = [
            (f.expires_at - 1, NameState::Active),
            (f.expires_at, NameState::Active),
            (f.expires_at + 1, NameState::GracePeriod),
            (f.grace_period_ends_at - 1, NameState::GracePeriod),
            (f.grace_period_ends_at, NameState::GracePeriod),
            (f.grace_period_ends_at + 1, NameState::Claimable),
        ];

        for _ in 0..3 {
            for (now, expected) in &checkpoints {
                assert_eq!(
                    client.name_state(&f.name, now),
                    *expected,
                    "state at {now} should be stable across queries",
                );
            }
        }
    }
}
