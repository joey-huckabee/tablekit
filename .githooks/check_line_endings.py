"""Reject CRLF in staged text, respecting staged Git attributes; never edit files."""
import os
import subprocess
import sys


def git(*args, data=None):
    return subprocess.run(
        ["git", *args], input=data, stdout=subprocess.PIPE, check=True
    ).stdout


def check():
    # Raw records carry exact index OIDs, including unborn HEAD and partial staging.
    entries = git(
        "diff", "--cached", "--raw", "--no-abbrev", "--no-renames",
        "--diff-filter=ACM", "-z",
    ).split(b"\0")
    candidates = []
    for i in range(0, len(entries) - 1, 2):
        _, mode, _, oid, _ = entries[i].split()
        if mode in (b"100644", b"100755"):
            candidates.append((entries[i + 1], oid))
    if not candidates:
        return 0
    attrs = git(
        "check-attr", "--cached", "-z", "--stdin", "text", "eol",
        data=b"".join(path + b"\0" for path, _ in candidates),
    ).split(b"\0")
    rules = {}
    for i in range(0, len(attrs) - 1, 3):
        path, key, value = attrs[i:i + 3]
        rules.setdefault(path, {})[key] = value
    failed = []
    # One Git process; bounded blob reads, no checkout reads or per-file processes.
    with subprocess.Popen(
        ["git", "cat-file", "--batch"], stdin=subprocess.PIPE, stdout=subprocess.PIPE
    ) as blobs:
        try:
            for path, oid in candidates:
                rule = rules[path]
                if rule[b"text"] == b"unset" or rule[b"eol"] == b"crlf":
                    continue  # Explicit binary/preserved-byte or CRLF exception.
                blobs.stdin.write(oid + b"\n")
                blobs.stdin.flush()
                actual, kind, size = blobs.stdout.readline().split()
                if actual != oid or kind != b"blob":
                    raise ValueError("invalid staged blob response")
                remaining = int(size)
                first = blobs.stdout.read(min(8000, remaining))
                remaining -= len(first)
                binary = rule[b"text"] != b"set" and b"\0" in first
                crlf = b"\r\n" in first
                tail = first[-1:]
                while remaining:
                    chunk = blobs.stdout.read(min(65536, remaining))
                    if not chunk:
                        raise ValueError("truncated staged blob response")
                    crlf |= b"\r\n" in tail + chunk
                    tail = chunk[-1:]
                    remaining -= len(chunk)
                if blobs.stdout.read(1) != b"\n":
                    raise ValueError("invalid staged blob delimiter")
                if crlf and not binary:
                    failed.append(path)
        finally:
            blobs.stdin.close()
            blobs.wait()
        if blobs.returncode:
            raise ValueError("git cat-file failed")
    if failed:
        print("pre-commit: staged text contains CRLF; save as LF and stage again:", file=sys.stderr)
        for path in failed:
            print("  " + repr(os.fsdecode(path)), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    try:
        sys.exit(check())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print("pre-commit: staged LF check failed: " + str(error), file=sys.stderr)
        sys.exit(1)
