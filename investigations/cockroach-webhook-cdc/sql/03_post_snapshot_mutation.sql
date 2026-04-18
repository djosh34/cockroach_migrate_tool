USE demo_cdc;

UPDATE customers
SET status = 'vip'
WHERE id = 3;
