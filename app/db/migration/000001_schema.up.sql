BEGIN

CREATE TABLE IF NOT EXISTS category_expenses(
	id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS subcategory_expenses(
	id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	category_id INT REFERENCES category_expenses(id),
	description TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS category_incomes(
	id INT PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
	description TEXT NOT NULL
);

CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS users(
	id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	username TEXT NOT NULL UNIQUE,
	email TEXT NOT NULL UNIQUE,
	password_hash TEXT NOT NULL,
	created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
	updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
	deleted_at TIMESTAMP WITH TIME ZONE
);

CREATE TABLE IF NOT EXISTS expenses(
	id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	user_id UUID REFERENCES users(id),
	subcategory_id REFERENCES subcategory_expenses(id),
	value FLOAT NOT NULL,
	description TEXT
);

CREATE TABLE IF NOT EXISTS incomes(
	id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
	user_id UUID REFERENCES users(id),
	category_id REFERENCES category_incomes(id),
	value FLOAT NOT NULL,
	description TEXT
);

COMMIT;
