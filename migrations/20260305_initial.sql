CREATE TABLE accounts (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    account_type VARCHAR(10) NOT NULL DEFAULT 'checking'
        CHECK (account_type IN ('checking', 'cash')),
    has_credit_card BOOLEAN NOT NULL DEFAULT FALSE,
    credit_limit NUMERIC(12, 2),
    billing_day SMALLINT CHECK (billing_day BETWEEN 1 AND 28),
    due_day SMALLINT CHECK (due_day BETWEEN 1 AND 28),
    has_debit_card BOOLEAN NOT NULL DEFAULT FALSE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE categories (
    id SERIAL PRIMARY KEY,
    name VARCHAR(100) NOT NULL,
    category_type VARCHAR(7) NOT NULL CHECK (category_type IN ('expense', 'income')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE installment_purchases (
    id SERIAL PRIMARY KEY,
    total_amount NUMERIC(12, 2) NOT NULL CHECK (total_amount > 0),
    installment_count SMALLINT NOT NULL CHECK (installment_count >= 2),
    description VARCHAR(255) NOT NULL,
    category_id INT NOT NULL REFERENCES categories(id),
    account_id INT NOT NULL REFERENCES accounts(id),
    first_installment_date DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE transactions (
    id SERIAL PRIMARY KEY,
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    description VARCHAR(255) NOT NULL,
    category_id INT NOT NULL REFERENCES categories(id),
    account_id INT NOT NULL REFERENCES accounts(id),
    transaction_type VARCHAR(7) NOT NULL
        CHECK (transaction_type IN ('expense', 'income')),
    payment_method VARCHAR(10) NOT NULL
        CHECK (payment_method IN ('pix', 'credit', 'debit', 'cash', 'boleto', 'transfer')),
    date DATE NOT NULL,
    installment_purchase_id INT REFERENCES installment_purchases(id) ON DELETE CASCADE,
    installment_number SMALLINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_transactions_date ON transactions(date);
CREATE INDEX idx_transactions_category ON transactions(category_id);
CREATE INDEX idx_transactions_account ON transactions(account_id);
CREATE INDEX idx_transactions_installment ON transactions(installment_purchase_id)
    WHERE installment_purchase_id IS NOT NULL;

CREATE TABLE transfers (
    id SERIAL PRIMARY KEY,
    from_account_id INT NOT NULL REFERENCES accounts(id),
    to_account_id INT NOT NULL REFERENCES accounts(id),
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    description VARCHAR(255) NOT NULL DEFAULT '',
    date DATE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (from_account_id != to_account_id)
);

CREATE TABLE credit_card_payments (
    id SERIAL PRIMARY KEY,
    account_id INT NOT NULL REFERENCES accounts(id),
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    date DATE NOT NULL,
    description VARCHAR(255) NOT NULL DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE budgets (
    id SERIAL PRIMARY KEY,
    category_id INT NOT NULL REFERENCES categories(id),
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    period VARCHAR(7) NOT NULL CHECK (period IN ('weekly', 'monthly', 'yearly')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(category_id, period)
);

CREATE TABLE recurring_transactions (
    id SERIAL PRIMARY KEY,
    amount NUMERIC(12, 2) NOT NULL CHECK (amount > 0),
    description VARCHAR(255) NOT NULL,
    category_id INT NOT NULL REFERENCES categories(id),
    account_id INT NOT NULL REFERENCES accounts(id),
    transaction_type VARCHAR(7) NOT NULL
        CHECK (transaction_type IN ('expense', 'income')),
    payment_method VARCHAR(10) NOT NULL
        CHECK (payment_method IN ('pix', 'credit', 'debit', 'cash', 'boleto', 'transfer')),
    frequency VARCHAR(7) NOT NULL
        CHECK (frequency IN ('daily', 'weekly', 'monthly', 'yearly')),
    next_due DATE NOT NULL,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE notifications (
    id SERIAL PRIMARY KEY,
    message TEXT NOT NULL,
    notification_type VARCHAR(20) NOT NULL
        CHECK (notification_type IN (
            'no_transactions',
            'overdue_recurring',
            'budget_50',
            'budget_75',
            'budget_90',
            'budget_100',
            'budget_exceeded'
        )),
    reference_id INT,
    read BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_notifications_unread ON notifications (created_at DESC)
    WHERE read = FALSE;

CREATE UNIQUE INDEX idx_notifications_dedup
    ON notifications (notification_type, COALESCE(reference_id, 0))
    WHERE read = FALSE;

INSERT INTO categories (name, category_type) VALUES
    ('Housing', 'expense'),
    ('Food', 'expense'),
    ('Transport', 'expense'),
    ('Health', 'expense'),
    ('Education', 'expense'),
    ('Entertainment', 'expense'),
    ('Clothing', 'expense'),
    ('Utilities', 'expense'),
    ('Subscriptions', 'expense'),
    ('Taxes', 'expense'),
    ('Debt', 'expense'),
    ('Personal Care', 'expense'),
    ('Pets', 'expense'),
    ('Gifts', 'expense'),
    ('Other', 'expense');

INSERT INTO categories (name, category_type) VALUES
    ('Salary', 'income'),
    ('Freelance', 'income'),
    ('Investments', 'income'),
    ('Sales', 'income'),
    ('Gifts', 'income'),
    ('Other', 'income');

INSERT INTO accounts (name, account_type) VALUES
    ('Cash', 'cash');
