# Docker Hub account and repository setup

- Date: 2026-09-13
- Status: v2.7.11
- Kind: Operations

## Changes

Create the public Community and Enterprise image repositories under `cognigraph`,
add descriptions, database categories and edition-specific overviews, and enable
immutable tags in both repositories. Enable the included Docker Scout analysis
slot for Community. Retain the existing active Read & Write publishing PAT and
GitHub Actions secret; no credential or paid subscription changes are needed.

The [publishing guide](../operations/docker-publishing.md) records current setup,
the token expiry and the remaining first-publication boundary. Maintain the
repository overviews with the code and retain an anonymous API setup receipt.
Remove obsolete Arango backend entries from the plain-language licensing map;
the operative license texts and edition allocation are unchanged.

## Verification

Docker Hub settings and PAT metadata were read back through the authenticated
account UI. Anonymous repository/tag APIs confirm both public identities,
immutable tags, database categories, saved overviews and zero image tags. Scout
selection persisted after reload for Community; Enterprise remains outside the
Personal plan allowance. The Actions secret exists, but its value was not read
or changed. Publisher login and image upload remain unexecuted.

Both public repository pages resolve and report an empty repository. Docker Hub
will display the saved detailed overviews after the first image push.

No source build, image publication, main promotion or deployment is part of
this account-setup record. Documentation checks qualify the local guide updates.
