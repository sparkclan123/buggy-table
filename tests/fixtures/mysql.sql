-- Disposable Compose fixture only. Never run on a production database.
CREATE USER 'tablepor_reader'@'%' IDENTIFIED BY 'local-mysql-reader';
GRANT SELECT, SHOW VIEW ON tablepor_test.* TO 'tablepor_reader'@'%';
CREATE USER 'tablepor_writer'@'%' IDENTIFIED BY 'local-mysql-writer';
GRANT SELECT, INSERT, UPDATE, DELETE ON tablepor_test.* TO 'tablepor_writer'@'%';
USE tablepor_test;
CREATE TABLE items (id BIGINT PRIMARY KEY, label VARCHAR(255) NOT NULL, amount DECIMAL(30,10), note TEXT, created_at DATETIME);
INSERT INTO items VALUES (1, 'First', 12345678901234567890.1234567890, NULL, '2026-10-14 10:00:00'), (2, 'Second', 2.5, 'original', '2026-10-14 11:00:00');
CREATE INDEX items_label_idx ON items(label);
CREATE VIEW items_view AS SELECT * FROM items;
CREATE TABLE binary_items (id INT PRIMARY KEY, payload BLOB);
CREATE TABLE generated_items (id INT PRIMARY KEY, copy INT GENERATED ALWAYS AS (id+1) VIRTUAL);
