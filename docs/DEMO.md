# GHOSTRACE live demo

`scripts/live-demo.sh` runs a complete walkthrough on your Mac in a temporary
workspace: its own GHOSTRACE home and key, a folder, and a Git repository. It uses
no network and none of your files or Git configuration, and it deletes the key,
home, and workspace when it exits, even after an error.

~~~sh
cargo build --release --locked
scripts/live-demo.sh
~~~

The transcript below is real output from the reference MacBook Pro (M1, macOS
26.6.2), recorded on 2026-09-29 with an unsigned release build. The temporary
directory is shown as `<workspace>`; event IDs and times differ on every run, and
FSEvents may coalesce a short-lived file into one event, so the watch count can
vary.

What it shows:

- `ghostrace run` refuses until you consent, then passes exit codes through (0,
  3, 127); a program that cannot start becomes a gap, not a fake success, and the
  argument `DEMO_SECRET=abc123` is never recorded.
- `ghostrace live watch` records file creation, renames, and deletion as salted
  digests. A rename is *contextual*: macOS does not say which old name became which
  new one, so GHOSTRACE does not claim it.
- `ghostrace live git-snapshot` records repository state without branch, file, or
  remote names. After `git commit --amend` the replaced commit is gone, so a
  `history_rewritten` gap marks the break.
- The timeline and the offline HTML report contain none of the secrets, file
  names, branch name, or workspace path.
- The script ends by listing what was not observed, then deletes the key and home.

