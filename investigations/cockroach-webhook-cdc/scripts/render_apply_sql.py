#!/usr/bin/env python3
import argparse
import json
from collections import defaultdict
from pathlib import Path


TABLE_ORDER = {
    "customers": 1,
    "products": 2,
    "orders": 3,
    "order_items": 4,
}


def sql_literal(value):
    if value is None:
        return "NULL"
    if isinstance(value, bool):
        return "TRUE" if value else "FALSE"
    if isinstance(value, (int, float)):
        return str(value)
    text = str(value).replace("'", "''")
    return f"'{text}'"


def render_upsert(table, row, primary_keys):
    columns = list(row.keys())
    values = ", ".join(sql_literal(row[column]) for column in columns)
    update_columns = [column for column in columns if column not in primary_keys]
    assignments = ", ".join(f"{column} = EXCLUDED.{column}" for column in update_columns)
    conflict_target = ", ".join(primary_keys)
    column_list = ", ".join(columns)
    if assignments:
        return (
            f"INSERT INTO {table} ({column_list}) VALUES ({values}) "
            f"ON CONFLICT ({conflict_target}) DO UPDATE SET {assignments};"
        )
    return (
        f"INSERT INTO {table} ({column_list}) VALUES ({values}) "
        f"ON CONFLICT ({conflict_target}) DO NOTHING;"
    )


def render_delete(table, row, primary_keys):
    conditions = " AND ".join(f"{column} = {sql_literal(row[column])}" for column in primary_keys)
    return f"DELETE FROM {table} WHERE {conditions};"


def load_batches(paths):
    batches = []
    for path in paths:
        obj = json.loads(path.read_text())["body_json"]
        payload = obj.get("payload")
        if not isinstance(payload, list):
            continue
        batches.append(payload)
    return batches


def iter_arrival_order(batches):
    for batch in batches:
        for row in batch:
            yield row


def iter_batch_topological(batches):
    for batch in batches:
        upserts = []
        deletes = []
        for row in batch:
            if row["op"] == "d":
                deletes.append(row)
            else:
                upserts.append(row)

        upserts.sort(key=lambda row: TABLE_ORDER[row["source"]["table_name"]])
        deletes.sort(key=lambda row: TABLE_ORDER[row["source"]["table_name"]], reverse=True)

        for row in upserts:
            yield row
        for row in deletes:
            yield row


def iter_collapsed_final_state(batches):
    state = defaultdict(dict)

    for batch in batches:
        for row in batch:
            table = row["source"]["table_name"]
            primary_keys = tuple(row["source"]["primary_keys"])
            key_source = row["before"] if row["op"] == "d" else row["after"]
            key = tuple(key_source[column] for column in primary_keys)
            if row["op"] == "d":
                state[table][key] = None
            else:
                state[table][key] = row["after"]

    for table, _ in sorted(TABLE_ORDER.items(), key=lambda item: item[1]):
        primary_keys = None
        rows = state.get(table, {})
        for key, row in sorted(rows.items()):
            if row is None:
                continue
            if primary_keys is None:
                # Use natural primary key ordering from the final row shape.
                if table == "order_items":
                    primary_keys = ["order_id", "line_no"]
                else:
                    primary_keys = ["id"]
            yield {
                "op": "c",
                "after": row,
                "source": {
                    "table_name": table,
                    "primary_keys": primary_keys,
                },
            }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--requests-dir", required=True)
    parser.add_argument("--glob", required=True)
    parser.add_argument("--db-name", required=True)
    parser.add_argument(
        "--mode",
        choices=["arrival", "batch-topological", "collapsed-final-state"],
        required=True,
    )
    args = parser.parse_args()

    paths = sorted(Path(args.requests_dir).glob(args.glob))
    if not paths:
        raise SystemExit("no request files matched")

    batches = load_batches(paths)
    if args.mode == "arrival":
        iterator = iter_arrival_order(batches)
    elif args.mode == "batch-topological":
        iterator = iter_batch_topological(batches)
    else:
        iterator = iter_collapsed_final_state(batches)

    print(f"\\connect {args.db_name}")
    print("BEGIN;")

    for row in iterator:
        table = row["source"]["table_name"]
        primary_keys = row["source"]["primary_keys"]
        if row["op"] == "d":
            print(render_delete(table, row["before"], primary_keys))
        else:
            print(render_upsert(table, row["after"], primary_keys))

    print("COMMIT;")


if __name__ == "__main__":
    main()
