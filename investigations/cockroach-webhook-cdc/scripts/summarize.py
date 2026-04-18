#!/usr/bin/env python3
import json
from collections import Counter, defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REQUEST_DIR = ROOT / "output" / "requests"
SUMMARY_PATH = ROOT / "output" / "summary.json"


def normalize_event(item):
    if "payload" in item and isinstance(item["payload"], dict):
        payload = dict(item["payload"])
        if "schema" in item:
            payload["schema"] = item["schema"]
        return payload
    return item


def classify_event(event):
    if "resolved" in event:
        return "resolved"
    if event.get("op"):
        return event["op"]
    after = event.get("after")
    before = event.get("before")
    if after is None and before is not None:
        return "d"
    if after is not None and before is not None:
        return "u"
    if after is not None:
        return "c_or_u"
    return "unknown"


def main():
    requests = sorted(REQUEST_DIR.glob("*.json"))
    summary = {
        "request_count": len(requests),
        "paths": {},
        "headers_seen": Counter(),
        "example_files": {},
    }
    path_stats = defaultdict(
        lambda: {
            "requests": 0,
            "payload_entries": 0,
            "events_by_kind": Counter(),
            "topics": Counter(),
            "sample_files": {},
        }
    )

    for request_path in requests:
        request = json.loads(request_path.read_text())
        path = request["path"]
        stats = path_stats[path]
        stats["requests"] += 1

        headers = request.get("headers", {})
        for header_name in headers:
            summary["headers_seen"][header_name] += 1

        body = request.get("body_json")
        if not isinstance(body, dict):
            continue

        payload = body.get("payload", [])
        if not isinstance(payload, list):
            continue

        stats["payload_entries"] += len(payload)

        for item in payload:
            event = normalize_event(item)
            kind = classify_event(event)
            stats["events_by_kind"][kind] += 1

            topic = event.get("topic")
            if topic:
                stats["topics"][topic] += 1

            if kind not in stats["sample_files"]:
                stats["sample_files"][kind] = request_path.name
            if topic and topic not in stats["sample_files"]:
                stats["sample_files"][topic] = request_path.name

    summary["headers_seen"] = dict(summary["headers_seen"])
    for path, stats in path_stats.items():
        summary["paths"][path] = {
            "requests": stats["requests"],
            "payload_entries": stats["payload_entries"],
            "events_by_kind": dict(stats["events_by_kind"]),
            "topics": dict(stats["topics"]),
            "sample_files": stats["sample_files"],
        }

    SUMMARY_PATH.write_text(json.dumps(summary, indent=2, sort_keys=True) + "\n")
    print(json.dumps(summary, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
