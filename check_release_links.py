#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
# Copyright (c) 2026 Janos Sonntag
"""
check_release_links.py — the site's Downloads menu points at the newest release.

The landing page's nav carries a Downloads menu whose six links go straight to
the GitHub release assets. Those assets cannot be named generically: CI packs
them as `bovnar-<version>-<count>-…`, where <count> is the tagged commit's total
commit count, so there is no name-agnostic URL GitHub would redirect. The menu
is therefore pinned to a tag by hand — and a hand-written URL set that has to be
retyped at every release is precisely the thing that gets forgotten. Forgotten
here it is silent: the links keep resolving, keep downloading, and keep handing
out the PREVIOUS version to everyone who takes the site at its word.

So this recomputes what the release job would have produced and compares:

  * the newest v* tag is the one the menu links to;
  * every asset filename matches what pack_artifacts.cmake names for that tag's
    version and commit count -- derived the way the packer derives them, from
    project() in CMakeLists.txt AT THE TAG and `git rev-list --count <tag>`,
    not from the tag string, so a tag cut without bumping project() is caught
    too (that mismatch would name every archive after the old version);
  * the version the panel prints in its heading is that same tag.

What it deliberately does not check: the human-readable sizes beside each link.
They are not derivable without asking github.com, and a test here must not
depend on the network (the same rule check_web_links.py works under). A stale
size mislabels a download; a stale URL ships the wrong software.

Note the ordering this implies when cutting a release: tag first, let CI build
and attach the archives, then update the menu. Updating the page first fails
here, and rightly -- until the tag exists those files do not.

Usage:
    python3 check_release_links.py            # exit non-zero on drift
    python3 check_release_links.py --verbose  # also print what matched
"""
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
PAGE = os.path.join(ROOT, "web", "index.html")

REPO = "sothis/bovnar"
DOWNLOAD_PREFIX = f"https://github.com/{REPO}/releases/download/"
RELEASES_URL = f"https://github.com/{REPO}/releases"

# The nav's downloads menu, from <li class="nav-dl"> to its closing </li>. Only
# this block is examined: the page links to github.com in a dozen other places
# and none of them is a release asset.
BLOCK_RE = re.compile(r'<li class="nav-dl">(.*?)\n\s*</li>\s*\n\s*<li>', re.S)
HREF_RE = re.compile(r'href="([^"]+)"')
# "Latest release <b>v1.1.0</b>" — the version the panel shows the reader.
HEAD_RE = re.compile(r'<div class="nav-dl-head">[^<]*<b>([^<]+)</b>')
# The schema.org SoftwareApplication entry's own statement of the version.
SOFTWARE_VERSION_RE = re.compile(r'"softwareVersion":\s*"([^"]+)"')


def git(*args):
    """Run git in the repo; return stdout, or None if git or the command fails."""
    try:
        out = subprocess.run(("git", "-C", ROOT) + args, capture_output=True,
                             text=True, check=True)
    except (OSError, subprocess.CalledProcessError):
        return None
    return out.stdout.strip()


def newest_tag():
    """The highest v<major>.<minor>.<patch> tag, or None if there are none.

    --sort=-v:refname orders by version number rather than lexically, so v1.10.0
    sorts above v1.9.0 the way a release does and not the way a string does.
    """
    out = git("tag", "--list", "v[0-9]*", "--sort=-v:refname")
    if not out:
        return None
    return out.splitlines()[0].strip()


def version_at(tag):
    """The project() version recorded in CMakeLists.txt at `tag`.

    This is the single source of truth pack_artifacts.cmake and the source-tarball
    job both read; taking it from the tag NAME instead would agree with them right
    up until the release where it does not.
    """
    src = git("show", f"{tag}:CMakeLists.txt")
    if src is None:
        return None
    m = re.search(r"project\(bovnar VERSION (\d+\.\d+\.\d+)", src)
    return m.group(1) if m else None


def expected_assets(version, count):
    """Filenames the release job attaches, in the order the menu lists them.

    Mirrors cmake/pack_artifacts.cmake (the amalgamation and one archive per
    platform and toolchain), the source-tarball job, and the SHA256SUMS the
    release job writes beside them.
    """
    tag = f"{version}-{count}"
    return [
        f"bovnar-linux-{tag}.tar.xz",
        f"bovnar-windows-msvc-{tag}.zip",
        f"bovnar-windows-mingw-{tag}.zip",
        f"bovnar-{tag}-amalgamate.tar.xz",
        f"bovnar-{tag}-source.tar.xz",
        "SHA256SUMS",
    ]


def fail(*lines):
    for line in lines:
        print(line, file=sys.stderr)
    return 1


