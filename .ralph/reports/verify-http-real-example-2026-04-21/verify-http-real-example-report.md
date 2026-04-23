# Verify HTTP Server Real Curl Report - 2026-04-21

This report is intentionally exhaustive. It contains the actual configs, SQL files, psql outputs, curl commands, raw HTTP responses, and service logs captured during the live run. The successful live run used PostgreSQL source on localhost:16432, CockroachDB destination on localhost:26262, and verify-service HTTP listener on localhost:18081. The listener was plain HTTP with no listener auth or mTLS in the main config because `listener.tls` is omitted. Database connections were passwordless because both database URLs omit a password and use `sslmode=disable` for the main HTTP-only example.

The POST /jobs filters are regexes because the request body fields are include/exclude filters compiled as POSIX regular expressions: `include_schema`, `include_table`, `exclude_schema`, `exclude_table`. Empty include filters default to the service default filter. A bad regex is rejected before a job starts; the raw response is included below.

Raw table HTTP output is enabled by `verify.raw_table_output: true`; disabling it returns HTTP 403. Both enabled and disabled outputs are included below.

Important setup correction: a first attempt to start a fresh Cockroach container through the image entrypoint failed because of the image wrapper/listen-address behavior. I then started CockroachDB directly through `/cockroach/cockroach` on host port 26262 and ran the successful proof against that real CockroachDB instance. Failed setup transcripts are included at the end; the authoritative run is `transcript-successful-http-run.txt`.

## Config: verify-http-raw-disabled.yml

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/config/verify-http-raw-disabled.yml`

```yaml
listener:
  bind_addr: 0.0.0.0:18081
verify:
  raw_table_output: false
  source:
    url: postgresql://postgres@localhost:16432/verify_report?sslmode=disable
  destination:
    url: postgresql://root@localhost:26262/verify_report?sslmode=disable

```

## Config: verify-http-raw-wrong-cert.yml

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/config/verify-http-raw-wrong-cert.yml`

```yaml
listener:
  bind_addr: 0.0.0.0:18081
verify:
  raw_table_output: true
  source:
    url: postgresql://postgres@localhost:16432/verify_report?sslmode=verify-full
    ca_cert_path: /config/certs/not-a-real-ca.crt
  destination:
    url: postgresql://root@localhost:26262/verify_report?sslmode=disable

```

## Config: verify-http-raw-wrong-database.yml

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/config/verify-http-raw-wrong-database.yml`

```yaml
listener:
  bind_addr: 0.0.0.0:18081
verify:
  raw_table_output: true
  source:
    url: postgresql://postgres@localhost:16432/database_that_does_not_exist?sslmode=disable
  destination:
    url: postgresql://root@localhost:26262/verify_report?sslmode=disable

```

## Config: verify-http-raw-wrong-permission.yml

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/config/verify-http-raw-wrong-permission.yml`

```yaml
listener:
  bind_addr: 0.0.0.0:18081
verify:
  raw_table_output: true
  source:
    url: postgresql://verify_no_select@localhost:16432/verify_report?sslmode=disable
  destination:
    url: postgresql://root@localhost:26262/verify_report?sslmode=disable

```

## Config: verify-http-raw.yml

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/config/verify-http-raw.yml`

```yaml
listener:
  bind_addr: 0.0.0.0:18081
verify:
  raw_table_output: true
  source:
    url: postgresql://postgres@localhost:16432/verify_report?sslmode=disable
  destination:
    url: postgresql://root@localhost:26262/verify_report?sslmode=disable

```

## SQL: destination_data_equal.sql

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_equal.sql`

```sql
INSERT INTO public.customers (customer_id, email, display_name) VALUES
  (1001, 'ada@example.test', 'Ada Lovelace'),
  (1002, 'grace@example.test', 'Grace Hopper');
INSERT INTO public.orders (order_id, customer_id, order_code, total_cents) VALUES
  (5001, 1001, 'ORD-ADA-001', 12500),
  (5002, 1002, 'ORD-GRACE-001', 19999);

```

## SQL: destination_data_mismatch.sql

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_mismatch.sql`

```sql
INSERT INTO public.customers (customer_id, email, display_name) VALUES
  (1001, 'ada@example.test', 'Ada Lovelace'),
  (1002, 'grace@example.test', 'Rear Admiral Grace Hopper');
INSERT INTO public.orders (order_id, customer_id, order_code, total_cents) VALUES
  (5001, 1001, 'ORD-ADA-001', 12500),
  (5002, 1002, 'ORD-GRACE-001', 20999),
  (5003, 1002, 'ORD-GRACE-EXTRA', 777);

```

## SQL: destination_schema_equal.sql

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql`

```sql
DROP DATABASE IF EXISTS verify_report CASCADE;
CREATE DATABASE verify_report;
USE verify_report;
CREATE TABLE public.customers (
  customer_id INT PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL
);
CREATE TABLE public.orders (
  order_id INT PRIMARY KEY,
  customer_id INT NOT NULL REFERENCES public.customers(customer_id),
  order_code TEXT NOT NULL UNIQUE,
  total_cents INT NOT NULL
);

```

## SQL: destination_schema_full_mismatch.sql

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_full_mismatch.sql`

```sql
DROP DATABASE IF EXISTS verify_report CASCADE;
CREATE DATABASE verify_report;
USE verify_report;
CREATE TABLE public.customers (
  customer_id INT PRIMARY KEY,
  email STRING NOT NULL,
  display_name STRING NOT NULL,
  loyalty_tier STRING NOT NULL DEFAULT 'standard'
);
CREATE TABLE public.orders (
  order_id INT PRIMARY KEY,
  customer_id INT NOT NULL,
  order_code STRING NOT NULL,
  total_cents STRING NOT NULL,
  status STRING NOT NULL DEFAULT 'open'
);

```

## SQL: destination_schema_partial_mismatch.sql

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_partial_mismatch.sql`

```sql
DROP DATABASE IF EXISTS verify_report CASCADE;
CREATE DATABASE verify_report;
USE verify_report;
CREATE TABLE public.customers (
  customer_id INT PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL
);
CREATE TABLE public.orders (
  order_id INT PRIMARY KEY,
  customer_id INT NOT NULL REFERENCES public.customers(customer_id),
  order_code TEXT NOT NULL UNIQUE,
  total_cents STRING NOT NULL
);

```

## SQL: source_data.sql

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/sql/source_data.sql`

```sql
INSERT INTO public.customers (customer_id, email, display_name) VALUES
  (1001, 'ada@example.test', 'Ada Lovelace'),
  (1002, 'grace@example.test', 'Grace Hopper');
INSERT INTO public.orders (order_id, customer_id, order_code, total_cents) VALUES
  (5001, 1001, 'ORD-ADA-001', 12500),
  (5002, 1002, 'ORD-GRACE-001', 19999);

```

## SQL: source_schema.sql

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/sql/source_schema.sql`

```sql
DROP DATABASE IF EXISTS verify_report;
CREATE DATABASE verify_report;
\connect verify_report
CREATE TABLE public.customers (
  customer_id INT PRIMARY KEY,
  email TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL
);
CREATE TABLE public.orders (
  order_id INT PRIMARY KEY,
  customer_id INT NOT NULL REFERENCES public.customers(customer_id),
  order_code TEXT NOT NULL UNIQUE,
  total_cents INT NOT NULL
);

```

## Schema/Data Output: source-schema-customers.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/source-schema-customers.txt`

```text
                                           Table "public.customers"
    Column    |  Type   | Collation | Nullable | Default | Storage  | Compression | Stats target | Description 
--------------+---------+-----------+----------+---------+----------+-------------+--------------+-------------
 customer_id  | integer |           | not null |         | plain    |             |              | 
 email        | text    |           | not null |         | extended |             |              | 
 display_name | text    |           | not null |         | extended |             |              | 
Indexes:
    "customers_pkey" PRIMARY KEY, btree (customer_id)
    "customers_email_key" UNIQUE CONSTRAINT, btree (email)
Referenced by:
    TABLE "orders" CONSTRAINT "orders_customer_id_fkey" FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
Access method: heap


```

