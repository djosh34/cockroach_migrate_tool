DROP DATABASE IF EXISTS verify_demo;
CREATE DATABASE verify_demo;
\connect verify_demo

CREATE TABLE customers (
    id BIGINT PRIMARY KEY,
    email VARCHAR(255) NOT NULL UNIQUE,
    region VARCHAR(32) NOT NULL,
    status VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX customers_region_status_idx ON customers (region, status);

CREATE TABLE products (
    id BIGINT PRIMARY KEY,
    sku VARCHAR(64) NOT NULL UNIQUE,
    category VARCHAR(64) NOT NULL,
    price_cents BIGINT NOT NULL,
    active BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX products_category_active_idx ON products (category, active);

CREATE TABLE orders (
    id BIGINT PRIMARY KEY,
    customer_id BIGINT NOT NULL REFERENCES customers (id),
    order_number VARCHAR(64) NOT NULL UNIQUE,
    status VARCHAR(32) NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    paid_at TIMESTAMPTZ NULL,
    total_cents BIGINT NOT NULL,
    shipping_country VARCHAR(8) NOT NULL
);

CREATE INDEX orders_customer_created_idx ON orders (customer_id, created_at DESC);
CREATE INDEX orders_status_idx ON orders (status);

CREATE TABLE order_items (
    order_id BIGINT NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    line_no INT NOT NULL,
    product_id BIGINT NOT NULL REFERENCES products (id),
    quantity INT NOT NULL,
    unit_price_cents BIGINT NOT NULL,
    discount_cents BIGINT NOT NULL,
    PRIMARY KEY (order_id, line_no)
);

CREATE INDEX order_items_product_idx ON order_items (product_id);
