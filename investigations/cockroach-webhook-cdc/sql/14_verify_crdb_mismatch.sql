USE verify_demo;

UPDATE customers
SET status = 'crdb-only-mismatch'
WHERE id = 2;
