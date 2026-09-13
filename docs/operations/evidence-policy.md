# Public records and private captures

Keep executable contracts, architecture, decisions, issues, dated research
conclusions, methods and generic operating guides with the public code. Keep
small synthetic or explicitly attributed fixtures needed by ordinary tests.
The [evidence catalog](../evidence/README.md) supplies path, size and SHA-256
references for private historical captures.

## Capture and publication

Write raw provider exchanges, labelled evaluation corpora, browser captures and
real deployment inventories in a new dated package in the private
`cognigraphdb/cognigraph-evidence` repository, normally the optional sibling
`../evidence/`. Use an explicit output path. The public checkout and CI must work
without that repository. Local scratch output may use ignored `data/` directories
or an external temporary directory; being ignored does not make a file an archive.

Keep credentials and full database exports out of both repositories. Review
screenshots as well as text before publishing a summary. Use example origins,
synthetic actor identities and public source revisions in public reports; actual
hostnames, account/service/volume identifiers and operator email addresses belong
in the private operating record.

Preserve sealed original bytes, licences and source attribution. Publish amended
reading copies separately with original provenance and a clear statement of what
changed. Keep negative results and qualification limitations visible. A public
reader cannot reproduce a private benchmark without authorized artifact access;
do not describe a hash as independent validation or signed attestation.

Research example output under `data/dailymed/` is local scratch. Prepare required
inputs using the documented research workflow or provide explicitly authorized
private inputs. Do not silently skip missing test fixtures, rewrite frozen results,
run model comparisons, or execute a holdout as part of routine code verification.

## Enforcement

`python3 scripts/check-public-distribution.py` checks the current publication
candidate for prohibited capture paths, deployment hostnames, credential-shaped
content, oversized artifacts, allowed Semantic Neurons test inputs and catalog
structure. It also verifies public descriptor hashes against originals when run
with `--private-root ../evidence`. That optional check is not required by CI.

The checker is part of the shared CI and pre-push workflow. It is a targeted
distribution guard, not a comprehensive secret scanner or a guarantee that
unstructured prose and images contain no private information. Review newly added
materials. Update policy exceptions only for a documented public verification need.

## Historical publication

Removing a current file does not remove its Git history, forks, caches or prior
downloads. Any history operation needs separate owner authorization, preserved
originals, an exact ref/path manifest and drift checks. Follow the generic
[push safeguards](push.md#owner-authorized-data-removal); keep incident-specific
inventories and remediation records in the private evidence repository.

Published image digests and source labels remain truthful historical identities.
Do not overwrite immutable tags or claim that a rewritten source revision was
the source of an already published image.
