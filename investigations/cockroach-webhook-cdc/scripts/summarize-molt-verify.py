#!/usr/bin/env python3
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
VERIFY_DIR = ROOT / "output" / "molt-verify"


def parse_json_lines(path: Path):
    records = []
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            records.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return records


def summarize_case(name: str):
    path = VERIFY_DIR / f"{name}.log"
    records = parse_json_lines(path)
    table_summaries = [
        record
        for record in records
        if record.get("type") == "summary" and "table_name" in record
    ]
    completion = next(
        (record for record in reversed(records) if record.get("message") == "verification complete"),
        None,
    )
    return {
        "exit_code": int((VERIFY_DIR / f"{name}.exit_code").read_text().strip()),
        "log_file": path.name,
        "table_summaries": table_summaries,
        "completion": completion,
    }


def main():
    summary = {
        "version": (VERIFY_DIR / "version.txt").read_text().strip(),
        "baseline": summarize_case("baseline"),
        "mismatch": summarize_case("mismatch"),
    }
    output_path = VERIFY_DIR / "summary.json"
    output_path.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
