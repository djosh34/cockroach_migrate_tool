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
