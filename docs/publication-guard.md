# Publication guard

This repository is developed on a private remote and published to a public
one. `.githooks/pre-push` refuses to push private material to a public remote.

## What it checks

For a remote whose URL matches `publication.publicRemotes` (default
`github\.com`), every commit about to be pushed is scanned: added lines and
commit messages are matched against a denylist of extended regular
expressions. A branch whose name ends in `-private` is refused outright.
Pushes to any other remote are not checked.

The denylist is deliberately **not** in this repository, because its contents
(host names, user paths, project names) are themselves private.

## Setup, once per clone

    git config core.hooksPath .githooks
    git config publication.denylist /path/to/denylist.txt

The guard fails closed: with no readable denylist, a push to a public remote
is refused.

## When it refuses

It prints the offending lines. Fix them by rewriting the commits that
introduced them — a later commit that deletes the string does not remove it
from the history being published.