The key is kept in the login keychain because the build is not Developer ID
signed; see [Key custody without Developer ID signing](PRIVACY.md#key-custody-without-developer-id-signing).
After rebuilding an unsigned binary, macOS asks once whether the new binary may use
the key, unless you sign builds with `scripts/local-signing.sh`.

## Using it on your own folders

~~~sh
G=target/release/ghostrace
$G live init
$G live consent-shell            # shows what `run` records; asks once
$G run -- make test
$G live watch ~/Projects/something --seconds 60
$G live git-snapshot ~/Projects/something
$G live timeline
$G live explain <event-id>
$G live report --output ~/Desktop/ghostrace-timeline.html
$G live forget
~~~

## Transcript

~~~text
## 1. Create a private home with its key in the login keychain
$ ghostrace live init --home <workspace>/home
GHOSTRACE home created at <workspace>/home
Key custody: login keychain (explicit opt-in for unsigned builds).
Nothing is recorded until you ask: `ghostrace live consent-shell` then
`ghostrace run -- <command>`, `ghostrace live watch <folder>`, or
`ghostrace live git-snapshot`. Never recorded: file contents or readable
file names, command arguments, environment, terminal input or output,
branch or remote names, or which app made a change.

## 2. Nothing runs through GHOSTRACE until you consent
$ ghostrace run --home <workspace>/home -- /usr/bin/true
error: invalid event: `ghostrace run` needs your consent first; run `ghostrace live consent-shell`
$ ghostrace live consent-shell --home <workspace>/home --yes
`ghostrace run -- <command>` will record, for each command you run through it:
- the program's name as a normalized token, and the kind of working folder
(as a salted digest, not a path)
- when it started and finished, how it ended, and its exit code or signal
Never recorded: arguments, environment variables, input or output, or anything
you run without `ghostrace run`. Consent lasts until `ghostrace live revoke-shell`.

Allowed. Revoke with `ghostrace live revoke-shell`.

## 3. Wrapped commands keep their exit codes; arguments are never recorded
$ ghostrace run --home <workspace>/home -- /bin/sh -c exit\ 0 DEMO_SECRET=abc123
$ ghostrace run --home <workspace>/home -- /bin/sh -c exit\ 3
(exit code 3)
$ ghostrace run --home <workspace>/home -- /nonexistent/tool
ghostrace: the command did not start (shell_exec_failed); recorded as a gap
(exit code 127)

## 4. Watch one folder while files are created, renamed, and deleted
Stopped after 5 s: 5 change(s) recorded, 0 outside scope, 0 lost.

## 5. Snapshot a Git repository, commit, then rewrite history
$ ghostrace live git-snapshot --home <workspace>/home <workspace>/repo
Recorded Git snapshot: clean worktree, local branch, 0 changed path(s).
$ ghostrace live git-snapshot --home <workspace>/home <workspace>/repo
Recorded Git snapshot: clean worktree, local branch, 0 changed path(s).
Since the previous snapshot: head_moved.
$ ghostrace live git-snapshot --home <workspace>/home <workspace>/repo
Recorded Git snapshot: clean worktree, local branch, 0 changed path(s).
Since the previous snapshot: head_moved.
History gap recorded: history_rewritten (earlier history cannot be re-proven).

## 6. What the journal holds
$ ghostrace live status --home <workspace>/home
key custody:    LoginKeychain
events:         17
gaps:           2
  filesystem   5
  git          4
  lifecycle    2
  shell        6
watched roots:  1
run consent:    granted
last event:     2026-09-29 01:14:45 UTC
$ ghostrace live timeline --home <workspace>/home --limit 100
01:14:38  shell      direct    Direct observation: event 29682c18-8636-4ce4-bae9-78fd7bc88f21: shell session started (ghostrace-run, session shell-af845e1b527c41048fe4ce39139b0cdd).
          29682c18-8636-4ce4-bae9-78fd7bc88f21
01:14:38  shell      direct    Direct observation: event fa03c8e1-755d-46b2-9245-00de30eb975d: shell session finished (Succeeded, 8 ms, session shell-af845e1b527c41048fe4ce39139b0cdd).
          fa03c8e1-755d-46b2-9245-00de30eb975d
01:14:38  shell      direct    Direct observation: event abaefd47-8087-4d0c-a728-849b66597fc8: shell session started (ghostrace-run, session shell-5afe06beaa344e4f89d4feb0f6d93618).
          abaefd47-8087-4d0c-a728-849b66597fc8
01:14:38  shell      direct    Direct observation: event 9b1aac73-53c2-4679-b9ea-15c1b99d5c69: shell session finished (Failed, 8 ms, session shell-5afe06beaa344e4f89d4feb0f6d93618).
          9b1aac73-53c2-4679-b9ea-15c1b99d5c69
01:14:38  shell      direct    Direct observation: event 3c9f0acb-82ab-436a-ba08-55479a6c9139: shell session started (ghostrace-run, session shell-e380bc58bad54d818d360d2a2a43547e).
          3c9f0acb-82ab-436a-ba08-55479a6c9139
01:14:38  shell      unknown   Unknown observation: event 4df66306-e46f-4c5a-93a7-c96398547165: shell coverage gap: shell_exec_failed (0 events were not observed).
          4df66306-e46f-4c5a-93a7-c96398547165
01:14:38  lifecycle  direct    Direct observation: event 7396f0b0-a79d-4661-85e4-85c6ce24d57d: filesystem collector started (ghostrace-watch).
          7396f0b0-a79d-4661-85e4-85c6ce24d57d
01:14:39  filesystem direct    Direct observation: event b1f2d694-1e02-4cd9-813a-f78caa9b833c: filesystem created (File, AbsoluteRedacted) in root root-ee52419b2569.
          b1f2d694-1e02-4cd9-813a-f78caa9b833c
01:14:39  filesystem direct    Direct observation: event 17409b2b-6ee7-40b9-92df-2ed30fb82422: filesystem created (File, AbsoluteRedacted) in root root-ee52419b2569.
          17409b2b-6ee7-40b9-92df-2ed30fb82422
01:14:40  filesystem contextual Contextual observation: event ef976e99-a204-4e55-94bf-4a1b61a14e5f: filesystem rename event (File) in root root-ee52419b2569. Old-to-new identity is not established.
          ef976e99-a204-4e55-94bf-4a1b61a14e5f
01:14:40  filesystem contextual Contextual observation: event df160cfb-5978-4de5-a580-52848ac97b00: filesystem rename event (File) in root root-ee52419b2569. Old-to-new identity is not established.
          df160cfb-5978-4de5-a580-52848ac97b00
01:14:40  filesystem direct    Direct observation: event a92add1a-3055-4de8-b96c-524f95a1e49c: filesystem deleted (File, AbsoluteRedacted) in root root-ee52419b2569.
          a92add1a-3055-4de8-b96c-524f95a1e49c
01:14:43  lifecycle  direct    Direct observation: event ea4939a5-5883-4067-8a71-b5bf8424115d: filesystem collector stopped (ghostrace-watch).
          ea4939a5-5883-4067-8a71-b5bf8424115d
01:14:44  git        direct    Direct observation: event 29e3bece-c143-44db-a1a6-cbc8a8fc5a69: Git snapshot for repository git-d47ae9ef2cb8a93b2a3e228d073c55f238c2631cdc9eec044ad3a735ae788ed6 at bfba8c8db7779a25b21610fbc885f05bde757a2d (0 changed files; dirty=false).
          29e3bece-c143-44db-a1a6-cbc8a8fc5a69
01:14:44  git        direct    Direct observation: event 7a5e9641-803d-4355-82a7-b47b69c1696e: Git snapshot for repository git-d47ae9ef2cb8a93b2a3e228d073c55f238c2631cdc9eec044ad3a735ae788ed6 at 9a93b28e34194a31009c13971cf3570b564acf67 (0 changed files; dirty=false).
          7a5e9641-803d-4355-82a7-b47b69c1696e
01:14:45  git        direct    Direct observation: event eb8a52d7-bf78-4fe0-9c59-5b0824b5c490: Git snapshot for repository git-d47ae9ef2cb8a93b2a3e228d073c55f238c2631cdc9eec044ad3a735ae788ed6 at 6fd31520c358a6fbf1fbb5a0666db25514e32c73 (0 changed files; dirty=false).
          eb8a52d7-bf78-4fe0-9c59-5b0824b5c490
01:14:45  git        unknown   Unknown observation: event 2f763532-3205-4b8d-95ce-0a2da9ae3baa: git coverage gap: git_history_rewritten (0 events were not observed).
          2f763532-3205-4b8d-95ce-0a2da9ae3baa

## 7. An offline HTML report
$ ghostrace live report --home <workspace>/home --yes --output <workspace>/timeline.html
This file is an unencrypted copy of part of your GHOSTRACE journal. Anyone who can open it can read it, and deleting journal records does not delete this copy.
Wrote <workspace>/timeline.html (17 event(s)).

## 8. Nothing sensitive was recorded
DEMO_SECRET    0 match(es)
abc123         0 match(es)
secret-plan    0 match(es)
final-plan     0 match(es)
notes.md       0 match(es)
demo-branch    0 match(es)
demo-file      0 match(es)
<workspace>    0 match(es)

## 9. What GHOSTRACE did not observe
- Anything run without `ghostrace run`, and the arguments, environment, and output
  of what was run through it.
- Which old file name became which new one: macOS reports a rename as two events,
  so the pairing stays contextual.
- File contents and readable file names: only salted digests are kept.
- Which program changed a file: FSEvents does not say.
- Changes macOS coalesced: a file created and deleted within one batch can appear
  as a single event, as notes.md may here.
- The commit that `--amend` replaced: it is gone from history, so a
  history_rewritten gap marks the break instead of a guess.
- Anything outside the one watched folder, or while nothing was running.

## 10. Forget: delete the key and the home
$ ghostrace live forget --home <workspace>/home --yes
Deleted the journal key and the GHOSTRACE home.
~~~
