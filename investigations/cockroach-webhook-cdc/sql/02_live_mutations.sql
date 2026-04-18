USE demo_cdc;

BEGIN;

UPDATE orders
SET status = 'paid',
    paid_at = TIMESTAMPTZ '2026-02-01 11:15:00+00',
    total_cents = total_cents - 125
WHERE id = 5;

UPDATE order_items
SET discount_cents = discount_cents + 125
WHERE order_id = 5 AND line_no = 2;

INSERT INTO customers (id, email, region, status, created_at)
VALUES (1001, 'late-buyer@example.com', 'north', 'active', TIMESTAMPTZ '2026-02-01 11:00:00+00');

INSERT INTO orders (id, customer_id, order_number, status, created_at, paid_at, total_cents, shipping_country)
VALUES (1001, 1001, 'ORD-01001', 'pending', TIMESTAMPTZ '2026-02-01 11:05:00+00', NULL, 5324, 'NL');

INSERT INTO order_items (order_id, line_no, product_id, quantity, unit_price_cents, discount_cents)
VALUES
    (1001, 1, 4, 1, 1600, 0),
    (1001, 2, 9, 2, 1862, 0);

DELETE FROM order_items
WHERE order_id = 7 AND line_no = 3;

COMMIT;