## Schema/Data Output: source-schema-orders.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/source-schema-orders.txt`

```text
                                            Table "public.orders"
   Column    |  Type   | Collation | Nullable | Default | Storage  | Compression | Stats target | Description 
-------------+---------+-----------+----------+---------+----------+-------------+--------------+-------------
 order_id    | integer |           | not null |         | plain    |             |              | 
 customer_id | integer |           | not null |         | plain    |             |              | 
 order_code  | text    |           | not null |         | extended |             |              | 
 total_cents | integer |           | not null |         | plain    |             |              | 
Indexes:
    "orders_pkey" PRIMARY KEY, btree (order_id)
    "orders_order_code_key" UNIQUE CONSTRAINT, btree (order_code)
Foreign-key constraints:
    "orders_customer_id_fkey" FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
Access method: heap


```

## Schema/Data Output: destination-schema-customers.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/destination-schema-customers.txt`

```text
    table_name    |                         create_statement                         
------------------+------------------------------------------------------------------
 public.customers | CREATE TABLE public.customers (                                 +
                  |         customer_id INT8 NOT NULL,                              +
                  |         email STRING NOT NULL,                                  +
                  |         display_name STRING NOT NULL,                           +
                  |         CONSTRAINT customers_pkey PRIMARY KEY (customer_id ASC),+
                  |         UNIQUE INDEX customers_email_key (email ASC)            +
                  | ) WITH (schema_locked = true);
(1 row)


```

## Schema/Data Output: destination-schema-orders.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/destination-schema-orders.txt`

```text
  table_name   |                                                create_statement                                                
---------------+----------------------------------------------------------------------------------------------------------------
 public.orders | CREATE TABLE public.orders (                                                                                  +
               |         order_id INT8 NOT NULL,                                                                               +
               |         customer_id INT8 NOT NULL,                                                                            +
               |         order_code STRING NOT NULL,                                                                           +
               |         total_cents INT8 NOT NULL,                                                                            +
               |         CONSTRAINT orders_pkey PRIMARY KEY (order_id ASC),                                                    +
               |         CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(customer_id),+
               |         UNIQUE INDEX orders_order_code_key (order_code ASC)                                                   +
               | ) WITH (schema_locked = true);
(1 row)


```

## Schema/Data Output: source-data.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/source-data.txt`

```text
 customer_id |       email        | display_name 
-------------+--------------------+--------------
        1001 | ada@example.test   | Ada Lovelace
        1002 | grace@example.test | Grace Hopper
(2 rows)

 order_id | customer_id |  order_code   | total_cents 
----------+-------------+---------------+-------------
     5001 |        1001 | ORD-ADA-001   |       12500
     5002 |        1002 | ORD-GRACE-001 |       19999
(2 rows)


```

## Schema/Data Output: destination-data-equal.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/destination-data-equal.txt`

```text
 customer_id |       email        | display_name 
-------------+--------------------+--------------
        1001 | ada@example.test   | Ada Lovelace
        1002 | grace@example.test | Grace Hopper
(2 rows)

 order_id | customer_id |  order_code   | total_cents 
----------+-------------+---------------+-------------
     5001 |        1001 | ORD-ADA-001   |       12500
     5002 |        1002 | ORD-GRACE-001 |       19999
(2 rows)


```

## HTTP Raw Response: bad-regex-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/bad-regex-post-job.txt`

```http
HTTP/1.1 400 Bad Request
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 201

{"error":{"category":"request_validation","code":"invalid_filter","message":"request validation failed","details":[{"field":"include_schema","reason":"error parsing regexp: missing closing ]: `[`"}]}}

```

## HTTP Raw Response: data-mismatch-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/data-mismatch-get-job.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:22 GMT
Content-Length: 1485

{"job_id":"job-000002","status":"failed","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":true},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":1,"num_missing":0,"num_mismatch":1,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":1,"num_missing":0,"num_mismatch":1,"num_column_mismatch":0,"num_extraneous":1,"num_live_retry":0}],"findings":[{"kind":"mismatching_row","schema":"public","table":"customers","primary_key":{"customer_id":"1002"},"mismatching_columns":["display_name"],"source_values":{"display_name":"Grace Hopper"},"destination_values":{"display_name":"Rear Admiral Grace Hopper"}},{"kind":"extraneous_row","schema":"public","table":"orders","primary_key":{"order_id":"5003"}},{"kind":"mismatching_row","schema":"public","table":"orders","primary_key":{"order_id":"5002"},"mismatching_columns":["total_cents"],"source_values":{"total_cents":"19999"},"destination_values":{"total_cents":"20999"}}],"mismatch_summary":{"has_mismatches":true,"affected_tables":[{"schema":"public","table":"customers"},{"schema":"public","table":"orders"}],"counts_by_kind":{"extraneous_row":1,"mismatching_row":2}}},"failure":{"category":"mismatch","code":"mismatch_detected","message":"verify detected mismatches in 2 table","details":[{"reason":"mismatch detected for public.customers"},{"reason":"mismatch detected for public.orders"}]}}

```

## HTTP Raw Response: data-mismatch-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/data-mismatch-post-job.txt`

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:19 GMT
Content-Length: 43

{"job_id":"job-000002","status":"running"}

```

## HTTP Raw Response: equal-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/equal-get-job.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:19 GMT
Content-Length: 584

{"job_id":"job-000001","status":"succeeded","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":false},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0}],"findings":[],"mismatch_summary":{"has_mismatches":false,"affected_tables":[],"counts_by_kind":{}}}}

```

## HTTP Raw Response: equal-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/equal-post-job.txt`

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}

```

## HTTP Raw Response: full-schema-mismatch-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/full-schema-mismatch-get-job.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 1292

{"job_id":"job-000004","status":"failed","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":true},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0}],"findings":[{"kind":"mismatching_table_definition","schema":"public","table":"customers","message":"extraneous column loyalty_tier found"},{"kind":"mismatching_table_definition","schema":"public","table":"orders","message":"column type mismatch on total_cents: int4 vs text"},{"kind":"mismatching_table_definition","schema":"public","table":"orders","message":"extraneous column status found"}],"mismatch_summary":{"has_mismatches":true,"affected_tables":[{"schema":"public","table":"customers"},{"schema":"public","table":"orders"}],"counts_by_kind":{"mismatching_table_definition":3}}},"failure":{"category":"mismatch","code":"mismatch_detected","message":"verify detected mismatches in 2 table","details":[{"reason":"mismatch detected for public.customers"},{"reason":"mismatch detected for public.orders"}]}}

```

## HTTP Raw Response: full-schema-mismatch-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/full-schema-mismatch-post-job.txt`

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:25 GMT
Content-Length: 43

{"job_id":"job-000004","status":"running"}

```

## HTTP Raw Response: initial-metrics.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/initial-metrics.txt`

```http
HTTP/1.1 200 OK
Content-Type: text/plain; version=0.0.4; charset=utf-8
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 602

# HELP cockroach_migration_tool_verify_active_jobs Current number of active verify jobs.
# TYPE cockroach_migration_tool_verify_active_jobs gauge
cockroach_migration_tool_verify_active_jobs 0
# HELP cockroach_migration_tool_verify_jobs_total Current number of verify jobs by lifecycle status.
# TYPE cockroach_migration_tool_verify_jobs_total gauge
cockroach_migration_tool_verify_jobs_total{status="failed"} 0
cockroach_migration_tool_verify_jobs_total{status="running"} 0
cockroach_migration_tool_verify_jobs_total{status="stopped"} 0
cockroach_migration_tool_verify_jobs_total{status="succeeded"} 0

```

## HTTP Raw Response: metrics.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/metrics.txt`

