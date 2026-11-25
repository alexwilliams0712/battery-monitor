"""Calculate the package version for a merged commit on main."""

import subprocess
import tomllib


def git(*args):
    return subprocess.check_output(["git", *args], text=True).strip()


def version(commit):
    manifest = tomllib.loads(git("show", f"{commit}:Cargo.toml"))
    return manifest["package"]["version"]


base = version("HEAD")
anchor = "HEAD"
for commit in git("log", "--first-parent", "--format=%H", "--", "Cargo.toml").splitlines():
    if version(commit) != base:
        break
    anchor = commit

major, minor, patch = base.split(".")
distance = git("rev-list", "--first-parent", "--count", f"{anchor}..HEAD")
print(f"{major}.{minor}.{int(patch) + int(distance)}")
