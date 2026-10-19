# Cutting a release

Four commands, and one of them is the one that is easy to forget.

```sh
# 1. The version, in both crates that carry one into a build.
sed -i '' 's/^version = "0.1.14"/version = "0.1.15"/' \
  crates/sail-app/Cargo.toml crates/sail-net/Cargo.toml
cargo build --release -p sail-app        # so Cargo.lock moves with them

# 2. Commit, tag, push.
git commit -am "v0.1.15"
git tag v0.1.15
git push origin main --tags
```

The tag starts `.github/workflows/release.yml`, which builds a universal macOS bundle, a
Windows executable and `sail-server` for both, and publishes them. **An hour from cold,
and about twenty-five minutes when the runner cache is still warm from a recent
release** — nearly all of the cold hour is the Windows job compiling Bevy. Watch it with
`gh run list` or `gh run view --log`.

## The part that is easy to get wrong

**The release notes come from the workflow file in the tagged commit.** Editing
`release.yml` after tagging does nothing for that release — the text was already read.
If the notes need fixing on a release that is already building or already out:

```sh
gh release edit v0.1.15 --notes-file notes.md
```

This happened for real: the macOS instructions told people to right-click and Open, which
Apple removed in macOS 15, and every release carrying that text had to be edited by hand
afterwards.

## Before the tag

```sh
cargo test --release          # 248 of them, about twenty seconds
cargo clippy --release --all-targets    # has to be silent
cargo fmt --all
```

Nothing in CI runs these — the only workflow is the release one. Until that changes, the
tests passing is a thing a person did, not a thing the repository knows.

### `--release` used to be load-bearing, and is not any more

Six tests in `settings.rs` **failed in a debug build and passed in a release one**:
`cargo test -p sail-app` panicked in all six with

```
Dropped TexturesDelta with 1 unapplied deltas.
```

which is a debug assertion inside `epaint`, fired by the egui harness those tests drive
and not by anything in this repository. In release the assertion is compiled out, which
is why the rule above was `--release` rather than a fix.

The fix was one line: the harness takes the frame's texture uploads and drops them on
purpose, which is what an application with a renderer would do with them. The suite is
clean in both profiles now, and a debug run of `cargo test` is a thing you can believe
again — which matters, because "those six always fail" is exactly how a seventh failure
goes unnoticed.

Written down because two people spent an evening reporting those four to each other and
to the user as a standing failure that "also fails on main" — which was true, and which
meant everybody stopped looking. It is a property of the profile, not of the code. If you
are about to fix them, measure in release first and find out whether there is anything
there to fix.

## Getting at a build before the release is published

The two jobs upload their artefacts as they finish, so the Mac build is available half an
hour before the release page exists:

```sh
gh run download <run-id> -n macos -D /tmp/sail
```

## What a version number means here

Nothing yet. It counts releases so that "the one with the bell in it" has a name, and so
the installer can tell whether it already has the newest. There is no stability promise
and no changelog file: the commit messages are the changelog, and they are written to be
read.

## When somebody says `irm is not recognized`

They pasted it into the Command Prompt. `irm` is `Invoke-RestMethod` and it is a
PowerShell word; `cmd.exe` has never heard of it and never will. The release notes now
say "press Start, type powershell" in as many words, and carry a second line that works
from either shell and from the Run box:

```
powershell -NoProfile -ExecutionPolicy Bypass -Command "[Net.ServicePointManager]::SecurityProtocol='Tls12'; irm https://raw.githubusercontent.com/kapoost/sail/main/dist/install.ps1 | iex"
```

The two extra pieces are for older machines and neither is superstition: `-ExecutionPolicy
Bypass` for one that refuses to run scripts, and the TLS line for a Windows 10 whose
PowerShell 5.1 still opens connections with a protocol GitHub stopped accepting in 2018.

Reported from a friend's machine the first time anybody who had not seen the instruction
written down tried to follow it, which is the only test a set of instructions has.
