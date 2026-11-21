#!/usr/bin/env python3
"""Vendors Michael Lane's ML Studio plugins (https://github.com/Dorfhaus/ML-Studio, GPL-3.0) into vendor/ml-studio/.

    tools/vendor-ml-studio.py /path/to/ML-Studio [crate ...]

Each crate's `src/` (and the folders its build needs) is copied as it is, its manifest is rewritten as Honeydew's own
(path dependencies on vendor/lemon-studio, nih-plug by path, the feature `embed`), the plugin editors are embedded as
tools/sync-lemon-plugins.sh embeds Lemon Studio's (same template) and `lemon_small_fx` (ML's own, much changed copy of
Lemon Studio's) is `ml_small_fx`, so that both exist beside each other. vendor/ml-studio/README.md says which commit.
"""
import os, re, shutil, subprocess, sys

fork = sys.argv[1]
root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
dest = os.path.join(root, "vendor", "ml-studio")
RENAME = {"lemon-small-fx": "ml-small-fx"}
SKIP_DIRS = {"examples", "tests", "benches", "target", "research", "design", "tools"}
# Dependencies that are Honeydew's vendored Lemon Studio crates.
OS = {"os-plugkit", "os-ui", "os-theme", "os-hostlink", "os-dsp", "os-engine", "os-model", "os-paths", "os-clap", "os-host"}

def manifest(name, src):
    text = open(os.path.join(src, "Cargo.toml")).read()
    desc = re.search(r'^description = "(.*)"', text, re.M)
    section, deps, build_deps = None, [], []
    for line in text.splitlines():
        m = re.match(r"\[(.+)\]", line)
        if m:
            section = m.group(1)
            continue
        if not line.strip() or line.startswith("#") or "=" not in line:
            continue
        key = line.split("=")[0].strip()
        if section == "dependencies":
            deps.append((key, line))
        elif section == "build-dependencies":
            build_deps.append(line)
    out = []
    has_stub = False
    for key, line in deps:
        k = key.split(".")[0]
        if k == "nih_plug":
            out.append('nih_plug = { path = "../../nih-plug" }')
        elif k in OS:
            feat = ', features = ["plugins"]' if k == "os-engine" else ""
            out.append(f'{k} = {{ path = "../../lemon-studio/{k}"{feat} }}')
        elif k in RENAME:
            out.append(f'{k} = {{ package = "{RENAME[k]}", path = "../{RENAME[k]}" }}')
        elif k.startswith("ml-") or k.startswith("lemon-"):
            out.append(f'{k} = {{ path = "../{k}" }}')
        elif k == "rfd":
            out.append('rfd = { package = "host-rfd", path = "../../lemon-studio/host-rfd" }')
        else:
            out.append(line)
    deps_text = "\n".join(out)
    embed = []
    if any(l.startswith("os-plugkit") for l in out):
        embed.append('"os-plugkit/embed"')
    if any(l.startswith("os-hostlink") for l in out):
        embed.append('"os-hostlink/embed"')
    for k, _ in deps:
        if k in RENAME or k.startswith("ml-") or k.startswith("lemon-"):
            embed.append(f'"{k}/embed"')
    pkg = RENAME.get(name, name)
    libname = pkg.replace("-", "_")
    return f'''[package]
name = "{pkg}"
description = "{desc.group(1) if desc else pkg}"
version = "0.0.1"
edition = "2024"
license = "GPL-3.0-or-later"
publish = false

[lib]
name = "{libname}"
doctest = false

[features]
# Compiled into Honeydew: no exported plugin entry points.
linked = []
# Honeydew (vendored addition): the editor draws into Honeydew's own window (`os-plugkit`'s `embed`).
embed = [{", ".join(sorted(set(embed)))}]

[dependencies]
{deps_text}
{"" if not build_deps else chr(10) + "[build-dependencies]" + chr(10) + chr(10).join(build_deps) + chr(10)}
[lints.rust]
unknown_lints = "allow"
dead_code = "allow"
'''

EMBED_AWK = r'''
held != "" {
    if ($0 ~ /ParentWindowHandle::X11Window/) {
        sub(/target_os = "linux"/, "any(target_os = \"linux\", all(feature = \"embed\", not(target_os = \"macos\")))", held)
        x11++
    }
    print held
    held = ""
}
/^ *#\[cfg\(target_os = "linux"\)\]$/ { held = $0; next }
/^ *#\[cfg\(any\(target_os = "linux", target_os = "macos"\)\)\]$/ { sub(/"macos"\)\)/, "\"macos\", feature = \"embed\"))"); gui++ }
/^use std::time::/ { sub(/^use std::time::/, "use os_plugkit::time::") }
/^ *while let Some\(input\) = self\.win\.next\(\) \{$/ { match($0, /^ */); indent = substr($0, 1, RLENGTH); end = indent "}" }
{ print }
end != "" && $0 == end { print indent "os_plugkit::park!(self);"; end = ""; park++ }
END { if (held != "") print held; exit !(gui && x11 == 1 && park == 1) }
'''

def embed_editor(path):
    r = subprocess.run(["awk", EMBED_AWK, path], capture_output=True, text=True)
    open(path, "w").write(r.stdout)
    if r.returncode:
        print(f"warning: {path} is off the editor template: embed it by hand", file=sys.stderr)

for name in sys.argv[2:]:
    src = os.path.join(fork, "plugins", name)
    pkg = RENAME.get(name, name)
    out = os.path.join(dest, pkg)
    shutil.rmtree(out, ignore_errors=True)
    shutil.copytree(src, out, ignore=lambda d, names: [n for n in names if n in SKIP_DIRS or n == "Cargo.toml" or n == "Cargo.lock"])
    open(os.path.join(out, "Cargo.toml"), "w").write(manifest(name, src))
    for dp, _, files in os.walk(out):
        for f in files:
            p = os.path.join(dp, f)
            if f.endswith(".rs"):
                s = open(p).read()
                s2 = s.replace("lemon_small_fx", "ml_small_fx")
                if s2 != s:
                    open(p, "w").write(s2)
                if f == "editor.rs" and "ParentWindowHandle" in s2:
                    embed_editor(p)
    print("vendored", name, "->", pkg)
