# Pull Request: Add Comprehensive Penetration Testing Checklist

## Title

docs(security): Add comprehensive penetration testing checklist for xlm-ns contracts

## Description

This PR introduces a structured penetration testing checklist (`docs/security/pentest-checklist.md`) to systematically enumerate and document security attack scenarios across all seven xlm-ns contracts before mainnet deployment.

## Motivation

Before mainnet deployment, xlm-ns contracts require a comprehensive security audit covering all known attack vectors. Without a systematic penetration testing checklist, auditors and internal security reviews may miss critical scenarios, leading to vulnerabilities being discovered in production where exploitation has real financial consequences.

This checklist ensures:

- **Systematic Coverage** — All 7 contracts are assessed for the same attack categories
- **Clear Severity Ratings** — Each scenario is rated CRITICAL, HIGH, MEDIUM, or LOW
- **Reproducible Test Procedures** — Auditors and developers can execute consistent verification steps
- **Reusability** — The checklist template can be adapted for each release cycle

## Changes

### Added Files

- `docs/security/pentest-checklist.md` — Complete penetration testing checklist with 30+ attack scenarios

### Scope

The checklist covers all seven xlm-ns contracts and includes:

#### **Contracts Covered (7)**

1. **Registry** — Ownership, lifecycle, and state mutation
2. **Registrar** — Issuance, renewal, and payment processing
3. **Resolver** — Forward/reverse resolution and record management
4. **Auction** — Vickrey-style settlement and bid management
5. **Subdomain** — Delegation and nested namespace creation
6. **NFT** — Tokenized ownership and transfers
7. **Bridge** — Cross-chain payload construction and delivery

#### **Attack Categories**

1. **Access Control (AC)** — 8 scenarios covering unauthorized function calls and privilege escalation
2. **Economic Attacks (EC)** — 8 scenarios covering front-running, fee evasion, and auction manipulation
3. **State Manipulation (ST)** — 7 scenarios covering reentrancy, consistency drift, and storage overflow
4. **Denial of Service (DO)** — 5 scenarios covering gas griefing, storage bloat, and unbounded loops
5. **Bridge-Specific (BRG)** — 6 scenarios covering cross-chain replay, message forgery, and relay manipulation
6. **Cross-Contract (XC)** — 2 scenarios covering lifecycle consistency and payment atomicity

### Scenario Format

Each scenario includes:

- **Severity Level** — CRITICAL, HIGH, MEDIUM to prioritize remediation
- **Category** — Attack classification for systematic auditing
- **Description** — Concise summary of the attack vector
- **Expected Behavior** — What should happen to prevent the attack
- **Test Procedure** — Step-by-step instructions to verify the defense

### Example: Registry Unauthorized Owner Mutation (AC-REG-001)

```
Severity: CRITICAL
Category: Access Control

Attack: An attacker attempts to transfer a name without ownership verification.

Expected Behavior:
- All mutating operations must verify the caller is the current owner.
- Non-owners receive AuthorizationError.

Test Procedure:
1. Register a name as owner A.
2. Attempt to transfer ownership as owner B.
3. Verify the operation fails with AuthorizationError.
4. Confirm name ownership remains with A.
```

## Coverage Summary

| Contract       | AC     | EC    | ST     | DO    | BRG/XC | Total  |
| -------------- | ------ | ----- | ------ | ----- | ------ | ------ |
| Registry       | 2      | 0     | 2      | 1     | 0      | 5      |
| Registrar      | 1      | 3     | 1      | 1     | 0      | 6      |
| Resolver       | 2      | 0     | 2      | 1     | 0      | 5      |
| Auction        | 2      | 2     | 1      | 1     | 0      | 6      |
| Subdomain      | 2      | 0     | 1      | 1     | 0      | 4      |
| NFT            | 3      | 0     | 1      | 1     | 0      | 5      |
| Bridge         | 2      | 0     | 0      | 0     | 6      | 8      |
| Cross-Contract | 0      | 0     | 2      | 0     | 0      | 2      |
| **Total**      | **14** | **5** | **10** | **5** | **6**  | **41** |

## Acceptance Criteria

- [x] Checklist covers all 7 contracts with contract-specific scenarios
- [x] Attack categories include access control, economic, state, and DoS
- [x] Each scenario has severity rating and test procedure
- [x] Bridge-specific attack scenarios documented (6 scenarios)
- [x] Checklist includes cross-contract interaction scenarios
- [x] Template is reusable for future release audits
- [x] Review and approval section provided for audit team sign-off

## How to Use

### For Security Auditors

1. Clone this branch
2. Read `docs/security/pentest-checklist.md`
3. For each scenario, follow the "Test Procedure" column
4. Use the "Test Execution Template" section as a guide
5. Document pass/fail results in the "Reviewed By" section
6. Sign off in the "Approval" section when complete

### For Engineers

- Reference this checklist during contract development
- Add new scenarios as new features are added
- Use test procedures as the basis for automated test cases
- Link automated tests to specific checklist scenarios

### For Future Releases

- Update version number in Revision History
- Add new scenarios discovered in previous audits
- Reuse test procedures as regression tests

## Related Issues

- Depends on #76 (threat models inform attack scenarios)
- Complements #95 (formal verification covers invariant proofs)
- Related to #92 (integration tests can automate some pentest scenarios)

## Testing

The checklist itself does not require code changes to pass; however:

- Manual verification of each scenario is the responsibility of the security audit team
- Scenarios can be automated incrementally via integration tests (see #92)
- A subset of high-severity scenarios should be included in release QA

## Documentation

- Checklist is self-documented with clear procedures
- Uses consistent severity and category taxonomy
- Includes approval and review sign-off for audit trail

## Backward Compatibility

N/A — This is documentation-only. No contract logic changes.

## Deployment Notes

- This checklist should be reviewed and approved by 2+ team members before mainnet deployment
- Consider running the full pentest at each major version release
- Update the checklist as new attack vectors are discovered

## Reviewers

@security-team @engineering-lead

---

## Summary

This PR establishes a foundation for systematic security review of xlm-ns contracts before mainnet. It provides a reusable template for penetration testing, ensures consistent audit coverage, and creates an audit trail for security sign-off. The 41 documented attack scenarios span all contract domains and attack categories, enabling confident deployment to production.