def main():
    verbose = "--verbose" in sys.argv[1:]

    with open(PAGE, encoding="utf-8") as fh:
        html = fh.read()

    block = BLOCK_RE.search(html)
    if not block:
        return fail(
            "web/index.html: no <li class=\"nav-dl\"> downloads menu found.",
            "If the menu was removed on purpose, remove this check with it;",
            "otherwise its markup has changed shape and this gate is now blind.")
    block = block.group(1)

    hrefs = HREF_RE.findall(block)
    assets = [h[len(DOWNLOAD_PREFIX):] for h in hrefs
              if h.startswith(DOWNLOAD_PREFIX)]
    others = [h for h in hrefs if not h.startswith(DOWNLOAD_PREFIX)]

    if not assets:
        return fail("web/index.html: the downloads menu links to no release "
                    "asset at all.")
    if RELEASES_URL not in others:
        return fail(f"web/index.html: the downloads menu has no {RELEASES_URL} "
                    "link. That one is the only entry that survives a release "
                    "untouched; keep it.")

    # --- the page agrees with itself ------------------------------------------
    # The JSON-LD block further down states a softwareVersion of its own. It is
    # outside the menu and nothing else reads it, so the menu could move to a
    # new release and leave the structured data -- what a search engine quotes
    # -- on the old one. This needs no git, so it holds in a tarball too.
    panel = HEAD_RE.search(block)
    declared = SOFTWARE_VERSION_RE.findall(html)
    if panel and declared != [panel.group(1).lstrip("v")]:
        return fail(
            f"web/index.html: the downloads menu is headed {panel.group(1)} "
            f"but the JSON-LD \"softwareVersion\" says "
            f"{', '.join(declared) if declared else 'nothing'}.",
            "They name the same release; move them together.")

    # --- what the tree says the newest release is ---------------------------
    # Ask git rather than looking for a .git DIRECTORY: in a `git worktree` it is
    # a file, and the isdir() spelling would have skipped this check entirely
    # from any worktree -- exactly the silent self-disabling it exists to prevent.
    # And ask for the toplevel, not merely "am I inside a work tree": a source
    # tarball unpacked somewhere under an UNRELATED repository is inside that
    # repository's work tree, and was then judged against its tags.
    top = git("rev-parse", "--show-toplevel")
    if top is None or os.path.realpath(top) != os.path.realpath(ROOT):
        # The source tarball is `git archive` output: tracked files, no history.
        # Nothing to compare against, and nothing wrong either.
        print("check_release_links.py: not a git checkout (source tarball?) -- "
              "the menu's release tag cannot be verified here")
        return 0
    if git("rev-parse", "--is-shallow-repository") == "true":
        return fail(
            "this is a shallow clone: `git rev-list --count` would report a "
            "commit count the release job never used, so the asset names cannot "
            "be checked.",
            "Run:  git fetch --unshallow --tags")

    tag = newest_tag()
    if not tag:
        return fail(
            "no v* tag in this clone, so there is no release to check the "
            "downloads menu against.",
            "Run:  git fetch --tags")

    version = version_at(tag)
    if not version:
        return fail(f"{tag}: could not read project(bovnar VERSION ...) from "
                    "CMakeLists.txt at that tag.")

    # THE TAGGED COMMIT ITSELF IS EXEMPT. The order this gate imposes is "tag
    # first, let CI build and attach the archives, then update the menu" -- so
    # on the commit the tag points at, the menu still names the previous
    # release, and it has to: the files it would link to are what this very CI
    # run is about to produce. Failing here made the tag's own test job red,
    # which withheld the archives from the release it was guarding. The check
    # is back in force on the first commit after the tag.
    head = git("rev-parse", "HEAD")
    if head and head == git("rev-parse", f"{tag}^{{commit}}"):
        if tag != f"v{version}":
            return fail(
                f"{tag} records project(bovnar VERSION {version}) in "
                "CMakeLists.txt; the tag and the project version disagree.")
        print(f"check_release_links.py: HEAD is {tag} itself -- its archives "
              "do not exist yet, so the menu is not checked on this commit. "
              "Update web/index.html once the release job has attached them.")
        return 0
    if tag != f"v{version}":
        return fail(
            f"{tag} records project(bovnar VERSION {version}) in CMakeLists.txt, "
            f"so the release job named its archives bovnar-{version}-… while the "
            f"tag itself says {tag}.",
            "One of the two was not bumped; fix that before trusting either.")

    count = git("rev-list", "--count", tag)
    if not count or not count.isdigit():
        return fail(f"{tag}: could not count commits (git rev-list --count).")

    # --- compare -------------------------------------------------------------
    want = expected_assets(version, count)
    want_prefixed = [f"{tag}/{a}" for a in want]

    if assets != want_prefixed:
        wrong_tag = sorted({a.split("/", 1)[0] for a in assets} - {tag})
        lines = [f"web/index.html: the downloads menu does not match {tag} "
                 f"(version {version}, build count {count})."]
        if wrong_tag:
            lines.append(f"  it links to tag(s): {', '.join(wrong_tag)}")
        lines.append("  expected, in order:")
        lines += [f"    {DOWNLOAD_PREFIX}{a}" for a in want_prefixed]
        lines.append("  found:")
        lines += [f"    {DOWNLOAD_PREFIX}{a}" for a in assets]
        lines.append("Retag the hrefs (and the sizes beside them, which this "
                     "check cannot see), then re-run ./gen_i18n.py --extract de "
                     "and translate the new keys.")
        return fail(*lines)

    head = HEAD_RE.search(block)
    if not head:
        return fail('web/index.html: the downloads panel has no '
                    '<div class="nav-dl-head">…<b>version</b> heading.')
    if head.group(1) != tag:
        return fail(f"web/index.html: the downloads panel is headed "
                    f"\"{head.group(1)}\" but links to {tag}.")

    print(f"check_release_links.py: the downloads menu matches {tag} "
          f"({len(want)} assets, build count {count})")
    if verbose:
        for a in want_prefixed:
            print(f"  {DOWNLOAD_PREFIX}{a}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
