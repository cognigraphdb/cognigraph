# Decision: the CLI never trusts a success status, and export never replaces a file

Status: accepted and implemented 2026-11-11 for [CG-114](../issues/CG-114.md).

## Context

The CLI's HTTP wrapper turned a 2xx response whose body could not be read or
parsed into the JSON string `"<non-JSON response>"` and reported success.
`cognigraph export --out FILE` then wrote that string over `FILE` with the
default file mode, printed "snapshot written" and exited 0. A connection that
dropped half-way, the client timeout during the body read, or a proxy or the
console fallback answering 200 with HTML each replaced a good backup with
garbage. The snapshot holds password hashes and every document.

## Decision

1. **A success status is not a success.** The body of every response is read
   in full and parsed. An unreadable or non-JSON body on a 2xx response is an
   error for every command, with exit code 1. An empty body is `null`.
2. **A snapshot must look like one.** `export` requires a `collections`
   object before it writes or prints anything.
3. **Private and durable.** `export --out` uses the writer that custody plans
   and `references repair` already use: a staging file created with mode
   0600, synced, then published with a no-replace rename, and the parent
   directory synced.
4. **No overwrite, and no flag for it.** If the destination exists the
   command fails and the file is untouched. A backup tool that can replace
   the previous backup needs every failure mode above to be right forever; a
   new name per backup needs none of them.
5. **Rejected: an `--overwrite` flag with an atomic replace.** It is safe
   when the new snapshot is good, and the checks in points 1 and 2 make that
   likely, but it puts the only copy at stake for the sake of a shorter
   command line.

## Consequences

- Breaking: a script that exports to a fixed path fails on the second run.
  Use a dated name, as the Helm backup job does.
- Error messages for failed requests show the start of the body instead of a
  placeholder.
- The client timeout that makes a truncated body likely is a separate ticket,
  [CG-147](../issues/CG-147.md).