```http
HTTP/1.1 200 OK
Content-Type: text/plain; version=0.0.4; charset=utf-8
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 602

# HELP cockroach_migration_tool_verify_active_jobs Current number of active verify jobs.
# TYPE cockroach_migration_tool_verify_active_jobs gauge
cockroach_migration_tool_verify_active_jobs 0
# HELP cockroach_migration_tool_verify_jobs_total Current number of verify jobs by lifecycle status.
# TYPE cockroach_migration_tool_verify_jobs_total gauge
cockroach_migration_tool_verify_jobs_total{status="failed"} 1
cockroach_migration_tool_verify_jobs_total{status="running"} 0
cockroach_migration_tool_verify_jobs_total{status="stopped"} 0
cockroach_migration_tool_verify_jobs_total{status="succeeded"} 0

```

## HTTP Raw Response: not-found-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/not-found-get-job.txt`

```http
HTTP/1.1 404 Not Found
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 84

{"error":{"category":"job_state","code":"job_not_found","message":"job not found"}}

```

## HTTP Raw Response: partial-schema-mismatch-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/partial-schema-mismatch-get-job.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:25 GMT
Content-Length: 955

{"job_id":"job-000003","status":"failed","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":true},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0}],"findings":[{"kind":"mismatching_table_definition","schema":"public","table":"orders","message":"column type mismatch on total_cents: int4 vs text"}],"mismatch_summary":{"has_mismatches":true,"affected_tables":[{"schema":"public","table":"orders"}],"counts_by_kind":{"mismatching_table_definition":1}}},"failure":{"category":"mismatch","code":"mismatch_detected","message":"verify detected mismatches in 1 table","details":[{"reason":"mismatch detected for public.orders"}]}}

```

## HTTP Raw Response: partial-schema-mismatch-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/partial-schema-mismatch-post-job.txt`

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:22 GMT
Content-Length: 43

{"job_id":"job-000003","status":"running"}

```

## HTTP Raw Response: raw-destination-orders.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/raw-destination-orders.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 305

{"database":"destination","schema":"public","table":"orders","columns":["order_id","customer_id","order_code","total_cents"],"rows":[{"customer_id":1001,"order_code":"ORD-ADA-001","order_id":5001,"total_cents":12500},{"customer_id":1002,"order_code":"ORD-GRACE-001","order_id":5002,"total_cents":19999}]}

```

## HTTP Raw Response: raw-disabled.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/raw-disabled.txt`

```http
HTTP/1.1 403 Forbidden
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:40 GMT
Content-Length: 41

{"error":"raw table output is disabled"}

```

## HTTP Raw Response: raw-invalid-identifier.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/raw-invalid-identifier.txt`

```http
HTTP/1.1 400 Bad Request
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 51

{"error":"schema must be a simple SQL identifier"}

```

## HTTP Raw Response: raw-source-customers.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/raw-source-customers.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 276

{"database":"source","schema":"public","table":"customers","columns":["customer_id","email","display_name"],"rows":[{"customer_id":1001,"display_name":"Ada Lovelace","email":"ada@example.test"},{"customer_id":1002,"display_name":"Grace Hopper","email":"grace@example.test"}]}

```

## HTTP Raw Response: wrong-cert-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/wrong-cert-get-job.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:40 GMT
Content-Length: 685

{"job_id":"job-000001","status":"failed","failure":{"category":"source_access","code":"connection_failed","message":"source connection failed: cannot parse `postgresql://postgres@localhost:16432/verify_report?sslmode=verify-full\u0026sslrootcert=%2Fconfig%2Fcerts%2Fnot-a-real-ca.crt`: failed to configure TLS (unable to read CA file: open /config/certs/not-a-real-ca.crt: no such file or directory)","details":[{"reason":"cannot parse `postgresql://postgres@localhost:16432/verify_report?sslmode=verify-full\u0026sslrootcert=%2Fconfig%2Fcerts%2Fnot-a-real-ca.crt`: failed to configure TLS (unable to read CA file: open /config/certs/not-a-real-ca.crt: no such file or directory)"}]}}

```

## HTTP Raw Response: wrong-cert-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/wrong-cert-post-job.txt`

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:37 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}

```

## HTTP Raw Response: wrong-database-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/wrong-database-get-job.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:32 GMT
Content-Length: 601

{"job_id":"job-000001","status":"failed","failure":{"category":"source_access","code":"connection_failed","message":"source connection failed: error connect: failed to connect to `user=postgres database=database_that_does_not_exist`: 127.0.0.1:16432 (localhost): server error: FATAL: database \"database_that_does_not_exist\" does not exist (SQLSTATE 3D000)","details":[{"reason":"error connect: failed to connect to `user=postgres database=database_that_does_not_exist`: 127.0.0.1:16432 (localhost): server error: FATAL: database \"database_that_does_not_exist\" does not exist (SQLSTATE 3D000)"}]}}

```

## HTTP Raw Response: wrong-database-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/wrong-database-post-job.txt`

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}

```

## HTTP Raw Response: wrong-permission-get-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/wrong-permission-get-job.txt`

```http
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:35 GMT
Content-Length: 421

{"job_id":"job-000001","status":"failed","failure":{"category":"verify_execution","code":"verify_failed","message":"verify execution failed: error splitting tables: cannot get minimum of public.customers: ERROR: permission denied for table customers (SQLSTATE 42501)","details":[{"reason":"error splitting tables: cannot get minimum of public.customers: ERROR: permission denied for table customers (SQLSTATE 42501)"}]}}

```

## HTTP Raw Response: wrong-permission-post-job.txt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/http/wrong-permission-post-job.txt`

```http
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:32 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}

```

