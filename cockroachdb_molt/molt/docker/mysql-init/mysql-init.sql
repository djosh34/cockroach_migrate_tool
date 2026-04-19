CREATE DATABASE molt;
use molt;

GRANT ALL PRIVILEGES ON *.* TO 'user'@'%' WITH GRANT OPTION;

CREATE TABLE employees (
    id INT AUTO_INCREMENT PRIMARY KEY,
    unique_id VARCHAR(100),
    name VARCHAR(50),
    created_at DATETIME,
    updated_at DATE,
    is_hired TINYINT(1),
    age TINYINT(2),
    salary DECIMAL(8, 2),
    bonus FLOAT
);

CREATE TABLE departments (
    id INT AUTO_INCREMENT PRIMARY KEY,
    name VARCHAR(50),
    description VARCHAR(100),
    number_employees INT
);

CREATE TABLE customers (
    id INT AUTO_INCREMENT PRIMARY KEY,
    unique_id VARCHAR(100),
    name VARCHAR(50),
    priority VARCHAR(20)
);

CREATE TABLE contractors (
    id INT AUTO_INCREMENT PRIMARY KEY,
    unique_id VARCHAR(100),
    location VARCHAR(50),
    hourly_rate DECIMAL(8, 2)
);

DELIMITER $$
CREATE PROCEDURE InsertEmployeesWithTransaction()
BEGIN
    DECLARE i INT;
    SET i = 1;
    
    START TRANSACTION;

    WHILE i <= 200000 DO
        INSERT INTO employees (unique_id, name, created_at, updated_at, is_hired, age, salary, bonus)
        VALUES (
            '550e8400-e29b-41d4-a716-446655440000',
            CONCAT('Employee_', i),
            '2023-11-03 09:00:00',
            '2023-11-03',
            1,
            24,
            5000.00,
            100.25
        );
        SET i = i + 1;
    END WHILE;

    COMMIT;
END$$
DELIMITER ;

CALL InsertEmployeesWithTransaction();

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

