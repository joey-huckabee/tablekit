"""Exercise tablekit's installed hooks using only synthetic temporary repositories."""
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


HOOKS = Path(__file__).resolve().parents[1] / ".githooks"


class Hooks(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.git("init", "-q", "-b", "feature/test")
        self.git("config", "core.autocrlf", "false")
        shutil.copytree(HOOKS, self.root / ".githooks")

    def git(self, *args, data=None):
        return subprocess.run(["git", "-C", str(self.root), *args], input=data,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True).stdout

    def stage(self, name, value):
        oid = self.git("hash-object", "-w", "--stdin", data=value).strip().decode()
        self.git("update-index", "--add", "--cacheinfo", "100644", oid, name)

    def hook(self, expected, name="pre-commit", *args):
        before_tree = self.git("write-tree")
        before_files = {p.relative_to(self.root): p.read_bytes()
                        for p in self.root.rglob("*")
                        if p.is_file() and ".git" not in p.relative_to(self.root).parts}
        result = subprocess.run(["sh", str(self.root / ".githooks" / name), *args],
                                cwd=self.root, capture_output=True)
        self.assertEqual(result.returncode, expected, result.stderr.decode(errors="replace"))
        self.assertEqual(before_tree, self.git("write-tree"))
        after_files = {p.relative_to(self.root): p.read_bytes()
                       for p in self.root.rglob("*")
                       if p.is_file() and ".git" not in p.relative_to(self.root).parts}
        self.assertEqual(before_files, after_files)
        return result

    def test_main_guard(self):
        self.git("symbolic-ref", "HEAD", "refs/heads/main")
        self.assertIn(b"refusing to commit on main", self.hook(1).stderr)

    def test_index_not_worktree(self):
        self.stage("partial.txt", b"bad\r\n")
        (self.root / "partial.txt").write_bytes(b"good\n")
        self.assertIn(b"staged text contains CRLF", self.hook(1).stderr)
        self.stage("partial.txt", b"good\n")
        (self.root / "partial.txt").write_bytes(b"bad\r\n")
        self.hook(0)

    def test_preserved_exceptions(self):
        self.stage(".gitattributes", b"* text=auto eol=lf\ntestdata/** -text\n*.bat text eol=crlf\n")
        self.stage("testdata/expected.csv", b"original\r\n")
        self.stage("run.bat", b"echo synthetic\r\n")
        self.stage("binary.bin", b"\0binary\r\n")
        self.hook(0)

    def test_commit_message_guard(self):
        message = self.root / "message"
        message.write_text("chore: synthetic check\n")
        self.hook(0, "commit-msg", str(message))
        message.write_text("chore: synthetic check\n\nCo-Authored-By: Claude <test@example.invalid>\n")
        self.hook(1, "commit-msg", str(message))


if __name__ == "__main__":
    unittest.main()
