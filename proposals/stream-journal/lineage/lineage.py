"""Lineage register: verify and render.

Every mutation in lineage.json names its parents: lines in pinned commits of
the source repositories, earlier register entries, or external sources. This
tool checks that each pinned line still says what the register claims it says,
that supersession links are consistent in both directions, and that every
"recorded in" anchor exists in the working tree. Then it renders LINEAGE.md.

    python3 lineage.py check  [--root DIR]      # exit 1 on any broken link
    python3 lineage.py render [--root DIR]      # check, then write ../LINEAGE.md
    python3 lineage.py resolve [--root DIR]     # fill in missing line numbers from `has`

--root is the directory holding the repository checkouts (workwarrior,
mobileos, ww-standard). It defaults to the parent of this repository.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "lineage.json")
OUT = os.path.join(HERE, "..", "LINEAGE.md")


def repo_root() -> str:
    return subprocess.run(["git", "-C", HERE, "rev-parse", "--show-toplevel"], capture_output=True, text=True, check=True).stdout.strip()


def blob(root: str, repo: str, commit: str, path: str) -> list[str]:
    r = subprocess.run(["git", "-C", os.path.join(root, repo), "show", f"{commit}:{path}"], capture_output=True, text=True)
    if r.returncode:
        raise FileNotFoundError(f"{repo}@{commit[:7]}:{path}: {r.stderr.strip()}")
    return r.stdout.splitlines()


def commit_of(doc, p) -> str:
    return p.get("commit") or doc["pins"][p["repo"]]["commit"]


def _anchors(doc):
    """Every pinned source anchor in the register, with the ID that owns it."""
    for e in doc["entries"]:
        for p in e["parents"]:
            yield e["id"], p
    for t in doc.get("terms", []):
        for p in t.get("was_src") or []:
            yield t["id"], p
    for x in doc.get("examples", []):
        for p in x.get("before_src") or []:
            yield x["id"], p


def resolve(doc, root):
    for owner, p in _anchors(doc):
        e = {"id": owner}
        if True:
            if "repo" in p and "line" not in p:
                lines = blob(root, p["repo"], commit_of(doc, p), p["path"])
                hits = [i + 1 for i, l in enumerate(lines) if p["has"] in l]
                if not hits:
                    raise ValueError(f"{e['id']}: {p['has']!r} not in {p['repo']}:{p['path']}")
                p["line"] = hits[0]
    return doc


def check(doc, root) -> list[str]:
    errs = []
    ids = [e["id"] for e in doc["entries"]]
    by = {e["id"]: e for e in doc["entries"]}
    if len(ids) != len(set(ids)):
        errs.append("duplicate ids: " + ", ".join(sorted({i for i in ids if ids.count(i) > 1})))
    statuses = set(doc["statuses"])
    ws = repo_root()
    for e in doc["entries"]:
        if e["status"] not in statuses:
            errs.append(f"{e['id']}: unknown status {e['status']!r}")
        if not e["parents"]:
            errs.append(f"{e['id']}: no parent; every mutation must descend from something")
        for p in e["parents"]:
            if "repo" in p:
                try:
                    lines = blob(root, p["repo"], commit_of(doc, p), p["path"])
                except FileNotFoundError as x:
                    errs.append(f"{e['id']}: {x}")
                    continue
                n = p.get("line")
                if not n or n > len(lines) or p["has"] not in lines[n - 1]:
                    errs.append(f"{e['id']}: {p['repo']}:{p['path']}:{n} does not contain {p['has']!r}")
            elif "entry" in p:
                if p["entry"] not in by:
                    errs.append(f"{e['id']}: parent entry {p['entry']} does not exist")
            elif "external" not in p:
                errs.append(f"{e['id']}: parent has no repo, entry or external source")
        sb = e.get("superseded_by")
        if sb:
            if sb not in by:
                errs.append(f"{e['id']}: superseded_by {sb} does not exist")
            elif e["id"] not in by[sb].get("supersedes", []):
                errs.append(f"{e['id']}: {sb} does not list it in supersedes")
            if e["status"] != "superseded":
                errs.append(f"{e['id']}: has superseded_by but status is {e['status']}")
        for s in e.get("supersedes", []):
            if s not in by or by[s].get("superseded_by") != e["id"]:
                errs.append(f"{e['id']}: supersedes {s}, but {s} does not point back")
        for r in e.get("recorded", []):
            path = os.path.join(ws, r["path"])
            if not os.path.exists(path):
                errs.append(f"{e['id']}: recorded path {r['path']} missing")
                continue
            with open(path, encoding="utf-8") as f:
                if r["has"] not in f.read():
                    errs.append(f"{e['id']}: {r['path']} does not contain {r['has']!r}")
    known = set(by) | {t["id"] for t in doc.get("terms", [])}
    for t in doc.get("terms", []):
        if not t.get("by"):
            errs.append(f"{t['id']}: no mutation changes this term")
        for i in t.get("by", []):
            if i not in known:
                errs.append(f"{t['id']}: changed by unknown {i}")
        if t.get("was") and not t.get("was_src"):
            errs.append(f"{t['id']}: has a current definition but no source")
    for owner, p in _anchors(doc):
        if owner in by:
            continue  # entries were checked above
        try:
            lines = blob(root, p["repo"], commit_of(doc, p), p["path"])
        except FileNotFoundError as x:
            errs.append(f"{owner}: {x}")
            continue
        n = p.get("line")
        if not n or n > len(lines) or p["has"] not in lines[n - 1]:
            errs.append(f"{owner}: {p['repo']}:{p['path']}:{n} does not contain {p['has']!r}")
    if doc.get("examples"):
        sys.path.insert(0, os.path.join(HERE, "..", "reference"))
        from notation import Notation, decode
        nt = Notation(os.path.join(HERE, "..", "reference", "vocab", "notation-0.1.json"))
        for x in doc["examples"]:
            for i in x.get("by", []):
                if i not in known:
                    errs.append(f"{x['id']}: refers to unknown {i}")
            if decode(x["run"], nt) != x["canonical"]:
                errs.append(f"{x['id']}: run form does not decode to its canonical lines")
    return errs


def _plink(doc, p) -> str:
    if "entry" in p:
        return f"[{p['entry']}](#{p['entry'].lower()})"
    if "external" in p:
        return f"*{p['external']}*"
    c = commit_of(doc, p)
    url = doc["pins"][p["repo"]]["url"]
    return f"[`{p['repo']}:{p['path']}:{p['line']}`]({url}/blob/{c}/{p['path']}#L{p['line']})"


def render(doc) -> str:
    st = doc["statuses"]
    out = [
        "# Lineage register",
        "",
        "<!-- GENERATED by lineage/lineage.py from lineage/lineage.json. Edit the JSON, then run `python3 lineage/lineage.py render`. -->",
        "",
        doc["preamble"],
        "",
        "## Pinned sources",
        "",
        "| Repository | Commit |",
        "|---|---|",
    ]
    for name, pin in doc["pins"].items():
        out.append(f"| {name} | [`{pin['commit'][:7]}`]({pin['url']}/tree/{pin['commit']}) |")
    out += ["", "## Status", "", "| Status | Meaning |", "|---|---|"]
    out += [f"| `{k}` | {v} |" for k, v in st.items()]
    counts = {}
    for e in doc["entries"]:
        counts[e["status"]] = counts.get(e["status"], 0) + 1
    out += ["", "**Totals:** " + ", ".join(f"{counts.get(k, 0)} {k}" for k in st if counts.get(k)), ""]

    st_of = {e["id"]: e["status"] for e in doc["entries"]}
    tid = {t["id"] for t in doc.get("terms", [])}

    def ref(i):
        return f"[{i}](#{i.lower()})" + (f" `{st_of[i]}`" if i in st_of else "")

    if doc.get("terms"):
        out += ["## Terms", "", "How each term is defined today, at a pinned line, and how this proposal redefines it. "
                "The decisions that change a term are listed with their status. Terms new to the standard have no current definition.", "",
                "| Term | Layer | Changed by |", "|---|---|---|"]
        for t in doc["terms"]:
            out.append(f"| [{t['term']}](#{t['id'].lower()}) | {t['layer']} | " + ", ".join(f"[{i}](#{i.lower()})" for i in t["by"]) + " |")
        out.append("")
        for t in doc["terms"]:
            out += [f"### {t['id']}", "", f"**{t['term']}** · {t['layer']}", ""]
            if t.get("was"):
                out.append(f"- **Was:** {t['was']} " + "; ".join(_plink(doc, p) for p in t["was_src"]))
            else:
                out.append("- **Was:** not defined")
            out.append(f"- **Now:** {t['now']}")
            out.append("- **Changed by:** " + ", ".join(ref(i) for i in t["by"]))
            out.append("")
    if doc.get("examples"):
        out += ["## Record coding", "", "The same occurrence coded today, in the journal draft (canonical, as trialled), in run form, and as a type under N11. "
                "`lineage.py check` decodes every run form below and confirms it reproduces the canonical lines.", "", doc.get("phase_note", ""), ""]
        for x in doc["examples"]:
            out += [f"### {x['id']}", "", f"**{x['title']}**", ""]
            if x.get("before"):
                out += ["Today (Stream v1), from " + "; ".join(_plink(doc, p) for p in x["before_src"]) + ":", "", "```", *x["before"], "```", ""]
            else:
                out += ["Today (Stream v1): no coding exists for this occurrence.", ""]
            out += ["Journal draft, canonical:", "", "```", *x["canonical"], "```", "", "Run form:", "", "```", *x["run"].rstrip("\n").split("\n"), "```", ""]
            out += [f"- **Type (N11):** {x['type']}", f"- **What changed:** {x['notes']}", "- **By:** " + ", ".join(ref(i) for i in x["by"]), ""]
    out += ["## Decisions", "", "Each decision is a mutation of a pinned source line or of an earlier decision.", ""]
    groups: dict[str, list] = {}
    for e in doc["entries"]:
        groups.setdefault(e["lineage"], []).append(e)
    for lin in doc["lineage_order"]:
        es = groups.get(lin, [])
        if not es:
            continue
        out += [f"### {lin}", "", doc["lineage_notes"].get(lin, ""), ""]
        for e in es:
            out += [f"#### {e['id']}", "", f"**{e['title']}** · `{e['status']}`" + (f" · cost: {e['cost']}" if e.get("cost") else ""), ""]
            out += ["- **Descends from:** " + "; ".join(_plink(doc, p) for p in e["parents"])]
            out += [f"- **Mutation:** {e['mutation']}"]
            if e.get("why"):
                out += [f"- **Why:** {e['why']}"]
            if e.get("supersedes"):
                out += ["- **Supersedes:** " + ", ".join(f"[{s}](#{s.lower()})" for s in e["supersedes"])]
            if e.get("superseded_by"):
                out += [f"- **Superseded by:** [{e['superseded_by']}](#{e['superseded_by'].lower()})"]
            if e.get("recorded"):
                out += ["- **Recorded in:** " + "; ".join(f"`{r['path'].split('/')[-1]}` ({r['has']})" for r in e["recorded"])]
            if e.get("note"):
                out += [f"- **Note:** {e['note']}"]
            out.append("")
    chains = [e for e in doc["entries"] if e.get("supersedes")]
    if chains:
        out += ["## Supersession chains", "", "| Was | Became | Why |", "|---|---|---|"]
        for e in chains:
            for s in e["supersedes"]:
                out.append(f"| [{s}](#{s.lower()}) | [{e['id']}](#{e['id'].lower()}) | {e.get('why', '')} |")
        out.append("")
    opens = [e for e in doc["entries"] if e["status"] == "open"]
    if opens:
        out += ["## Open", "", "| ID | Question |", "|---|---|"]
        out += [f"| [{e['id']}](#{e['id'].lower()}) | {e['title']} |" for e in opens]
        out.append("")
    return "\n".join(out)


def main():
    args = sys.argv[1:]
    cmd = args[0] if args else "check"
    root = args[args.index("--root") + 1] if "--root" in args else os.path.dirname(repo_root())
    with open(DATA, encoding="utf-8") as f:
        doc = json.load(f)
    if cmd == "resolve":
        resolve(doc, root)
        with open(DATA, "w", encoding="utf-8") as f:
            json.dump(doc, f, indent=1, ensure_ascii=False)
            f.write("\n")
    errs = check(doc, root)
    for x in errs:
        print("FAIL", x)
    n = sum(len(e["parents"]) for e in doc["entries"])
    print(f"{len(doc['entries'])} decisions, {len(doc.get('terms', []))} terms, {len(doc.get('examples', []))} coding examples, "
          f"{sum(1 for _ in _anchors(doc))} pinned links, {len(errs)} problems")
    if errs:
        sys.exit(1)
    if cmd == "render":
        with open(OUT, "w", encoding="utf-8") as f:
            f.write(render(doc) + "\n")
        print("wrote", os.path.relpath(OUT))


if __name__ == "__main__":
    main()
