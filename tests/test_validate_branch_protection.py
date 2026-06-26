import importlib.util
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = REPO_ROOT / "scripts" / "validate_branch_protection.py"
SPEC = importlib.util.spec_from_file_location("validate_branch_protection", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class ValidateBranchProtectionTests(unittest.TestCase):
    def test_passes_when_policy_is_meet(self) -> None:
        protection = {
            "required_pull_request_reviews": {
                "required_approving_review_count": 1,
            },
            "required_status_checks": {
                "strict": True,
                "contexts": ["build", "test"],
            },
            "allow_force_pushes": False,
            "allow_deletions": False,
        }

        with tempfile.TemporaryDirectory() as tmpdir:
            codeowners_path = Path(tmpdir) / "CODEOWNERS"
            codeowners_path.write_text("/contracts/ @team\n/packages/ @team\n/.github/ @team\n", encoding="utf-8")
            errors = MODULE.validate_branch_protection(protection)
            errors.extend(MODULE.validate_codeowners(codeowners_path))

        self.assertEqual(errors, [])

    def test_reports_missing_policy_requirements(self) -> None:
        protection = {
            "required_pull_request_reviews": {
                "required_approving_review_count": 0,
            },
            "required_status_checks": {
                "strict": False,
                "contexts": ["lint"],
            },
            "allow_force_pushes": True,
            "allow_deletions": True,
        }

        with tempfile.TemporaryDirectory() as tmpdir:
            codeowners_path = Path(tmpdir) / "CODEOWNERS"
            codeowners_path.write_text("/docs/ @team\n", encoding="utf-8")
            errors = MODULE.validate_branch_protection(protection)
            errors.extend(MODULE.validate_codeowners(codeowners_path))

        self.assertTrue(any("required_pull_request_reviews" in error for error in errors))
        self.assertTrue(any("required_status_checks" in error for error in errors))
        self.assertTrue(any("allow_force_pushes" in error for error in errors))
        self.assertTrue(any("CODEOWNERS must cover required path" in error for error in errors))


if __name__ == "__main__":
    unittest.main()
