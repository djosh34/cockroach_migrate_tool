#!/usr/bin/env python3

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path


INTERNAL_JSON_PREFIX = "@nix "
STORE_PATH_PREFIX = "/nix/store/"
DRV_PATH_RE = re.compile(r"^/nix/store/[0-9a-z]{32}-[^ ]+\.drv$")


def fail(message: str) -> None:
    print(message, file=sys.stderr)
    raise SystemExit(1)


def run_command(argv: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(argv, text=True, capture_output=True)
    if check and result.returncode != 0:
        stderr = result.stderr.strip()
        stdout = result.stdout.strip()
        if stderr:
            message = stderr
        elif stdout:
            message = stdout
        else:
            message = f"command failed with exit code {result.returncode}: {' '.join(argv)}"
        fail(message)
    return result


def parse_bundle_json(bundle_json: str) -> dict:
    try:
        bundle = json.loads(bundle_json)
    except json.JSONDecodeError as exc:
        fail(f"invalid bundle json: {exc}")

    if not isinstance(bundle, dict):
        fail("bundle json must decode to an object")

    installables = bundle.get("installables")
    if not isinstance(installables, list) or not installables or not all(isinstance(item, str) and item for item in installables):
        fail("bundle json must contain a non-empty installables array of strings")

    for required_key in ("bundle_id", "artifact_name"):
        if not isinstance(bundle.get(required_key), str) or not bundle[required_key]:
            fail(f"bundle json must contain a non-empty {required_key} string")

    metadata = bundle.get("metadata", {})
    if not isinstance(metadata, dict):
        fail("bundle metadata must be a JSON object when provided")

    return bundle


def load_json_file(path: Path) -> dict:
    try:
        return json.loads(path.read_text())
    except FileNotFoundError:
        fail(f"missing json file: {path}")
    except json.JSONDecodeError as exc:
        fail(f"invalid json in {path}: {exc}")


def ensure_directory(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)


def split_non_empty_lines(raw: str) -> list[str]:
    return [line.strip() for line in raw.splitlines() if line.strip()]


def nix_path_info(paths: list[str]) -> dict[str, dict]:
    result = run_command(["nix", "path-info", "--json", *paths])
    try:
        data = json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        fail(f"invalid nix path-info json: {exc}")

    if not isinstance(data, dict):
        fail("nix path-info json must decode to an object")

    return data


def nix_drv_paths(paths: list[str]) -> dict[str, str]:
    drv_paths: dict[str, str] = {}
    for path in paths:
        result = run_command(["nix", "path-info", "--derivation", path])
        path_drv_paths = split_non_empty_lines(result.stdout)
        if len(path_drv_paths) != 1:
            fail(f"nix path-info --derivation did not return exactly one drv path for {path}")
        drv_paths[path] = path_drv_paths[0]
    return drv_paths


def describe_store_paths(store_paths: list[str], *, logical_ids: dict[str, str] | None = None, installables: dict[str, str] | None = None) -> list[dict]:
    path_info = nix_path_info(store_paths)
    drv_paths = nix_drv_paths(store_paths)
    records: list[dict] = []

    for store_path in store_paths:
        metadata = path_info.get(store_path)
        if not isinstance(metadata, dict):
            fail(f"missing path-info metadata for {store_path}")

        drv_path = drv_paths.get(store_path)
        if drv_path is None:
            fail(f"missing drv path for {store_path}")

        record = {
            "drv_path": drv_path,
            "nar_hash": metadata.get("narHash"),
            "nar_size": metadata.get("narSize"),
            "out_path": store_path,
        }
        if logical_ids and store_path in logical_ids:
            record["logical_id"] = logical_ids[store_path]
        if installables and store_path in installables:
            record["installable"] = installables[store_path]

        if not isinstance(record["nar_hash"], str) or not record["nar_hash"]:
            fail(f"missing narHash for {store_path}")
        if not isinstance(record["drv_path"], str) or not record["drv_path"]:
            fail(f"missing drv path for {store_path}")

        records.append(record)

    return records


def discover_link_farm_members(root_paths: list[str]) -> list[dict[str, str]]:
    members: list[dict[str, str]] = []

    for root_path in root_paths:
        root = Path(root_path)
        if not root.is_dir():
            continue

        for entry in sorted(root.iterdir()):
            if not entry.is_symlink():
                continue

            target = entry.resolve(strict=True)
            target_text = str(target)
            if not target_text.startswith(STORE_PATH_PREFIX):
                continue

            members.append(
                {
                    "logical_id": entry.name,
                    "out_path": target_text,
                    "source_root_path": root_path,
                }
            )

    return members


def write_json(path: Path, payload: dict) -> None:
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def build_bundle(args: argparse.Namespace) -> None:
    bundle = parse_bundle_json(args.bundle_json)
    output_dir = Path(args.output_dir).resolve()
    ensure_directory(output_dir)
    cache_dir = output_dir / "binary-cache"
    ensure_directory(cache_dir)

    started_at = time.time()
    build_result = run_command(["nix", "build", "--no-link", "--print-out-paths", *bundle["installables"]])
    completed_at = time.time()
    root_paths = split_non_empty_lines(build_result.stdout)

    if len(root_paths) != len(bundle["installables"]):
        fail("bundle build must produce exactly one output path per installable")

    root_installables = dict(zip(root_paths, bundle["installables"]))
    root_records = describe_store_paths(root_paths, installables=root_installables)

    member_refs = discover_link_farm_members(root_paths)
    bundle_members: list[dict] = []
    if member_refs:
        member_paths = [member_ref["out_path"] for member_ref in member_refs]
        logical_ids = {member_ref["out_path"]: member_ref["logical_id"] for member_ref in member_refs}
        described_members = describe_store_paths(member_paths, logical_ids=logical_ids)
        members_by_path = {member["out_path"]: member for member in described_members}
        for member_ref in member_refs:
            member = members_by_path.get(member_ref["out_path"])
            if member is None:
                fail(f"missing member metadata for {member_ref['out_path']}")
            bundle_members.append(member | {"source_root_path": member_ref["source_root_path"]})

    run_command(["nix", "copy", "--to", cache_dir.resolve().as_uri(), *root_paths])

    manifest = {
        "artifact_name": bundle["artifact_name"],
        "build_timing": {
            "completed_at_epoch_seconds": completed_at,
            "duration_seconds": completed_at - started_at,
            "started_at_epoch_seconds": started_at,
        },
        "bundle_id": bundle["bundle_id"],
        "bundle_members": bundle_members,
        "bundle_metadata": bundle["metadata"],
        "cache_uri": cache_dir.resolve().as_uri(),
        "import_paths": root_paths,
        "manifest_kind": "nix_bundle",
        "root_records": root_records,
        "schema_version": 1,
    }

    write_json(output_dir / "manifest.json", manifest)
    print(output_dir / "manifest.json")


def import_bundle(args: argparse.Namespace) -> None:
    bundle_dir = Path(args.bundle_dir).resolve()
    manifest_path = bundle_dir / "manifest.json"
    manifest = load_json_file(manifest_path)

    cache_dir = bundle_dir / "binary-cache"
    if not cache_dir.is_dir():
        fail(f"missing bundle cache directory: {cache_dir}")

    import_paths = manifest.get("import_paths")
    if not isinstance(import_paths, list) or not import_paths or not all(isinstance(path, str) and path for path in import_paths):
        fail("bundle manifest must contain a non-empty import_paths array")

    run_command(["nix", "copy", "--from", cache_dir.resolve().as_uri(), *import_paths])
    run_command(["nix", "path-info", *import_paths])
    print(manifest_path)


def extract_planned_drv_paths(stderr_text: str) -> list[str]:
    planned_drv_paths: list[str] = []
    seen: set[str] = set()

    for line in stderr_text.splitlines():
        if not line.startswith(INTERNAL_JSON_PREFIX):
            continue

        try:
            event = json.loads(line[len(INTERNAL_JSON_PREFIX) :])
        except json.JSONDecodeError:
            continue

        if event.get("action") != "msg":
            continue

        message = str(event.get("msg", "")).strip()
        if DRV_PATH_RE.match(message) and message not in seen:
            planned_drv_paths.append(message)
            seen.add(message)

    return planned_drv_paths


def assert_no_duplicate_build_plan(args: argparse.Namespace) -> None:
    manifest = load_json_file(Path(args.bundle_manifest).resolve())

    protected_drv_paths = {
        record["drv_path"]
        for record in manifest.get("root_records", []) + manifest.get("bundle_members", [])
        if isinstance(record, dict) and isinstance(record.get("drv_path"), str) and record["drv_path"]
    }
    if not protected_drv_paths:
        fail("bundle manifest does not contain any protected drv paths")

    result = run_command(
        ["nix", "build", "--dry-run", "--log-format", "internal-json", *args.installable],
        check=False,
    )
    if result.returncode != 0:
        stderr = result.stderr.strip() or result.stdout.strip()
        fail(stderr or "nix dry-run failed")

    planned_drv_paths = extract_planned_drv_paths(result.stderr)
    duplicate_drv_paths = sorted(protected_drv_paths.intersection(planned_drv_paths))
    report = {
        "bundle_id": manifest.get("bundle_id"),
        "duplicate_drv_paths": duplicate_drv_paths,
        "planned_drv_paths": planned_drv_paths,
        "schema_version": 1,
    }

    if args.output:
        write_json(Path(args.output).resolve(), report)

    if duplicate_drv_paths:
        fail(json.dumps(report, indent=2, sort_keys=True))

    print(json.dumps(report, indent=2, sort_keys=True))


def load_publish_record(path: Path) -> dict:
    payload = load_json_file(path)
    required_fields = [
        "duration_seconds",
        "image_output_id",
        "platform_digest",
        "platform_image_ref",
        "source_bundle_id",
        "source_nar_hash",
        "source_out_path",
    ]
    for field in required_fields:
        value = payload.get(field)
        if value in (None, ""):
            fail(f"publish manifest {path} is missing required field {field}")
    return payload


def manifest_identity(manifest: dict) -> tuple[str, str]:
    root_records = manifest.get("root_records")
    if not isinstance(root_records, list) or not root_records:
        fail(f"manifest {manifest.get('bundle_id')} is missing root records")

    first_root = root_records[0]
    out_path = first_root.get("out_path")
    nar_hash = first_root.get("nar_hash")
    if not isinstance(out_path, str) or not out_path:
        fail(f"manifest {manifest.get('bundle_id')} is missing root out_path")
    if not isinstance(nar_hash, str) or not nar_hash:
        fail(f"manifest {manifest.get('bundle_id')} is missing root nar_hash")
    return out_path, nar_hash


def load_manifest_map(paths: list[str], expected_kind: str) -> dict[str, dict]:
    manifest_map: dict[str, dict] = {}
    for raw_path in paths:
        path = Path(raw_path).resolve()
        manifest = load_json_file(path)
        if manifest.get("manifest_kind") != expected_kind:
            fail(f"manifest {path} has unexpected kind {manifest.get('manifest_kind')}")
        bundle_id = manifest.get("bundle_id")
        if not isinstance(bundle_id, str) or not bundle_id:
            fail(f"manifest {path} is missing bundle_id")
        if bundle_id in manifest_map:
            fail(f"duplicate manifest bundle_id detected: {bundle_id}")
        timing = manifest.get("build_timing")
        if not isinstance(timing, dict) or timing.get("duration_seconds") in (None, ""):
            fail(f"manifest {path} is missing build_timing.duration_seconds")
        manifest_map[bundle_id] = manifest
    return manifest_map


def ensure_required_ids(kind: str, required_ids: list[str], actual_ids: set[str]) -> None:
    missing_ids = sorted(set(required_ids) - actual_ids)
    if missing_ids:
        fail(f"missing required {kind} ids: {', '.join(missing_ids)}")


def audit_run(args: argparse.Namespace) -> None:
    build_manifests = load_manifest_map(args.build_manifest, "nix_bundle")
    image_manifests = load_manifest_map(args.image_manifest, "nix_bundle")
    publish_records = [load_publish_record(Path(path).resolve()) for path in args.publish_manifest]

    ensure_required_ids("build bundle", args.required_build_bundle_id, set(build_manifests))
    ensure_required_ids("image output", args.required_image_output_id, set(image_manifests))
    ensure_required_ids("published image", args.required_publish_output_id, {record["image_output_id"] for record in publish_records})

    for publish_record in publish_records:
        source_bundle_id = publish_record["source_bundle_id"]
        source_manifest = image_manifests.get(source_bundle_id)
        if source_manifest is None:
            fail(f"publish record references missing image bundle id {source_bundle_id}")

        out_path, nar_hash = manifest_identity(source_manifest)
        if publish_record["source_out_path"] != out_path:
            fail(f"publish record for {publish_record['image_output_id']} does not match source out_path")
        if publish_record["source_nar_hash"] != nar_hash:
            fail(f"publish record for {publish_record['image_output_id']} does not match source nar_hash")

    report = {
        "build_bundle_ids": sorted(build_manifests),
        "image_output_ids": sorted(image_manifests),
        "publish_output_ids": sorted(record["image_output_id"] for record in publish_records),
        "schema_version": 1,
    }

    if args.output:
        write_json(Path(args.output).resolve(), report)

    print(json.dumps(report, indent=2, sort_keys=True))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Build, import, and audit immutable Nix CI bundles.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    build_bundle_parser = subparsers.add_parser("build-bundle")
    build_bundle_parser.add_argument("--bundle-json", required=True)
    build_bundle_parser.add_argument("--output-dir", required=True)
    build_bundle_parser.set_defaults(func=build_bundle)

    import_bundle_parser = subparsers.add_parser("import-bundle")
    import_bundle_parser.add_argument("--bundle-dir", required=True)
    import_bundle_parser.set_defaults(func=import_bundle)

    assert_parser = subparsers.add_parser("assert-no-duplicate-build-plan")
    assert_parser.add_argument("--bundle-manifest", required=True)
    assert_parser.add_argument("--installable", action="append", required=True)
    assert_parser.add_argument("--output")
    assert_parser.set_defaults(func=assert_no_duplicate_build_plan)

    audit_parser = subparsers.add_parser("audit-run")
    audit_parser.add_argument("--build-manifest", action="append", required=True)
    audit_parser.add_argument("--image-manifest", action="append", required=True)
    audit_parser.add_argument("--publish-manifest", action="append", required=True)
    audit_parser.add_argument("--required-build-bundle-id", action="append", required=True)
    audit_parser.add_argument("--required-image-output-id", action="append", required=True)
    audit_parser.add_argument("--required-publish-output-id", action="append", required=True)
    audit_parser.add_argument("--output")
    audit_parser.set_defaults(func=audit_run)

    return parser


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
