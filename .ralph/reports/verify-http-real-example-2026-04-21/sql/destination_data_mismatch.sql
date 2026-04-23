INSERT INTO public.customers (customer_id, email, display_name) VALUES
  (1001, 'ada@example.test', 'Ada Lovelace'),
  (1002, 'grace@example.test', 'Rear Admiral Grace Hopper');
INSERT INTO public.orders (order_id, customer_id, order_code, total_cents) VALUES
  (5001, 1001, 'ORD-ADA-001', 12500),
  (5002, 1002, 'ORD-GRACE-001', 20999),
  (5003, 1002, 'ORD-GRACE-EXTRA', 777);
