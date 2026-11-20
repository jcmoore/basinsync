#!/usr/bin/env python3
"""Capture/recheck GitHub prompt requests; never run agents or write to GitHub."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


class RequestError(Exception):
    pass


def run(*args: str) -> str:
    result = subprocess.run(args, text=True, capture_output=True, check=False)
    if result.returncode:
        raise RequestError(result.stderr.strip() or f"{args[0]} failed")
    return result.stdout.strip()


def issue_target(url: str) -> tuple[str, int]:
    match = re.fullmatch(
        r"https://github\.com/([\w.-]+/[\w.-]+)/issues/([1-9][0-9]*)/?", url
    )
    if not match:
        raise RequestError("Use a full https://github.com/OWNER/REPO/issues/NUMBER URL.")
    return match[1], int(match[2])


def origin_repository(remote: str) -> str:
    match = re.fullmatch(r"(?:https://github\.com/|git@github\.com:|ssh://git@github\.com/)([\w.-]+/[\w.-]+?)(?:\.git)?/?", remote)
    if not match:
        raise RequestError("origin must be a github.com HTTPS or SSH repository URL.")
    return match[1]


def gh_json(endpoint: str, paginate: bool = False):
    args = ["gh", "api", "--hostname", "github.com", "--method", "GET", endpoint]
    if paginate:
        args += ["--paginate", "--slurp"]
    return json.loads(run(*args))


def fetch_request(repository: str, number: int, fixture: Path | None = None) -> dict:
    if fixture:
        data = json.loads(fixture.read_text())
    else:
        metadata = gh_json(f"repos/{repository}")
        if not metadata["has_issues"]:
            raise RequestError(f"Issues are disabled for {repository}; enable them before a live test.")
        issue = gh_json(f"repos/{repository}/issues/{number}")
        pages = gh_json(f"repos/{repository}/issues/{number}/comments?per_page=100", paginate=True)
        data = {"issue": issue, "comments": [comment for page in pages for comment in page]}
    return normalize(data, repository, number)


def normalize(data: dict, repository: str, number: int) -> dict:
    issue = data["issue"]
    target = issue_target(issue["html_url"])
    if target[0].lower() != repository.lower() or target[1] != number:
        raise RequestError("The returned issue does not match the requested repository and number.")
    if "pull_request" in issue:
        raise RequestError("Select an issue, not a pull request.")
    labels = sorted(label["name"] for label in issue["labels"])
    if "prompt-request" not in labels:
        raise RequestError("The issue must have the prompt-request label.")
    if issue["state"] != "open":
        raise RequestError("The request is closed; select an open request.")
    comments = sorted(data["comments"], key=lambda comment: comment["id"])
    if len(comments) != issue["comments"]:
        raise RequestError("Comment count changed or comments are incomplete; fetch the request again.")
    return {
        "issue": {
            "url": issue["html_url"],
            "number": number,
            "author": issue["user"]["login"],
            "title": issue["title"],
            "body": issue["body"] or "",
            "state": issue["state"],
            "updated_at": issue["updated_at"],
            "labels": labels,
            "assignees": sorted(user["login"] for user in issue["assignees"]),
        },
        "comments": [
            {
                "id": comment["id"],
                "url": comment["html_url"],
                "author": comment["user"]["login"],
                "body": comment["body"] or "",
                "updated_at": comment["updated_at"],
            }
            for comment in comments
        ],
    }


def digest(request: dict) -> str:
    encoded = json.dumps(request, sort_keys=True, separators=(",", ":"), ensure_ascii=False)
    return hashlib.sha256(encoded.encode()).hexdigest()


def field(body: str, label: str) -> str:
    # GitHub issue forms render each field beneath an h3 heading.
    matches = re.findall(r"^### " + re.escape(label) + r"\r?\n+(.*?)(?=^### |\Z)", body, re.M | re.S)
    if len(matches) > 1:
        raise RequestError(f"Duplicate {label} fields; clarify attribution before proceeding.")
    return matches[0].strip() if matches else ""


def attribution(request: dict) -> str:
    issue = request["issue"]
    preference = field(issue["body"], "Attribution preference")
    if preference in ("", "_No response_", "No additional credit"):
        return "No additional personal credit. Retain the source issue link.\n"
    if preference == "Acknowledge my GitHub account":
        return f"Prompt contribution: @{issue['author']} ({issue['url']}).\n"
    if preference != "Commit co-authorship (opt in)":
        raise RequestError("Unknown attribution preference; clarify it before proceeding.")
    identity = field(issue["body"], "Co-author identity")
    if not re.fullmatch(r"[^<>\r\n]+ <[^<>\s@]+@[^<>\s@]+>", identity) or any(ord(c) < 32 for c in identity):
        raise RequestError("Co-authorship needs the requester's chosen Name <GitHub-associated email> on one line.")
    return f"Subject to project authorship policy and any later requester clarification:\nCo-authored-by: {identity}\n"


def fenced(body: str) -> str:
    fence = "`" * max(3, max((len(m[0]) + 1 for m in re.finditer(r"`+", body)), default=3))
    return f"{fence}text\n{body}\n{fence}"


def render(snapshot: dict) -> str:
    request = snapshot["request"]
    issue = request["issue"]
    text = [
        "# Prompt request snapshot",
        f"Mode: {snapshot['mode']} (fixture mode is an offline preview only).",
        f"Source: {issue['url']}",
        f"Snapshot SHA-256: {snapshot['sha256']}",
        f"Checkout base commit: {snapshot['base_commit']}",
        "The maintainer must establish the accepted scope; this file records fetched contribution material.",
        "## Issue",
        fenced(f"Title: {issue['title']}\nAuthor: {issue['author']}\n\n{issue['body']}"),
        "## Discussion",
    ]
    for comment in request["comments"]:
        text += [f"Comment {comment['id']}: {comment['url']}", fenced(f"Author: {comment['author']}\n\n{comment['body']}")]
    if not request["comments"]:
        text.append("No comments at capture time.")
    text += ["## Attribution from the issue form", fenced(attribution(request).rstrip()), "Review later requester clarifications before using this credit."]
    return "\n\n".join(text) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare = commands.add_parser("prepare", help="Save an open prompt request and all comments locally")
    prepare.add_argument("issue_url")
    prepare.add_argument("--output", type=Path, help="New directory; existing output is never overwritten")
    prepare.add_argument("--fixture", type=Path, help="Offline preview using a JSON issue/comments fixture")
    check = commands.add_parser("check", help="Detect request changes since a saved snapshot")
    check.add_argument("snapshot", type=Path)
    check.add_argument("--fixture", type=Path, help="Required for checking an offline fixture snapshot")
    args = parser.parse_args(argv)
    try:
        root = Path(run("git", "rev-parse", "--show-toplevel"))
        repository = origin_repository(run("git", "remote", "get-url", "origin"))
        if args.command == "prepare":
            target_repository, number = issue_target(args.issue_url)
            if target_repository.lower() != repository.lower():
                raise RequestError(f"Issue repository differs from origin ({repository}); use the intended fork's issue.")
            request = fetch_request(repository, number, args.fixture)
            checksum = digest(request)
            snapshot = {"schema_version": 1, "repository": repository, "mode": "fixture" if args.fixture else "github", "base_commit": run("git", "rev-parse", "HEAD"), "sha256": checksum, "request": request}
            credit = attribution(request)
            markdown = render(snapshot)
            output = args.output or root / ".local" / "prompt-requests" / f"{number}-{checksum[:12]}"
            output.mkdir(parents=True, exist_ok=False)
            (output / "snapshot.json").write_text(json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n")
            (output / "request.md").write_text(markdown)
            (output / "attribution.txt").write_text(credit)
            print(f"Saved {snapshot['mode']} snapshot: {output.resolve()}\nSHA-256: {checksum}")
        else:
            snapshot = json.loads(args.snapshot.read_text())
            if snapshot["schema_version"] != 1 or snapshot["sha256"] != digest(snapshot["request"]):
                raise RequestError("Snapshot version or digest is invalid.")
            if snapshot["repository"].lower() != repository.lower():
                raise RequestError("Snapshot repository differs from this checkout's origin.")
            if snapshot["mode"] not in ("fixture", "github") or (snapshot["mode"] == "fixture") != bool(args.fixture):
                raise RequestError("Fixture and live checks cannot be mixed; use --fixture only for fixture snapshots.")
            target_repository, number = issue_target(snapshot["request"]["issue"]["url"])
            if target_repository.lower() != repository.lower():
                raise RequestError("Snapshot issue differs from this checkout's origin.")
            current = fetch_request(repository, number, args.fixture)
            if digest(current) != snapshot["sha256"]:
                print("Request changed. Capture a new snapshot and reconcile it with the accepted scope.", file=sys.stderr)
                return 1
            print(f"Request unchanged ({snapshot['mode']}); SHA-256: {snapshot['sha256']}")
        return 0
    except (RequestError, OSError, ValueError, KeyError, TypeError) as exc:
        print(f"prompt-request: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
