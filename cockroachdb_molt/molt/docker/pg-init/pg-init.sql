CREATE DATABASE molt;
\c molt;

CREATE TABLE employees (
    id serial PRIMARY KEY,
    unique_id UUID,
    name VARCHAR(50),
    created_at TIMESTAMPTZ,
    updated_at DATE,
    is_hired BOOLEAN,
    age SMALLINT,
    salary NUMERIC(8, 2),
    bonus REAL
);

CREATE TABLE tbl1(id INT PRIMARY KEY, t TEXT);

CREATE TABLE departments (
    id serial PRIMARY KEY,
    name VARCHAR(50),
    description VARCHAR(100),
    number_employees INTEGER
);

CREATE TABLE customers (
    id serial PRIMARY KEY,
    unique_id VARCHAR(100),
    name VARCHAR(50),
    priority VARCHAR(20)
);

CREATE TABLE contractors (
    id SERIAL PRIMARY KEY,
    unique_id VARCHAR(100),
    location VARCHAR(50),
    hourly_rate DECIMAL(8, 2)
);

DO $$ 
DECLARE 
    i INT;
BEGIN
    i := 1;
    WHILE i <= 200000 LOOP
        INSERT INTO employees (unique_id, name, created_at, updated_at, is_hired, age, salary, bonus)
        VALUES (
            ('550e8400-e29b-41d4-a716-446655440000'::uuid),
            'Employee_' || i,
            '2023-11-03 09:00:00'::timestamp,
            '2023-11-03'::date,
            true,
            24,
            5000.00,
            100.25
        );
        i := i + 1;
    END LOOP;
END $$;

INSERT INTO tbl1 VALUES (1, 'aaa'), (2, 'bb b'), (3, 'ééé'), (4, '🫡🫡🫡'), (5, '娜娜'), (6, 'Лукас'), (7, 'ルカス');

INSERT INTO departments(name, description, number_employees) VALUES ('engineering', 'building tech', 400), ('sales', 'building funnel', 200);

INSERT INTO customers (unique_id, name, priority) 
VALUES 
('ABC123', 'John Doe', 'High'),
('DEF456', 'Jane Smith', 'Medium'),
('GHI789', 'Alice Johnson', 'Low');

INSERT INTO contractors (unique_id, location, hourly_rate) 
VALUES 
('CON123', 'New York', 50.00),
('CON456', 'Los Angeles', 45.50),
('CON789', 'Chicago', 55.75);
