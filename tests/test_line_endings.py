"""Synthetic Git-index probes; no project/reference files are modified."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECK = Path(__file__).resolve().parents[1] / ".githooks/check_line_endings.py"


class LineEndings(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q")
        self.git("config", "core.autocrlf", "false")

    def git(self, *args, data=None):
        return subprocess.run(["git", "-C", str(self.root), *args], input=data,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True).stdout

    def stage(self, name, data, mode="100644"):
        oid = self.git("hash-object", "-w", "--stdin", data=data).strip().decode()
        self.git("update-index", "--add", "--cacheinfo", mode, oid, name)

    def check(self, expected):
        before = self.git("write-tree")
        result = subprocess.run([sys.executable, str(CHECK)], cwd=self.root,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.assertEqual(result.returncode, expected, result.stderr.decode(errors="replace"))
        self.assertEqual(before, self.git("write-tree"))
        return result

    def test_empty_and_lf(self):
        self.check(0)
        self.stage("empty", b"")
        self.stage("fine.txt", b"one\ntwo\n")
        self.check(0)

    def test_crlf_and_mixed_reject(self):
        for value in [b"one\r\n", b"one\ntwo\r\n"]:
            self.stage("bad.txt", value)
            self.check(1)

    def test_index_not_worktree(self):
        self.stage("partial.txt", b"bad\r\n")
        (self.root / "partial.txt").write_bytes(b"good\n")
        self.check(1)
        self.stage("partial.txt", b"good\n")
        (self.root / "partial.txt").write_bytes(b"bad\r\n")
        self.check(0)

    def test_staged_attributes_and_preserved_exceptions(self):
        self.stage(".gitattributes", b"* text=auto eol=lf\nkeep/* -text\n*.bat text eol=crlf\n")
        (self.root / ".gitattributes").write_bytes(b"* text eol=lf\n")
        self.stage("keep/reference.txt", b"original\r\n")
        self.stage("run.bat", b"echo example\r\n")
        self.check(0)
        self.stage("normal.txt", b"not exempt\r\n")
        self.check(1)

    def test_binary_and_forced_text(self):
        self.stage("image.bin", b"\0binary\r\n")
        self.check(0)
        self.stage(".gitattributes", b"*.bin text\n")
        self.check(1)

    def test_chunk_boundary_and_unusual_paths(self):
        self.stage("space and unicode-\u00e9.txt", b"a" * 7999 + b"\r\n")
        self.stage("-leading.txt", b"a" * (8000 + 65535) + b"\r\n")
        result = self.check(1)
        self.assertIn(b"-leading.txt", result.stderr)
        self.assertIn(b"space and unicode", result.stderr)

    def test_symlink_skipped(self):
        self.stage("link", b"target\r\n", "120000")
        self.check(0)

    def test_committed_and_deleted_crlf_not_rechecked(self):
        self.stage("old.txt", b"old\r\n")
        self.git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
                 "-c", "core.hooksPath=/dev/null", "commit", "-qm", "fixture")
        self.check(0)
        self.git("update-index", "--force-remove", "old.txt")
        self.check(0)


if __name__ == "__main__":
    unittest.main()
