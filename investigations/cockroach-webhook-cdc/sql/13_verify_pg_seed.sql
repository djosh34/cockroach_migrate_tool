\connect verify_demo

INSERT INTO customers (id, email, region, status, created_at)
SELECT
    customer_id,
    'customer-' || customer_id::text || '@example.com',
    CASE customer_id % 4
        WHEN 0 THEN 'north'
        WHEN 1 THEN 'south'
        WHEN 2 THEN 'west'
        ELSE 'east'
    END,
    CASE customer_id % 3
        WHEN 0 THEN 'new'
        WHEN 1 THEN 'active'
        ELSE 'paused'
    END,
    TIMESTAMPTZ '2026-01-01 08:00:00+00' + (customer_id * INTERVAL '6 hour')
FROM generate_series(1, 24) AS customer_id;

INSERT INTO products (id, sku, category, price_cents, active, created_at)
SELECT
    product_id,
    'SKU-' || lpad(product_id::text, 4, '0'),
    CASE product_id % 4
        WHEN 0 THEN 'books'
        WHEN 1 THEN 'hardware'
        WHEN 2 THEN 'office'
        ELSE 'software'
    END,
    900 + product_id * 175,
    product_id % 5 != 0,
    TIMESTAMPTZ '2026-01-02 09:30:00+00' + (product_id * INTERVAL '4 hour')
FROM generate_series(1, 18) AS product_id;

INSERT INTO orders (id, customer_id, order_number, status, created_at, paid_at, total_cents, shipping_country)
SELECT
    order_id,
    ((order_id - 1) % 24) + 1,
    'ORD-' || lpad(order_id::text, 5, '0'),
    CASE order_id % 4
        WHEN 0 THEN 'paid'
        WHEN 1 THEN 'pending'
        WHEN 2 THEN 'shipped'
        ELSE 'cancelled'
    END,
    TIMESTAMPTZ '2026-01-05 10:00:00+00' + (order_id * INTERVAL '2 hour'),
    CASE order_id % 4
        WHEN 0 THEN TIMESTAMPTZ '2026-01-05 11:00:00+00' + (order_id * INTERVAL '2 hour')
        WHEN 2 THEN TIMESTAMPTZ '2026-01-05 12:00:00+00' + (order_id * INTERVAL '2 hour')
        ELSE NULL
    END,
    0,
    CASE order_id % 3
        WHEN 0 THEN 'NL'
        WHEN 1 THEN 'DE'
        ELSE 'US'
    END
FROM generate_series(1, 72) AS order_id;

INSERT INTO order_items (order_id, line_no, product_id, quantity, unit_price_cents, discount_cents)
SELECT
    order_id,
    line_no,
    ((order_id * 5 + line_no) % 18) + 1,
    (line_no % 3) + 1,
    900 + ((((order_id * 5 + line_no) % 18) + 1) * 175),
    CASE
        WHEN (order_id + line_no) % 4 = 0 THEN 150
        ELSE 0
    END
FROM generate_series(1, 72) AS order_id
CROSS JOIN generate_series(1, 3) AS line_no;

UPDATE orders
SET total_cents = item_totals.total_cents
FROM (
    SELECT
        order_id,
        SUM(quantity * unit_price_cents - discount_cents) AS total_cents
    FROM order_items
    GROUP BY order_id
) AS item_totals
WHERE orders.id = item_totals.order_id;
