DROP DATABASE IF EXISTS demo_cdc CASCADE;
CREATE DATABASE demo_cdc;
USE demo_cdc;

CREATE TABLE customers (
    id INT PRIMARY KEY,
    email STRING NOT NULL UNIQUE,
    region STRING NOT NULL,
    status STRING NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX customers_region_status_idx ON customers (region, status);

CREATE TABLE products (
    id INT PRIMARY KEY,
    sku STRING NOT NULL UNIQUE,
    category STRING NOT NULL,
    price_cents INT NOT NULL,
    active BOOL NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX products_category_active_idx ON products (category, active);

CREATE TABLE orders (
    id INT PRIMARY KEY,
    customer_id INT NOT NULL REFERENCES customers (id),
    order_number STRING NOT NULL UNIQUE,
    status STRING NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    paid_at TIMESTAMPTZ NULL,
    total_cents INT NOT NULL,
    shipping_country STRING NOT NULL
);

CREATE INDEX orders_customer_created_idx ON orders (customer_id, created_at DESC);
CREATE INDEX orders_status_idx ON orders (status);

CREATE TABLE order_items (
    order_id INT NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    line_no INT NOT NULL,
    product_id INT NOT NULL REFERENCES products (id),
    quantity INT NOT NULL,
    unit_price_cents INT NOT NULL,
    discount_cents INT NOT NULL,
    PRIMARY KEY (order_id, line_no)
);

CREATE INDEX order_items_product_idx ON order_items (product_id);
