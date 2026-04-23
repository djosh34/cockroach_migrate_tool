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