## Verify Service Log: verify-service.log

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/verify-service.log`

```json
{"level":"info","service":"verify","event":"runtime.starting","timestamp":"2026-04-21T07:23:15.336027981Z","message":"verify-service runtime starting"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.421726201Z","message":"starting verify on public.customers, shard 1/8, range: [<beginning> - 1002)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.421764326Z","message":"starting verify on public.customers, shard 8/8, range: [1008 - <end>]"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.421888118Z","message":"starting verify on public.customers, shard 4/8, range: [1004 - 1005)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.421911826Z","message":"starting verify on public.customers, shard 2/8, range: [1002 - 1003)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.421966743Z","message":"starting verify on public.customers, shard 3/8, range: [1003 - 1004)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.422369827Z","message":"starting verify on public.customers, shard 6/8, range: [1006 - 1007)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.422430744Z","message":"starting verify on public.customers, shard 5/8, range: [1005 - 1006)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.422482577Z","message":"starting verify on public.customers, shard 7/8, range: [1007 - 1008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.428813754Z","message":"finished row verification on public.customers (shard 2/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.428900921Z","message":"starting verify on public.orders, shard 1/8, range: [<beginning> - 5002)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.429080588Z","message":"finished row verification on public.customers (shard 6/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.429164671Z","message":"starting verify on public.orders, shard 2/8, range: [5002 - 5003)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.429168588Z","message":"finished row verification on public.customers (shard 7/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.429223755Z","message":"starting verify on public.orders, shard 3/8, range: [5003 - 5004)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.429230296Z","message":"finished row verification on public.customers (shard 3/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.429264755Z","message":"starting verify on public.orders, shard 4/8, range: [5004 - 5005)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.429773672Z","message":"finished row verification on public.customers (shard 1/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.429825922Z","message":"starting verify on public.orders, shard 5/8, range: [5005 - 5006)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.4315858Z","message":"finished row verification on public.customers (shard 8/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.431654092Z","message":"starting verify on public.orders, shard 6/8, range: [5006 - 5007)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.431757425Z","message":"finished row verification on public.customers (shard 5/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.432215343Z","message":"starting verify on public.orders, shard 7/8, range: [5007 - 5008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.433096011Z","message":"finished row verification on public.customers (shard 4/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:16.433173053Z","message":"starting verify on public.orders, shard 8/8, range: [5008 - <end>]"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.435398723Z","message":"finished row verification on public.orders (shard 2/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.435501348Z","message":"finished row verification on public.orders (shard 4/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.435779515Z","message":"finished row verification on public.orders (shard 1/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.438319769Z","message":"finished row verification on public.orders (shard 6/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.440725648Z","message":"finished row verification on public.orders (shard 8/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.441988359Z","message":"finished row verification on public.orders (shard 7/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.441999775Z","message":"finished row verification on public.orders (shard 3/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:16.44199065Z","message":"finished row verification on public.orders (shard 5/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649096187Z","message":"starting verify on public.customers, shard 1/8, range: [<beginning> - 1002)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649186771Z","message":"starting verify on public.customers, shard 8/8, range: [1008 - <end>]"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649207687Z","message":"starting verify on public.customers, shard 4/8, range: [1004 - 1005)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649241771Z","message":"starting verify on public.customers, shard 3/8, range: [1003 - 1004)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649267312Z","message":"starting verify on public.customers, shard 2/8, range: [1002 - 1003)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649361146Z","message":"starting verify on public.customers, shard 5/8, range: [1005 - 1006)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649381479Z","message":"starting verify on public.customers, shard 6/8, range: [1006 - 1007)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.649453729Z","message":"starting verify on public.customers, shard 7/8, range: [1007 - 1008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.654204154Z","message":"finished row verification on public.customers (shard 3/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.654253612Z","message":"starting verify on public.orders, shard 1/8, range: [<beginning> - 5002)"}
{"level":"warn","service":"verify","type":"data","table_schema":"public","table_name":"customers","source_values":{"display_name":"Grace Hopper"},"target_values":{"display_name":"Rear Admiral Grace Hopper"},"primary_key":["1002"],"timestamp":"2026-04-21T07:23:19.654456613Z","message":"mismatching row value"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":1,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.654478071Z","message":"finished row verification on public.customers (shard 2/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.654542654Z","message":"starting verify on public.orders, shard 2/8, range: [5002 - 5003)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.654562779Z","message":"finished row verification on public.customers (shard 1/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.654640321Z","message":"starting verify on public.orders, shard 3/8, range: [5003 - 5004)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.654892155Z","message":"finished row verification on public.customers (shard 4/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.654941738Z","message":"starting verify on public.orders, shard 4/8, range: [5004 - 5005)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.656068157Z","message":"finished row verification on public.customers (shard 8/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.65611999Z","message":"starting verify on public.orders, shard 5/8, range: [5005 - 5006)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.656781866Z","message":"finished row verification on public.customers (shard 7/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.656863491Z","message":"finished row verification on public.customers (shard 6/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.656880825Z","message":"starting verify on public.orders, shard 6/8, range: [5006 - 5007)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.656927033Z","message":"starting verify on public.orders, shard 7/8, range: [5007 - 5008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.656977783Z","message":"finished row verification on public.customers (shard 5/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:19.657050783Z","message":"starting verify on public.orders, shard 8/8, range: [5008 - <end>]"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662235Z","message":"finished row verification on public.orders (shard 4/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662235125Z","message":"finished row verification on public.orders (shard 6/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662278125Z","message":"finished row verification on public.orders (shard 8/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662291209Z","message":"finished row verification on public.orders (shard 7/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662338334Z","message":"finished row verification on public.orders (shard 5/8)"}
{"level":"warn","service":"verify","type":"data","table_schema":"public","table_name":"orders","primary_key":["5003"],"timestamp":"2026-04-21T07:23:19.662463376Z","message":"extraneous row"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":1,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662479209Z","message":"finished row verification on public.orders (shard 3/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662510542Z","message":"finished row verification on public.orders (shard 1/8)"}
{"level":"warn","service":"verify","type":"data","table_schema":"public","table_name":"orders","source_values":{"total_cents":"19999"},"target_values":{"total_cents":"20999"},"primary_key":["5002"],"timestamp":"2026-04-21T07:23:19.662637001Z","message":"mismatching row value"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":1,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:19.662648751Z","message":"finished row verification on public.orders (shard 2/8)"}
{"level":"warn","service":"verify","type":"data","table_schema":"public","table_name":"orders","mismatch_info":"column type mismatch on total_cents: int4 vs text","timestamp":"2026-04-21T07:23:22.851860509Z","message":"mismatching table definition"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.853656636Z","message":"starting verify on public.customers, shard 1/8, range: [<beginning> - 1002)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.85377622Z","message":"starting verify on public.customers, shard 8/8, range: [1008 - <end>]"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.853822803Z","message":"starting verify on public.customers, shard 4/8, range: [1004 - 1005)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.853848553Z","message":"starting verify on public.customers, shard 5/8, range: [1005 - 1006)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.853869553Z","message":"starting verify on public.customers, shard 2/8, range: [1002 - 1003)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.85390847Z","message":"starting verify on public.customers, shard 3/8, range: [1003 - 1004)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.853922929Z","message":"starting verify on public.customers, shard 6/8, range: [1006 - 1007)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.853978929Z","message":"starting verify on public.customers, shard 7/8, range: [1007 - 1008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.860637273Z","message":"finished row verification on public.customers (shard 3/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.860711023Z","message":"starting verify on public.orders, shard 1/8, range: [<beginning> - 5002)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.860894357Z","message":"finished row verification on public.customers (shard 8/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.860933607Z","message":"finished row verification on public.customers (shard 2/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.860987773Z","message":"starting verify on public.orders, shard 2/8, range: [5002 - 5003)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.861069482Z","message":"starting verify on public.orders, shard 3/8, range: [5003 - 5004)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.861324191Z","message":"finished row verification on public.customers (shard 6/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.861393899Z","message":"starting verify on public.orders, shard 4/8, range: [5004 - 5005)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.861922983Z","message":"finished row verification on public.customers (shard 1/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.862049775Z","message":"starting verify on public.orders, shard 5/8, range: [5005 - 5006)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.862711026Z","message":"finished row verification on public.customers (shard 7/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.862782151Z","message":"starting verify on public.orders, shard 6/8, range: [5006 - 5007)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.86320961Z","message":"finished row verification on public.customers (shard 5/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.863272527Z","message":"starting verify on public.orders, shard 7/8, range: [5007 - 5008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.86490028Z","message":"finished row verification on public.customers (shard 4/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:22.865016572Z","message":"starting verify on public.orders, shard 8/8, range: [5008 - <end>]"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.867562534Z","message":"finished row verification on public.orders (shard 1/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.868895453Z","message":"finished row verification on public.orders (shard 4/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.870054621Z","message":"finished row verification on public.orders (shard 6/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.87005908Z","message":"finished row verification on public.orders (shard 2/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.87047433Z","message":"finished row verification on public.orders (shard 7/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.870937789Z","message":"finished row verification on public.orders (shard 5/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.871001373Z","message":"finished row verification on public.orders (shard 3/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:22.871880124Z","message":"finished row verification on public.orders (shard 8/8)"}
{"level":"warn","service":"verify","type":"data","table_schema":"public","table_name":"customers","mismatch_info":"extraneous column loyalty_tier found","timestamp":"2026-04-21T07:23:26.001734411Z","message":"mismatching table definition"}
{"level":"warn","service":"verify","type":"data","table_schema":"public","table_name":"orders","mismatch_info":"column type mismatch on total_cents: int4 vs text","timestamp":"2026-04-21T07:23:26.001756202Z","message":"mismatching table definition"}
{"level":"warn","service":"verify","type":"data","table_schema":"public","table_name":"orders","mismatch_info":"extraneous column status found","timestamp":"2026-04-21T07:23:26.001758827Z","message":"mismatching table definition"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003659164Z","message":"starting verify on public.customers, shard 1/8, range: [<beginning> - 1002)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003793497Z","message":"starting verify on public.customers, shard 8/8, range: [1008 - <end>]"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003804206Z","message":"starting verify on public.customers, shard 4/8, range: [1004 - 1005)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003836456Z","message":"starting verify on public.customers, shard 2/8, range: [1002 - 1003)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003850123Z","message":"starting verify on public.customers, shard 3/8, range: [1003 - 1004)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003901539Z","message":"starting verify on public.customers, shard 5/8, range: [1005 - 1006)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003915414Z","message":"starting verify on public.customers, shard 6/8, range: [1006 - 1007)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.003926914Z","message":"starting verify on public.customers, shard 7/8, range: [1007 - 1008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.009823799Z","message":"finished row verification on public.customers (shard 5/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.009841632Z","message":"finished row verification on public.customers (shard 6/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.009879424Z","message":"starting verify on public.orders, shard 1/8, range: [<beginning> - 5002)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.009954257Z","message":"starting verify on public.orders, shard 2/8, range: [5002 - 5003)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.01022755Z","message":"finished row verification on public.customers (shard 7/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.01027555Z","message":"finished row verification on public.customers (shard 4/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.010304591Z","message":"starting verify on public.orders, shard 3/8, range: [5003 - 5004)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.010400133Z","message":"starting verify on public.orders, shard 4/8, range: [5004 - 5005)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.010524592Z","message":"finished row verification on public.customers (shard 3/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.0105798Z","message":"starting verify on public.orders, shard 5/8, range: [5005 - 5006)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.015388475Z","message":"finished row verification on public.customers (shard 8/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.015478058Z","message":"starting verify on public.orders, shard 6/8, range: [5006 - 5007)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.0156326Z","message":"finished row verification on public.customers (shard 1/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.01567535Z","message":"starting verify on public.orders, shard 7/8, range: [5007 - 5008)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.024444281Z","message":"finished row verification on public.orders (shard 5/8)"}
{"level":"info","service":"verify","timestamp":"2026-04-21T07:23:26.024510073Z","message":"starting verify on public.orders, shard 8/8, range: [5008 - <end>]"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.024863948Z","message":"finished row verification on public.orders (shard 3/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.024939532Z","message":"finished row verification on public.orders (shard 4/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.025634033Z","message":"finished row verification on public.orders (shard 2/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.027337619Z","message":"finished row verification on public.orders (shard 1/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"customers","num_truth_rows":1,"num_success":1,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.030056123Z","message":"finished row verification on public.customers (shard 2/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.032311544Z","message":"finished row verification on public.orders (shard 6/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.033806379Z","message":"finished row verification on public.orders (shard 8/8)"}
{"level":"info","service":"verify","type":"summary","table_schema":"public","table_name":"orders","num_truth_rows":0,"num_success":0,"num_conditional_success":0,"num_missing":0,"num_mismatch":0,"num_extraneous":0,"num_live_retry":0,"num_column_mismatch":0,"timestamp":"2026-04-21T07:23:26.034183922Z","message":"finished row verification on public.orders (shard 7/8)"}

```

## Verify Service Log: verify-service-wrong-database.log

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/verify-service-wrong-database.log`

```json
{"level":"info","service":"verify","event":"runtime.starting","timestamp":"2026-04-21T07:23:29.498597293Z","message":"verify-service runtime starting"}

```

## Verify Service Log: verify-service-wrong-permission.log

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/verify-service-wrong-permission.log`

```json
{"level":"info","service":"verify","event":"runtime.starting","timestamp":"2026-04-21T07:23:32.801685777Z","message":"verify-service runtime starting"}

```

## Verify Service Log: verify-service-wrong-cert.log

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/verify-service-wrong-cert.log`

```json
{"level":"info","service":"verify","event":"runtime.starting","timestamp":"2026-04-21T07:23:36.102884841Z","message":"verify-service runtime starting"}

```

## Verify Service Log: verify-service-raw-disabled.log

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/logs/verify-service-raw-disabled.log`

```json
{"level":"info","service":"verify","event":"runtime.starting","timestamp":"2026-04-21T07:23:40.430379737Z","message":"verify-service runtime starting"}

```

## Successful Command Transcript

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/transcript-successful-http-run.txt`

```bash
+ IMAGE=cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest
+ PG_CONTAINER=verify-report-pg
+ VERIFY_CONTAINER=verify-report-service
+ PG_PORT=16432
+ CRDB_PORT=26262
+ VERIFY_PORT=18081
+ docker rm -f verify-report-service verify-report-pg
Error response from daemon: No such container: verify-report-service
verify-report-pg
+ docker run -d --name verify-report-pg -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=postgres -p 16432:5432 postgres:16
1f513d5db228b338191e63487ae9865c76199571937bbbedddfc379995d6c878
++ seq 1 60
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ break
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
 ?column? 
----------
        1
(1 row)

+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
 ?column? 
----------
        1
(1 row)

+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/source_schema.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/source_schema.sql:1: NOTICE:  database "verify_report" does not exist, skipping
DROP DATABASE
CREATE DATABASE
You are now connected to database "verify_report" as user "postgres".
CREATE TABLE
CREATE TABLE
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/source_data.sql
INSERT 0 2
INSERT 0 2
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c 'DROP ROLE IF EXISTS verify_no_select; CREATE ROLE verify_no_select LOGIN; GRANT CONNECT ON DATABASE verify_report TO verify_no_select; GRANT USAGE ON SCHEMA public TO verify_no_select;'
NOTICE:  role "verify_no_select" does not exist, skipping
DROP ROLE
CREATE ROLE
GRANT
GRANT
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql:1: NOTICE:  waiting for job(s) to complete: 1168788012694142977
If the statement is canceled, jobs will continue in the background.
DROP DATABASE
CREATE DATABASE
SET
CREATE TABLE
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql:14: NOTICE:  waiting for job(s) to complete: 1168788013093683201, 1168788013093715969
If the statement is canceled, jobs will continue in the background.
CREATE TABLE
+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_equal.sql
INSERT 0 2
INSERT 0 2
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c '\d+ public.customers'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/logs/source-schema-customers.txt
                                           Table "public.customers"
    Column    |  Type   | Collation | Nullable | Default | Storage  | Compression | Stats target | Description 
--------------+---------+-----------+----------+---------+----------+-------------+--------------+-------------
 customer_id  | integer |           | not null |         | plain    |             |              | 
 email        | text    |           | not null |         | extended |             |              | 
 display_name | text    |           | not null |         | extended |             |              | 
Indexes:
    "customers_pkey" PRIMARY KEY, btree (customer_id)
    "customers_email_key" UNIQUE CONSTRAINT, btree (email)
Referenced by:
    TABLE "orders" CONSTRAINT "orders_customer_id_fkey" FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
Access method: heap

+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c '\d+ public.orders'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/logs/source-schema-orders.txt
                                            Table "public.orders"
   Column    |  Type   | Collation | Nullable | Default | Storage  | Compression | Stats target | Description 
-------------+---------+-----------+----------+---------+----------+-------------+--------------+-------------
 order_id    | integer |           | not null |         | plain    |             |              | 
 customer_id | integer |           | not null |         | plain    |             |              | 
 order_code  | text    |           | not null |         | extended |             |              | 
 total_cents | integer |           | not null |         | plain    |             |              | 
Indexes:
    "orders_pkey" PRIMARY KEY, btree (order_id)
    "orders_order_code_key" UNIQUE CONSTRAINT, btree (order_code)
Foreign-key constraints:
    "orders_customer_id_fkey" FOREIGN KEY (customer_id) REFERENCES customers(customer_id)
Access method: heap

+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SHOW CREATE TABLE public.customers;'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/logs/destination-schema-customers.txt
    table_name    |                         create_statement                         
------------------+------------------------------------------------------------------
 public.customers | CREATE TABLE public.customers (                                 +
                  |         customer_id INT8 NOT NULL,                              +
                  |         email STRING NOT NULL,                                  +
                  |         display_name STRING NOT NULL,                           +
                  |         CONSTRAINT customers_pkey PRIMARY KEY (customer_id ASC),+
                  |         UNIQUE INDEX customers_email_key (email ASC)            +
                  | ) WITH (schema_locked = true);
(1 row)

+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SHOW CREATE TABLE public.orders;'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/logs/destination-schema-orders.txt
  table_name   |                                                create_statement                                                
---------------+----------------------------------------------------------------------------------------------------------------
 public.orders | CREATE TABLE public.orders (                                                                                  +
               |         order_id INT8 NOT NULL,                                                                               +
               |         customer_id INT8 NOT NULL,                                                                            +
               |         order_code STRING NOT NULL,                                                                           +
               |         total_cents INT8 NOT NULL,                                                                            +
               |         CONSTRAINT orders_pkey PRIMARY KEY (order_id ASC),                                                    +
               |         CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES public.customers(customer_id),+
               |         UNIQUE INDEX orders_order_code_key (order_code ASC)                                                   +
               | ) WITH (schema_locked = true);
(1 row)

+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c 'TABLE public.customers; TABLE public.orders;'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/logs/source-data.txt
 customer_id |       email        | display_name 
-------------+--------------------+--------------
        1001 | ada@example.test   | Ada Lovelace
        1002 | grace@example.test | Grace Hopper
(2 rows)

 order_id | customer_id |  order_code   | total_cents 
----------+-------------+---------------+-------------
     5001 |        1001 | ORD-ADA-001   |       12500
     5002 |        1002 | ORD-GRACE-001 |       19999
(2 rows)

+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c 'TABLE public.customers; TABLE public.orders;'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/logs/destination-data-equal.txt
 customer_id |       email        | display_name 
-------------+--------------------+--------------
        1001 | ada@example.test   | Ada Lovelace
        1002 | grace@example.test | Grace Hopper
(2 rows)

 order_id | customer_id |  order_code   | total_cents 
----------+-------------+---------------+-------------
     5001 |        1001 | ORD-ADA-001   |       12500
     5002 |        1002 | ORD-GRACE-001 |       19999
(2 rows)

+ docker run -d --name verify-report-service --network host -v /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/reports/verify-http-real-example-2026-04-21/config:/config:ro cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest --log-format json --config /config/verify-http-raw.yml
8deb0809191817d2d75967870efe95c21b3a2b9bae915c1f4e440346141dcaa0
++ seq 1 60
+ for i in $(seq 1 60)
+ curl -sS http://127.0.0.1:18081/metrics
+ sleep 1
+ for i in $(seq 1 60)
+ curl -sS http://127.0.0.1:18081/metrics
+ break
+ curl -i -sS http://127.0.0.1:18081/metrics
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/initial-metrics.txt
HTTP/1.1 200 OK
Content-Type: text/plain; version=0.0.4; charset=utf-8
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 602

# HELP cockroach_migration_tool_verify_active_jobs Current number of active verify jobs.
# TYPE cockroach_migration_tool_verify_active_jobs gauge
cockroach_migration_tool_verify_active_jobs 0
# HELP cockroach_migration_tool_verify_jobs_total Current number of verify jobs by lifecycle status.
# TYPE cockroach_migration_tool_verify_jobs_total gauge
cockroach_migration_tool_verify_jobs_total{status="failed"} 0
cockroach_migration_tool_verify_jobs_total{status="running"} 0
cockroach_migration_tool_verify_jobs_total{status="stopped"} 0
cockroach_migration_tool_verify_jobs_total{status="succeeded"} 0
+ curl -i -sS -X POST http://127.0.0.1:18081/tables/raw -H 'Content-Type: application/json' --data '{"database":"source","schema":"public","table":"customers"}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/raw-source-customers.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 276

{"database":"source","schema":"public","table":"customers","columns":["customer_id","email","display_name"],"rows":[{"customer_id":1001,"display_name":"Ada Lovelace","email":"ada@example.test"},{"customer_id":1002,"display_name":"Grace Hopper","email":"grace@example.test"}]}
+ curl -i -sS -X POST http://127.0.0.1:18081/tables/raw -H 'Content-Type: application/json' --data '{"database":"destination","schema":"public","table":"orders"}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/raw-destination-orders.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 305

{"database":"destination","schema":"public","table":"orders","columns":["order_id","customer_id","order_code","total_cents"],"rows":[{"customer_id":1001,"order_code":"ORD-ADA-001","order_id":5001,"total_cents":12500},{"customer_id":1002,"order_code":"ORD-GRACE-001","order_id":5002,"total_cents":19999}]}
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/equal-post-job.txt
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:16 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}
+ sleep 3
+ curl -i -sS http://127.0.0.1:18081/jobs/job-000001
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/equal-get-job.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:19 GMT
Content-Length: 584

{"job_id":"job-000001","status":"succeeded","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":false},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0}],"findings":[],"mismatch_summary":{"has_mismatches":false,"affected_tables":[],"counts_by_kind":{}}}}
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql:1: NOTICE:  waiting for job(s) to complete: 1168788027638251521
If the statement is canceled, jobs will continue in the background.
DROP DATABASE
CREATE DATABASE
SET
CREATE TABLE
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql:14: NOTICE:  waiting for job(s) to complete: 1168788028053815297, 1168788028053848065
If the statement is canceled, jobs will continue in the background.
CREATE TABLE
+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_mismatch.sql
INSERT 0 2
INSERT 0 3
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/data-mismatch-post-job.txt
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:19 GMT
Content-Length: 43

{"job_id":"job-000002","status":"running"}
+ sleep 3
+ curl -i -sS http://127.0.0.1:18081/jobs/job-000002
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/data-mismatch-get-job.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:22 GMT
Content-Length: 1485

{"job_id":"job-000002","status":"failed","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":true},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":1,"num_missing":0,"num_mismatch":1,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":1,"num_missing":0,"num_mismatch":1,"num_column_mismatch":0,"num_extraneous":1,"num_live_retry":0}],"findings":[{"kind":"mismatching_row","schema":"public","table":"customers","primary_key":{"customer_id":"1002"},"mismatching_columns":["display_name"],"source_values":{"display_name":"Grace Hopper"},"destination_values":{"display_name":"Rear Admiral Grace Hopper"}},{"kind":"extraneous_row","schema":"public","table":"orders","primary_key":{"order_id":"5003"}},{"kind":"mismatching_row","schema":"public","table":"orders","primary_key":{"order_id":"5002"},"mismatching_columns":["total_cents"],"source_values":{"total_cents":"19999"},"destination_values":{"total_cents":"20999"}}],"mismatch_summary":{"has_mismatches":true,"affected_tables":[{"schema":"public","table":"customers"},{"schema":"public","table":"orders"}],"counts_by_kind":{"extraneous_row":1,"mismatching_row":2}}},"failure":{"category":"mismatch","code":"mismatch_detected","message":"verify detected mismatches in 2 table","details":[{"reason":"mismatch detected for public.customers"},{"reason":"mismatch detected for public.orders"}]}}
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_partial_mismatch.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_partial_mismatch.sql:1: NOTICE:  waiting for job(s) to complete: 1168788038210945025
If the statement is canceled, jobs will continue in the background.
DROP DATABASE
CREATE DATABASE
SET
CREATE TABLE
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_partial_mismatch.sql:14: NOTICE:  waiting for job(s) to complete: 1168788038541869057, 1168788038541901825
If the statement is canceled, jobs will continue in the background.
CREATE TABLE
+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_equal.sql
INSERT 0 2
INSERT 0 2
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/partial-schema-mismatch-post-job.txt
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:22 GMT
Content-Length: 43

{"job_id":"job-000003","status":"running"}
+ sleep 3
+ curl -i -sS http://127.0.0.1:18081/jobs/job-000003
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/partial-schema-mismatch-get-job.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:25 GMT
Content-Length: 955

{"job_id":"job-000003","status":"failed","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":true},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0}],"findings":[{"kind":"mismatching_table_definition","schema":"public","table":"orders","message":"column type mismatch on total_cents: int4 vs text"}],"mismatch_summary":{"has_mismatches":true,"affected_tables":[{"schema":"public","table":"orders"}],"counts_by_kind":{"mismatching_table_definition":1}}},"failure":{"category":"mismatch","code":"mismatch_detected","message":"verify detected mismatches in 1 table","details":[{"reason":"mismatch detected for public.orders"}]}}
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_full_mismatch.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_full_mismatch.sql:1: NOTICE:  waiting for job(s) to complete: 1168788048645750785
If the statement is canceled, jobs will continue in the background.
DROP DATABASE
CREATE DATABASE
SET
CREATE TABLE
CREATE TABLE
+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_equal.sql
INSERT 0 2
INSERT 0 2
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/full-schema-mismatch-post-job.txt
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:25 GMT
Content-Length: 43

{"job_id":"job-000004","status":"running"}
+ sleep 3
+ curl -i -sS http://127.0.0.1:18081/jobs/job-000004
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/full-schema-mismatch-get-job.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 1292

{"job_id":"job-000004","status":"failed","result":{"summary":{"tables_verified":2,"tables_with_data":2,"has_mismatches":true},"table_summaries":[{"schema":"public","table":"customers","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0},{"schema":"public","table":"orders","num_verified":2,"num_success":2,"num_missing":0,"num_mismatch":0,"num_column_mismatch":0,"num_extraneous":0,"num_live_retry":0}],"findings":[{"kind":"mismatching_table_definition","schema":"public","table":"customers","message":"extraneous column loyalty_tier found"},{"kind":"mismatching_table_definition","schema":"public","table":"orders","message":"column type mismatch on total_cents: int4 vs text"},{"kind":"mismatching_table_definition","schema":"public","table":"orders","message":"extraneous column status found"}],"mismatch_summary":{"has_mismatches":true,"affected_tables":[{"schema":"public","table":"customers"},{"schema":"public","table":"orders"}],"counts_by_kind":{"mismatching_table_definition":3}}},"failure":{"category":"mismatch","code":"mismatch_detected","message":"verify detected mismatches in 2 table","details":[{"reason":"mismatch detected for public.customers"},{"reason":"mismatch detected for public.orders"}]}}
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{"include_schema":"["}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/bad-regex-post-job.txt
HTTP/1.1 400 Bad Request
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 201

{"error":{"category":"request_validation","code":"invalid_filter","message":"request validation failed","details":[{"field":"include_schema","reason":"error parsing regexp: missing closing ]: `[`"}]}}
+ curl -i -sS http://127.0.0.1:18081/jobs/no-such-job
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/not-found-get-job.txt
HTTP/1.1 404 Not Found
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 84

{"error":{"category":"job_state","code":"job_not_found","message":"job not found"}}
+ curl -i -sS -X POST http://127.0.0.1:18081/tables/raw -H 'Content-Type: application/json' --data '{"database":"source","schema":"public;drop","table":"customers"}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/raw-invalid-identifier.txt
HTTP/1.1 400 Bad Request
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 51

{"error":"schema must be a simple SQL identifier"}
+ curl -i -sS http://127.0.0.1:18081/metrics
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/metrics.txt
HTTP/1.1 200 OK
Content-Type: text/plain; version=0.0.4; charset=utf-8
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 602

# HELP cockroach_migration_tool_verify_active_jobs Current number of active verify jobs.
# TYPE cockroach_migration_tool_verify_active_jobs gauge
cockroach_migration_tool_verify_active_jobs 0
# HELP cockroach_migration_tool_verify_jobs_total Current number of verify jobs by lifecycle status.
# TYPE cockroach_migration_tool_verify_jobs_total gauge
cockroach_migration_tool_verify_jobs_total{status="failed"} 1
cockroach_migration_tool_verify_jobs_total{status="running"} 0
cockroach_migration_tool_verify_jobs_total{status="stopped"} 0
cockroach_migration_tool_verify_jobs_total{status="succeeded"} 0
+ docker logs verify-report-service
+ docker rm -f verify-report-service
verify-report-service
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql:1: NOTICE:  waiting for job(s) to complete: 1168788059590885377
If the statement is canceled, jobs will continue in the background.
DROP DATABASE
CREATE DATABASE
SET
CREATE TABLE
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql:14: NOTICE:  waiting for job(s) to complete: 1168788059920957441, 1168788059920990209
If the statement is canceled, jobs will continue in the background.
CREATE TABLE
+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_equal.sql
INSERT 0 2
INSERT 0 2
+ docker run -d --name verify-report-service --network host -v /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/reports/verify-http-real-example-2026-04-21/config:/config:ro cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest --log-format json --config /config/verify-http-raw-wrong-database.yml
30e7311e7169dfbc5b696c04aa1aa41e648b8c088fdb1772aa35bedf82994e95
++ seq 1 60
+ for i in $(seq 1 60)
+ curl -sS http://127.0.0.1:18081/metrics
+ break
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/wrong-database-post-job.txt
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:29 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}
+ sleep 3
+ curl -i -sS http://127.0.0.1:18081/jobs/job-000001
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/wrong-database-get-job.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:32 GMT
Content-Length: 601

{"job_id":"job-000001","status":"failed","failure":{"category":"source_access","code":"connection_failed","message":"source connection failed: error connect: failed to connect to `user=postgres database=database_that_does_not_exist`: 127.0.0.1:16432 (localhost): server error: FATAL: database \"database_that_does_not_exist\" does not exist (SQLSTATE 3D000)","details":[{"reason":"error connect: failed to connect to `user=postgres database=database_that_does_not_exist`: 127.0.0.1:16432 (localhost): server error: FATAL: database \"database_that_does_not_exist\" does not exist (SQLSTATE 3D000)"}]}}
+ docker logs verify-report-service
+ docker rm -f verify-report-service
verify-report-service
+ docker run -d --name verify-report-service --network host -v /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/reports/verify-http-real-example-2026-04-21/config:/config:ro cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest --log-format json --config /config/verify-http-raw-wrong-permission.yml
bede45f834a55ae9a0841a3c1e84272a0c5125b2e6bb83a12652cd2e65229be6
++ seq 1 60
+ for i in $(seq 1 60)
+ curl -sS http://127.0.0.1:18081/metrics
+ break
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/wrong-permission-post-job.txt
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:32 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}
+ sleep 3
+ curl -i -sS http://127.0.0.1:18081/jobs/job-000001
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/wrong-permission-get-job.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:35 GMT
Content-Length: 421

{"job_id":"job-000001","status":"failed","failure":{"category":"verify_execution","code":"verify_failed","message":"verify execution failed: error splitting tables: cannot get minimum of public.customers: ERROR: permission denied for table customers (SQLSTATE 42501)","details":[{"reason":"error splitting tables: cannot get minimum of public.customers: ERROR: permission denied for table customers (SQLSTATE 42501)"}]}}
+ docker logs verify-report-service
+ docker rm -f verify-report-service
verify-report-service
+ docker run -d --name verify-report-service --network host -v /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/reports/verify-http-real-example-2026-04-21/config:/config:ro cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest --log-format json --config /config/verify-http-raw-wrong-cert.yml
2e52b20e506f72ea244131e932363d5dbade63349b4df4b422136faafa922ec1
++ seq 1 60
+ for i in $(seq 1 60)
+ curl -sS http://127.0.0.1:18081/metrics
+ sleep 1
+ for i in $(seq 1 60)
+ curl -sS http://127.0.0.1:18081/metrics
+ break
+ curl -i -sS -X POST http://127.0.0.1:18081/jobs -H 'Content-Type: application/json' --data '{}'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/wrong-cert-post-job.txt
HTTP/1.1 202 Accepted
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:37 GMT
Content-Length: 43

{"job_id":"job-000001","status":"running"}
+ sleep 3
+ curl -i -sS http://127.0.0.1:18081/jobs/job-000001
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/wrong-cert-get-job.txt
HTTP/1.1 200 OK
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:40 GMT
Content-Length: 685

{"job_id":"job-000001","status":"failed","failure":{"category":"source_access","code":"connection_failed","message":"source connection failed: cannot parse `postgresql://postgres@localhost:16432/verify_report?sslmode=verify-full\u0026sslrootcert=%2Fconfig%2Fcerts%2Fnot-a-real-ca.crt`: failed to configure TLS (unable to read CA file: open /config/certs/not-a-real-ca.crt: no such file or directory)","details":[{"reason":"cannot parse `postgresql://postgres@localhost:16432/verify_report?sslmode=verify-full\u0026sslrootcert=%2Fconfig%2Fcerts%2Fnot-a-real-ca.crt`: failed to configure TLS (unable to read CA file: open /config/certs/not-a-real-ca.crt: no such file or directory)"}]}}
+ docker logs verify-report-service
+ docker rm -f verify-report-service
verify-report-service
+ docker run -d --name verify-report-service --network host -v /home/joshazimullah.linux/work_mounts/patroni_rewrite/cockroach_migrate_tool/.ralph/reports/verify-http-real-example-2026-04-21/config:/config:ro cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest --log-format json --config /config/verify-http-raw-disabled.yml
aff39d092b6a77e69b97c40853ec6f608a85a1214c2d2ac6e5fd80b162f43e25
++ seq 1 60
+ for i in $(seq 1 60)
+ curl -sS http://127.0.0.1:18081/metrics
+ break
+ tee .ralph/reports/verify-http-real-example-2026-04-21/http/raw-disabled.txt
+ curl -i -sS -X POST http://127.0.0.1:18081/tables/raw -H 'Content-Type: application/json' --data '{"database":"source","schema":"public","table":"customers"}'
HTTP/1.1 403 Forbidden
Content-Type: application/json
Date: Tue, 21 Apr 2026 07:23:40 GMT
Content-Length: 41

{"error":"raw table output is disabled"}
+ docker logs verify-report-service
+ docker rm -f verify-report-service
verify-report-service
+ docker rm -f verify-report-pg
verify-report-pg
+ set +x
DONE report artifacts in .ralph/reports/verify-http-real-example-2026-04-21

```

## Failed Setup Transcript: first attempt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/transcript.txt`

```bash
+ IMAGE=cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest
+ PG_CONTAINER=verify-report-pg
+ CRDB_CONTAINER=verify-report-crdb
+ VERIFY_CONTAINER=verify-report-service
+ PG_PORT=16432
+ CRDB_PORT=26262
+ VERIFY_PORT=18081
+ docker rm -f verify-report-service verify-report-pg verify-report-crdb
Error response from daemon: No such container: verify-report-service
Error response from daemon: No such container: verify-report-pg
Error response from daemon: No such container: verify-report-crdb
+ docker run -d --name verify-report-pg -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=postgres -p 16432:5432 postgres:16
77f5b3640af0f220ffbe20a57dc64811eba8524435c23ffe9deb7631e584dd15
+ docker run -d --name verify-report-crdb -p 26262:26257 cockroachdb/cockroach:v26.1.2 start-single-node --insecure --listen-addr=0.0.0.0:26257 --http-addr=0.0.0.0:8080
b994923a615c1bd2671d4b7f555016faee93e30d12a7b1811188169e1f8e8682
++ seq 1 60
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ break
++ seq 1 60
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/source_schema.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/source_schema.sql:1: NOTICE:  database "verify_report" does not exist, skipping
DROP DATABASE
CREATE DATABASE
You are now connected to database "verify_report" as user "postgres".
CREATE TABLE
CREATE TABLE
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/source_data.sql
INSERT 0 2
INSERT 0 2
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c 'DROP ROLE IF EXISTS verify_no_select; CREATE ROLE verify_no_select LOGIN; GRANT CONNECT ON DATABASE verify_report TO verify_no_select; GRANT USAGE ON SCHEMA public TO verify_no_select;'
NOTICE:  role "verify_no_select" does not exist, skipping
DROP ROLE
CREATE ROLE
GRANT
GRANT
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql
psql: error: connection to server at "localhost" (::1), port 26262 failed: Connection refused
	Is the server running on that host and accepting TCP/IP connections?
connection to server at "localhost" (127.0.0.1), port 26262 failed: Connection refused
	Is the server running on that host and accepting TCP/IP connections?

```

## Failed Setup Transcript: second attempt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/transcript-rerun.txt`

```bash
+ IMAGE=cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest
+ PG_CONTAINER=verify-report-pg
+ VERIFY_CONTAINER=verify-report-service
+ PG_PORT=16432
+ CRDB_PORT=26260
+ VERIFY_PORT=18081
+ docker rm -f verify-report-service verify-report-pg verify-report-crdb
Error response from daemon: No such container: verify-report-service
Error response from daemon: No such container: verify-report-pg
verify-report-crdb
+ docker run -d --name verify-report-pg -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=postgres -p 16432:5432 postgres:16
28d0e95e757c0128f0914ad977999c3c47913be32f1d62bee341c96ea3d19681
++ seq 1 60
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ break
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
 ?column? 
----------
        1
(1 row)

+ psql 'postgresql://root@localhost:26260/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
psql: error: connection to server at "localhost" (::1), port 26260 failed: Connection refused
	Is the server running on that host and accepting TCP/IP connections?
connection to server at "localhost" (127.0.0.1), port 26260 failed: Connection refused
	Is the server running on that host and accepting TCP/IP connections?

```

## Failed Setup Transcript: psql escaping attempt

Source file: `.ralph/reports/verify-http-real-example-2026-04-21/transcript-final-run.txt`

```bash
+ IMAGE=cockroach-migrate-verify-novice-3592830-1776754579763806893-0:latest
+ PG_CONTAINER=verify-report-pg
+ VERIFY_CONTAINER=verify-report-service
+ PG_PORT=16432
+ CRDB_PORT=26262
+ VERIFY_PORT=18081
+ docker rm -f verify-report-service verify-report-pg
Error response from daemon: No such container: verify-report-service
verify-report-pg
+ docker run -d --name verify-report-pg -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_DB=postgres -p 16432:5432 postgres:16
992b1675f378393ac03823c16e818337aba1c4ea2ee17c8cbc3a6f5432687289
++ seq 1 60
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ sleep 1
+ for i in $(seq 1 60)
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
+ break
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
 ?column? 
----------
        1
(1 row)

+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -c 'SELECT 1;'
 ?column? 
----------
        1
(1 row)

+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ cat
+ psql 'postgresql://postgres@localhost:16432/postgres?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/source_schema.sql
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/source_schema.sql:1: NOTICE:  database "verify_report" does not exist, skipping
DROP DATABASE
CREATE DATABASE
You are now connected to database "verify_report" as user "postgres".
CREATE TABLE
CREATE TABLE
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/source_data.sql
INSERT 0 2
INSERT 0 2
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c 'DROP ROLE IF EXISTS verify_no_select; CREATE ROLE verify_no_select LOGIN; GRANT CONNECT ON DATABASE verify_report TO verify_no_select; GRANT USAGE ON SCHEMA public TO verify_no_select;'
NOTICE:  role "verify_no_select" does not exist, skipping
DROP ROLE
CREATE ROLE
GRANT
GRANT
+ psql 'postgresql://root@localhost:26262/defaultdb?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql
DROP DATABASE
CREATE DATABASE
SET
CREATE TABLE
psql:.ralph/reports/verify-http-real-example-2026-04-21/sql/destination_schema_equal.sql:14: NOTICE:  waiting for job(s) to complete: 1168787774564859905, 1168787774564892673
If the statement is canceled, jobs will continue in the background.
CREATE TABLE
+ psql 'postgresql://root@localhost:26262/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -f .ralph/reports/verify-http-real-example-2026-04-21/sql/destination_data_equal.sql
INSERT 0 2
INSERT 0 2
+ psql 'postgresql://postgres@localhost:16432/verify_report?sslmode=disable' -v ON_ERROR_STOP=1 -c '\\d+ public.customers'
+ tee .ralph/reports/verify-http-real-example-2026-04-21/logs/source-schema-customers.txt
invalid command \

```
