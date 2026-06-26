#!/usr/bin/env python3
import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Dict, List, Sequence, Tuple

REQUIRED_STATUS_CHECKS = ("build", "test")
REQUIRED_CODEOWNERS_PATHS = (".github", "contracts", "packages")


def normalize_pattern(pattern: str) -> str:
    pattern = pattern.strip()
    if not pattern or pattern.startswith("#"):
        return ""
    if pattern.startswith("!"):
        return pattern
    if pattern.startswith("/"):
        pattern = pattern[1:]
    if pattern.endswith("/"):
        pattern = pattern[:-1]
    return pattern


def load_protection_data(repo: str, branch: str, protection_file: str | None = None) -> Dict[str, object]:
    if protection_file:
        with open(protection_file, "r", encoding="utf-8") as handle:
            return json.load(handle)

    command = ["gh", "api", f"repos/{repo}/branches/{branch}/protection"]
    completed = subprocess.run(command, capture_output=True, text=True, check=False)
    if completed.returncode != 0:
        error_output = completed.stderr.strip() or completed.stdout.strip() or "gh api returned a non-zero exit code"
        raise RuntimeError(f"Unable to fetch branch protection data: {error_output}")

    try:
        return json.loads(completed.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"Unable to parse branch protection JSON: {exc}") from exc


def collect_status_check_contexts(required_status_checks: Dict[str, object]) -> List[str]:
    contexts: List[str] = []
    raw_contexts = required_status_checks.get("contexts") or []
    if isinstance(raw_contexts, list):
        contexts.extend([str(item) for item in raw_contexts if str(item)])

    raw_checks = required_status_checks.get("checks") or []
    if isinstance(raw_checks, list):
        for item in raw_checks:
            if isinstance(item, dict):
                context = item.get("context")
                if context:
                    contexts.append(str(context))
            elif item:
                contexts.append(str(item))

    return contexts


def validate_branch_protection(protection: Dict[str, object]) -> List[str]:
    errors: List[str] = []

    required_reviews = protection.get("required_pull_request_reviews")
    if not isinstance(required_reviews, dict):
        errors.append("required_pull_request_reviews must be configured")
    else:
        review_count = required_reviews.get("required_approving_review_count", 0)
        if int(review_count) < 1:
            errors.append(
                "required_pull_request_reviews.required_approving_review_count must be at least 1"
            )

    required_status_checks = protection.get("required_status_checks")
    if not isinstance(required_status_checks, dict):
        errors.append("required_status_checks must be configured")
    else:
        if not bool(required_status_checks.get("strict", False)):
            errors.append("required_status_checks.strict must be true")

        contexts = collect_status_check_contexts(required_status_checks)
        missing_checks = [name for name in REQUIRED_STATUS_CHECKS if name not in contexts]
        if missing_checks:
            errors.append(
                "required_status_checks must include the following contexts: "
                + ", ".join(REQUIRED_STATUS_CHECKS)
            )

    if bool(protection.get("allow_force_pushes", True)):
        errors.append("allow_force_pushes must be false")

    if bool(protection.get("allow_deletions", True)):
        errors.append("allow_deletions must be false")

    return errors


def validate_codeowners(codeowners_path: Path, required_paths: Sequence[str] = REQUIRED_CODEOWNERS_PATHS) -> List[str]:
    errors: List[str] = []
    if not codeowners_path.exists():
        errors.append(f"CODEOWNERS file not found at {codeowners_path}")
        return errors

    patterns = []
    for raw_line in codeowners_path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        patterns.append(line.split()[0])

    for required_path in required_paths:
        normalized_required_path = required_path.strip().strip("/")
        found = False
        for pattern in patterns:
            normalized_pattern = normalize_pattern(pattern)
            if not normalized_pattern:
                continue
            if normalized_pattern == normalized_required_path or normalized_pattern.startswith(
                normalized_required_path + "/"
            ):
                found = True
                break
        if not found:
            errors.append(f"CODEOWNERS must cover required path {required_path}")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate repository branch protection policy")
    parser.add_argument("--repo", default=os.getenv("GITHUB_REPOSITORY", ""), help="GitHub repository name")
    parser.add_argument("--branch", default=os.getenv("BRANCH", "main"), help="Branch to inspect")
    parser.add_argument(
        "--protection-file",
        default=None,
        help="Optional path to a local JSON file containing branch protection data",
    )
    parser.add_argument(
        "--codeowners-file",
        default=os.getenv("CODEOWNERS_FILE", "CODEOWNERS"),
        help="Path to the CODEOWNERS file",
    )
    args = parser.parse_args()

    if not args.repo and not args.protection_file:
        parser.error("--repo is required unless --protection-file is provided")

    try:
        protection = load_protection_data(args.repo, args.branch, args.protection_file)
    except RuntimeError as exc:
        print(f"Branch protection policy check failed: {exc}")
        return 1

    errors = validate_branch_protection(protection)
    errors.extend(validate_codeowners(Path(args.codeowners_file)))

    if errors:
        print("Branch protection policy check failed:")
        for error in errors:
            print(f"- {error}")
        return 1

    print("Branch protection policy check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
