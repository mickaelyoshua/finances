BEGIN;

CREATE TABLE IF NOT EXISTS category_expenses(
	id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	description TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS subcategory_expenses(
	id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	category_id INT NOT NULL UNIQUE REFERENCES category_expenses(id) ON DELETE CASCADE,
	description TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS category_incomes(
	id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	description TEXT NOT NULL UNIQUE
);

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS users(
	id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	name TEXT NOT NULL CHECK (length(name) >= 3 AND length(name) <= 50),
	email TEXT NOT NULL UNIQUE CHECK (email ~* '^[A-Za-z0-9._+%-]+@[A-Za-z0-9.-]+[.][A-Za-z]+$'),
	password_hash TEXT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
	updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
	deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE IF NOT EXISTS expenses(
	id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	subcategory_id INT NOT NULL REFERENCES subcategory_expenses(id),
	value NUMERIC(10, 2) NOT NULL CHECK (value > 0),
	transaction_date DATE NOT NULL DEFAULT CURRENT_DATE,
	description TEXT,
	created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS incomes(
	id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
	category_id INT NOT NULL REFERENCES category_incomes(id),
	value NUMERIC(10, 2) NOT NULL CHECK (value > 0),
	transaction_date DATE NOT NULL DEFAULT CURRENT_DATE,
	description TEXT,
	created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Add indexes for foreign keys to improve query performance
CREATE INDEX ON expenses (user_id);
CREATE INDEX ON expenses (subcategory_id);
CREATE INDEX ON incomes (user_id);
CREATE INDEX ON incomes (category_id);
CREATE INDEX ON subcategory_expenses (category_id);

COMMIT;
