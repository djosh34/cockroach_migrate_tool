CREATE DATABASE molt; 
USE molt;

CREATE TABLE employees (
	id SERIAL4 PRIMARY KEY,
	unique_id UUID,
	name VARCHAR(50),
	created_at TIMESTAMPTZ,
	updated_at DATE,
	is_hired BOOL,
	age INT2,
	salary DECIMAL(8,2),
	bonus FLOAT4
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
