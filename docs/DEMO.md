# GHOSTRACE live demo

This is a real transcript from the reference MacBook Pro (M1, macOS 26.6.2),
recorded on 2026-09-28 with an unsigned debug build, a throwaway GHOSTRACE home,
and a throwaway Git repository. Nothing left the machine.

What it shows:

- `ghostrace run` wraps three commands. Exit codes pass through unchanged (0, 3,
  127); the command that could not start becomes a gap, not a fake success; the
  argument `SECRET_TOKEN=abc123` is never recorded.
- `ghostrace live watch` records the creation, rename, and deletion of files in a
  folder as salted digests. The rename is only *contextual*: macOS does not say
  which old name became which new one, so GHOSTRACE does not claim it.
- `ghostrace live git-snapshot` records repository state without branch, file, or
  remote names. After `git commit --amend`, the earlier commit is no longer in
  the history, so a `history_rewritten` gap is recorded instead of pretending the
  history is continuous.
- A search of the decrypted journal for the secret, the file names, and the demo
  path finds nothing.
- `ghostrace live forget` deletes the key and the home.

The key is kept in the login keychain because the build is not Developer ID
signed; see [Key custody without Developer ID signing](PRIVACY.md#key-custody-without-developer-id-signing).
After rebuilding an unsigned binary, macOS asks once whether the new binary may
use the key.

## Try it

~~~sh
cargo build --release --locked
G=target/release/ghostrace
$G live init
$G live consent-shell   # shows what `run` records; asks once
$G run -- make test
$G live watch ~/Projects/something --seconds 60
$G live git-snapshot ~/Projects/something
$G live timeline
$G live explain <event-id>
$G live forget
~~~

## Transcript

Recorded before `ghostrace run` required `live consent-shell`; the output is
otherwise unchanged.

~~~text
$ ghostrace live init
GHOSTRACE home created at <home>
Key custody: login keychain (explicit opt-in for unsigned builds).
Nothing is recorded until you run `ghostrace run`, `ghostrace live watch`,
or `ghostrace live git-snapshot`.

$ ghostrace run -- ls
(exit 0)

$ ghostrace run -- sh -c 'echo SECRET_TOKEN=abc123 >/dev/null; exit 3'
(exit 3)

$ ghostrace run -- ./does-not-exist
ghostrace: the command did not start (shell_exec_failed); recorded as a gap
(exit 127)

$ ghostrace live git-snapshot
Recorded Git snapshot: clean worktree, local branch, 0 changed path(s).

$ ghostrace live watch . --seconds 9 --yes
GHOSTRACE will watch this folder until you stop it:
  <demo>/project

Recorded: which files change (as salted digests, not names), the kind of change
(created, modified, renamed, deleted), file or folder, and when.
Not recorded: file contents, file names in readable form, which app made the change.
Limits: macOS may coalesce rapid changes; missed history is recorded as a gap.
Watching. Press Ctrl-C to stop.
Stopped after 9 s: 5 change(s) recorded, 0 outside scope, 0 lost.

$ ghostrace live git-snapshot   # after a new commit
Recorded Git snapshot: clean worktree, local branch, 0 changed path(s).
Since the previous snapshot: head_moved.

$ ghostrace live git-snapshot   # after git commit --amend
Recorded Git snapshot: clean worktree, local branch, 0 changed path(s).
Since the previous snapshot: head_moved.
History gap recorded: history_rewritten (earlier history cannot be re-proven).

$ ghostrace live status
key custody:    LoginKeychain
events:         17
gaps:           2
  filesystem   5
  git          4
  lifecycle    2
  shell        6
watched roots:  1
last event:     2026-09-28 18:08:42 UTC

$ ghostrace live timeline
18:08:32  shell      direct    Direct observation: event 29ddea0a-a4c8-4b52-b27f-c0c9fca5a765: shell session started (ghostrace-run, session shell-e8398ad41bc8416287427f74c3319902).
          29ddea0a-a4c8-4b52-b27f-c0c9fca5a765
18:08:32  shell      direct    Direct observation: event 9759a046-aba0-4c93-928e-a422df709e6c: shell session finished (Succeeded, 10 ms, session shell-e8398ad41bc8416287427f74c3319902).
          9759a046-aba0-4c93-928e-a422df709e6c
18:08:32  shell      direct    Direct observation: event 52c2ad82-ad31-4987-b23a-f63050629b6c: shell session started (ghostrace-run, session shell-1e55dc74786844cd898099e1f5878ad5).
          52c2ad82-ad31-4987-b23a-f63050629b6c
18:08:32  shell      direct    Direct observation: event a099eb68-221f-4d15-8580-c4282274eef6: shell session finished (Failed, 11 ms, session shell-1e55dc74786844cd898099e1f5878ad5).
          a099eb68-221f-4d15-8580-c4282274eef6
18:08:32  shell      direct    Direct observation: event e7ae4fbe-09f1-4da8-b3d0-a9cfad3db6ef: shell session started (ghostrace-run, session shell-e448d25444f04ab78b70643cbe85399b).
          e7ae4fbe-09f1-4da8-b3d0-a9cfad3db6ef
18:08:32  shell      unknown   Unknown observation: event 5ad57b2a-ab61-4173-93a3-e956449d7679: shell coverage gap: shell_exec_failed (0 events were not observed).
          5ad57b2a-ab61-4173-93a3-e956449d7679
18:08:32  git        direct    Direct observation: event 92b4c9d2-4671-4415-9ab3-7d4a190b674d: Git snapshot for repository git-2d329052347669fa8d903ff8a429425ca9f6f13e6218a709ef1603970a2f95c0 at 4f7591938c9ea827bbfc6e12440f6bb2c283fa67 (0 changed files; dirty=false).
          92b4c9d2-4671-4415-9ab3-7d4a190b674d
18:08:32  lifecycle  direct    Direct observation: event 453b6249-3368-47e6-bee1-ce93b26f3931: filesystem collector started (ghostrace-watch).
          453b6249-3368-47e6-bee1-ce93b26f3931
18:08:36  filesystem direct    Direct observation: event 08ff9d2b-efd6-4753-ba2d-1a34e53d5052: filesystem created (File, AbsoluteRedacted) in root root-f0a24aa5d6e7.
          08ff9d2b-efd6-4753-ba2d-1a34e53d5052
18:08:37  filesystem direct    Direct observation: event 04f96cca-dcd1-4c1b-ba30-4c1a856aa208: filesystem created (File, AbsoluteRedacted) in root root-f0a24aa5d6e7.
          04f96cca-dcd1-4c1b-ba30-4c1a856aa208
18:08:38  filesystem contextual Contextual observation: event 2b6a3289-3295-4a20-8187-4badf832caf1: filesystem rename event (File) in root root-f0a24aa5d6e7. Old-to-new identity is not established.
          2b6a3289-3295-4a20-8187-4badf832caf1
18:08:38  filesystem contextual Contextual observation: event 7841f07d-87da-47b7-8e7e-be2508a14150: filesystem rename event (File) in root root-f0a24aa5d6e7. Old-to-new identity is not established.
          7841f07d-87da-47b7-8e7e-be2508a14150
18:08:39  filesystem direct    Direct observation: event 44ee3c83-1a34-4a4f-ad56-2830f52dfe52: filesystem deleted (File, AbsoluteRedacted) in root root-f0a24aa5d6e7.
          44ee3c83-1a34-4a4f-ad56-2830f52dfe52
18:08:42  lifecycle  direct    Direct observation: event 9a908446-45b0-4df8-b30b-a05be873eef9: filesystem collector stopped (ghostrace-watch).
          9a908446-45b0-4df8-b30b-a05be873eef9
18:08:42  git        direct    Direct observation: event 26b165aa-6d2a-4473-bce5-663809edc26c: Git snapshot for repository git-2d329052347669fa8d903ff8a429425ca9f6f13e6218a709ef1603970a2f95c0 at a14c9522d66634971f505501d125b432e828fc2a (0 changed files; dirty=false).
          26b165aa-6d2a-4473-bce5-663809edc26c
18:08:42  git        direct    Direct observation: event a715e301-9e49-45ec-a559-9870a7383446: Git snapshot for repository git-2d329052347669fa8d903ff8a429425ca9f6f13e6218a709ef1603970a2f95c0 at 4eff0a641c33fbb596f2041ccc24327e17d2be09 (0 changed files; dirty=false).
          a715e301-9e49-45ec-a559-9870a7383446
18:08:42  git        unknown   Unknown observation: event af369da0-514b-452c-bdb5-cafed7ffdd45: git coverage gap: git_history_rewritten (0 events were not observed).
          af369da0-514b-452c-bdb5-cafed7ffdd45

$ ghostrace live explain <last file event>
{
  "statements": [
    "Direct observation: event 44ee3c83-1a34-4a4f-ad56-2830f52dfe52: filesystem deleted (File, AbsoluteRedacted) in root root-f0a24aa5d6e7."
  ],
  "coverage": {
    "chain_event_count": 1,
    "gap_event_count": 0,
    "warnings": []
  }
}

$ privacy check: grep the decrypted journal for secrets and names
0

$ ghostrace live forget --yes
Deleted the journal key and the GHOSTRACE home.
~~~
