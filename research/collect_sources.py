"""Read-only public-source collection. Does not build or execute downloaded code."""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
import subprocess
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parent
CACHE = ROOT / "cache"
HEADERS = {"User-Agent": "MnemosyneOS-research", "Accept": "application/vnd.github+json"}

def fetch(url):
    req = urllib.request.Request(url, headers=HEADERS)
    with urllib.request.urlopen(req, timeout=40) as response:
        return response.read()

def api(path):
    return json.loads(fetch("https://api.github.com/" + path))

def inventory(repo):
    folder = CACHE / repo.replace("/", "__")
    folder.mkdir(parents=True, exist_ok=True)
    existing = folder / "inventory.json"
    if existing.exists():
        return json.loads(existing.read_text(encoding="utf-8"))
    try:
        meta = api("repos/" + repo)
        commit = api("repos/" + repo + "/commits/" + meta["default_branch"])
        tree = api("repos/" + repo + "/git/trees/" + commit["sha"] + "?recursive=1")
    except urllib.error.HTTPError as error:
        if error.code != 403:
            raise
        gitdir = folder / "git-metadata"
        if not gitdir.exists():
            subprocess.run(["git", "clone", "--filter=blob:none", "--depth=1", "--no-checkout",
                            "https://github.com/" + repo + ".git", str(gitdir)], check=True,
                           capture_output=True, timeout=90)
        def git(*args):
            return subprocess.check_output(["git", "-C", str(gitdir), *args], text=True).strip()
        sha = git("rev-parse", "HEAD")
        branch = git("branch", "--show-current")
        meta = {"full_name": repo, "html_url": "https://github.com/" + repo,
                "default_branch": branch, "archived": None}
        commit = {"sha": sha, "commit": {"committer": {"date": git("show", "-s", "--format=%cI", "HEAD")}}}
        rows = git("ls-tree", "-r", "HEAD").splitlines()
        tree = {"truncated": False, "tree": []}
        for row in rows:
            left, path = row.split("\t", 1)
            mode, typ, oid = left.split()
            tree["tree"].append({"type": typ, "path": path, "sha": oid})
    result = {
        "repo": meta["full_name"], "url": meta["html_url"],
        "sha": commit["sha"], "commit_date": commit["commit"]["committer"]["date"],
        "default_branch": meta["default_branch"], "archived": meta["archived"],
        "license_api": meta.get("license"), "tree_truncated": tree.get("truncated"),
        "retrieved_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "files": [{"path": x["path"], "size": x.get("size"), "sha": x["sha"]}
                  for x in tree["tree"] if x["type"] == "blob"]
    }
    existing.write_text(json.dumps(result, indent=2), encoding="utf-8")
    return result

def source(repo, path):
    inv = inventory(repo)
    if path not in {x["path"] for x in inv["files"]}:
        raise ValueError("Path not in recorded tree: " + path)
    target = CACHE / repo.replace("/", "__") / "source" / path
    target.parent.mkdir(parents=True, exist_ok=True)
    url = "https://raw.githubusercontent.com/" + inv["repo"] + "/" + inv["sha"] + "/" + path
    data = target.read_bytes() if target.exists() else fetch(url)
    target.write_bytes(data)
    item = {"repo": inv["repo"], "sha": inv["sha"], "path": path,
            "url": "https://github.com/" + inv["repo"] + "/blob/" + inv["sha"] + "/" + path,
            "sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
    return item

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=["inventory", "files"])
    parser.add_argument("items", nargs="+")
    args = parser.parse_args()
    if args.mode == "inventory":
        for repo in args.items:
            try:
                inv = inventory(repo)
                print(json.dumps({k: inv[k] for k in ["repo", "sha", "commit_date", "archived", "tree_truncated"]}), flush=True)
            except Exception as error:
                print(json.dumps({"repo": repo, "error": str(error)}), flush=True)
    else:
        pairs = [(repo, path) for name in args.items
                 for repo, paths in json.loads(Path(name).read_text(encoding="utf-8")).items()
                 for path in paths]
        result = []
        with concurrent.futures.ThreadPoolExecutor(max_workers=4) as executor:
            pending = {executor.submit(source, r, p): (r, p) for r, p in pairs}
            for future in concurrent.futures.as_completed(pending):
                repo, path = pending[future]
                try:
                    result.append(future.result())
                except Exception as error:
                    result.append({"repo": repo, "path": path, "error": str(error)})
        output = ROOT / "source-manifest.json"
        previous = json.loads(output.read_text(encoding="utf-8")) if output.exists() else []
        merged = {(x["repo"], x["path"]): x for x in previous}
        merged.update({(x["repo"], x["path"]): x for x in result})
        output.write_text(json.dumps(list(merged.values()), indent=2), encoding="utf-8")
        print(json.dumps({"fetched": sum("error" not in x for x in result),
                          "errors": [x for x in result if "error" in x]}), flush=True)

if __name__ == "__main__":
    main()
