#!/usr/bin/env python3
"""Verify the pinned CUAD archive and reproduce prepared inputs in a temporary directory."""

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

ROOT = Path(__file__).resolve().parent


def verify(archive):
    raise ValueError("This evidence package was partially withdrawn after data removal; original reproduction is retired.")
    manifest = json.loads((ROOT / "provenance.json").read_text())
    data = manifest["dataset"]
    if archive.stat().st_size != data["archive_bytes"]:
        raise ValueError("CUAD archive size differs from the pinned source")
    with archive.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if digest != data["archive_sha256"]:
        raise ValueError("CUAD archive SHA-256 differs from the pinned source")
    with tempfile.TemporaryDirectory(prefix="cognigraph-cuad-prep-") as tmp:
        stage = Path(tmp)
        count = 0
        with zipfile.ZipFile(archive) as bundle:
            for member in bundle.infolist():
                parts = PurePosixPath(member.filename).parts
                if (len(parts) == 2 and parts == ("CUAD_v1", "master_clauses.csv")) or (
                    len(parts) == 3 and parts[:2] == ("CUAD_v1", "full_contract_txt")
                    and parts[2].endswith(".txt") and parts[2] not in {".", ".."}
                ):
                    destination = stage.joinpath(*parts)
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with bundle.open(member) as source, destination.open("xb") as output:
                        shutil.copyfileobj(source, output)
                    count += 1
        if count != 511:
            raise ValueError(f"expected CSV and 510 contract texts, got {count}")
        prep = ROOT / "historical/prep.py"
        if hashlib.sha256(prep.read_bytes()).hexdigest() != manifest["historical_files"]["prep.py"]["sha256"]:
            raise ValueError("historical preparation script changed")
        shutil.copyfile(prep, stage / "prep.py")
        run = subprocess.run([sys.executable, "prep.py"], cwd=stage, capture_output=True,
                             text=True, check=True, timeout=60)
        reproduced = {}
        for name in ("sample.json", "gold.json", "design_chunks.jsonl", "holdout_chunks.jsonl"):
            result = (stage / "work" / name).read_bytes()
            expected = (ROOT / "historical/work" / name).read_bytes()
            if result != expected:
                raise ValueError(f"prepared artifact differs: {name}")
            reproduced[name] = hashlib.sha256(result).hexdigest()
        return {"archive_sha256": digest, "source_members": count,
                "byte_identical_prepared_files": reproduced, "preparation_stdout": run.stdout,
                "python_version": sys.version.split()[0], "model_calls": 0}


if __name__ == "__main__":
    raise SystemExit("Source preparation retired after repository data removal; no download was performed.")
    parser = argparse.ArgumentParser(description=__doc__)
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--archive", type=Path, help="verify an already downloaded CUAD_v1.zip")
    source.add_argument("--download-to", type=Path, help="download the pinned archive to a new path, then verify")
    args = parser.parse_args()
    if args.download_to:
        dataset = json.loads((ROOT / "provenance.json").read_text())["dataset"]
        # Download privately first; publish only a verified complete archive.
        with tempfile.TemporaryDirectory(prefix="cognigraph-cuad-download-") as tmp:
            archive = Path(tmp) / "CUAD_v1.zip"
            with urllib.request.urlopen(dataset["download_url"], timeout=60) as response, archive.open("wb") as out:
                while block := response.read(1024 * 1024):
                    out.write(block)
                    if out.tell() > dataset["archive_bytes"]:
                        raise ValueError("download exceeds pinned archive size")
            result = verify(archive)
            with archive.open("rb") as src, args.download_to.open("xb") as out:
                shutil.copyfileobj(src, out)
    else:
        result = verify(args.archive)
    print(json.dumps(result, indent=2))
