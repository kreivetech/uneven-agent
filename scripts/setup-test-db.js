import Database from 'better-sqlite3';
import fs from 'fs';
import path from 'path';

const dir = './test-project';
if (!fs.existsSync(dir)){
    fs.mkdirSync(dir);
}

const dbPath = path.join(dir, 'test.db');
if (fs.existsSync(dbPath)) {
    fs.unlinkSync(dbPath);
}

const db = new Database(dbPath);

db.exec(`
  CREATE TABLE accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT,
    balance REAL
  );

  CREATE TABLE transactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    account_id INTEGER,
    amount REAL,
    category TEXT,
    description TEXT,
    date TEXT,
    FOREIGN KEY (account_id) REFERENCES accounts(id)
  );

  INSERT INTO accounts (name, balance) VALUES ('Checking', 5000.0), ('Savings', 12000.0);

  INSERT INTO transactions (account_id, amount, category, description, date) VALUES 
  (1, 1500.0, 'Salary', 'Monthly pay', '2023-05-01'),
  (1, -50.0, 'Food', 'Lunch', '2023-05-02'),
  (2, -120.0, 'Bills', 'Electricity', '2023-05-03'),
  (1, -30.0, 'Transport', 'Uber', '2023-05-04'),
  (2, 200.0, 'Gift', 'Birthday', '2023-05-05');
`);

console.log('Test database created successfully at ' + dbPath);
db.close();
