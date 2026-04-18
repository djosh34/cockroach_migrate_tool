USE demo_cdc;

UPDATE customers
SET region = 'priority-east'
WHERE id = 4;
