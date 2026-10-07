from pathlib import Path
import tempfile
import unittest

from laya_runtime_config import resolve_environment, resolve_snapshot


class LayaRuntimeConfigTests(unittest.TestCase):
    def test_linux_defaults_are_portable(self):
        with tempfile.TemporaryDirectory() as directory:
            resolved = resolve_environment({}, windows=False, home=Path(directory))
            self.assertEqual(resolved["HF_HOME"], str(Path(directory) / ".cache" / "huggingface"))
            self.assertEqual(resolved["HUGGINGFACE_HUB_CACHE"], str(Path(directory) / ".cache" / "huggingface" / "hub"))
            snapshot = resolve_snapshot(resolved)
            self.assertIn("models--convaiinnovations--laya", str(snapshot))

    def test_explicit_cache_and_snapshot_win(self):
        resolved = resolve_environment(
            {
                "HF_HOME": "/models/hf",
                "HUGGINGFACE_HUB_CACHE": "/models/hf-cache",
                "TMPDIR": "/tmp/jev",
            },
            windows=False,
        )
        self.assertEqual(resolved["HF_HOME"], "/models/hf")
        self.assertEqual(resolved["HUGGINGFACE_HUB_CACHE"], "/models/hf-cache")
        self.assertEqual(resolved["TMP"], "/tmp/jev")
        self.assertEqual(resolve_snapshot({"LOCAL_LAYA_SNAPSHOT": "~/laya-snapshot"}), Path.home() / "laya-snapshot")


if __name__ == "__main__":
    unittest.main()
