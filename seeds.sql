-- Seed data for testing the TUI
-- Run with: psql -h localhost -U finances -d finances -f seeds.sql
-- Or: docker exec -i finances-db psql -U finances -d finances < seeds.sql

-- Accounts (the default 'Cash' account is id=1 from the migration)
INSERT INTO accounts (name, account_type, has_credit_card, credit_limit, billing_day, due_day, has_debit_card)
VALUES
    ('Nubank', 'checking', TRUE, 5000.00, 3, 10, TRUE),
    ('PicPay', 'checking', FALSE, NULL, NULL, NULL, TRUE),
    ('Inter', 'checking', TRUE, 3000.00, 5, 15, TRUE);

-- Transactions (using default categories from migration: Food=2, Transport=3, Health=4, Entertainment=6, Subscriptions=9, Salary=16)
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    -- Salary
    (4500.00, 'March salary',           16, 2, 'income',  'transfer', '2026-03-01'),
    -- Food expenses
    (45.90,   'Supermarket weekly',       2, 2, 'expense', 'debit',    '2026-03-01'),
    (32.50,   'Restaurant lunch',         2, 2, 'expense', 'credit',   '2026-03-02'),
    (18.00,   'Bakery',                   2, 1, 'expense', 'cash',     '2026-03-03'),
    (120.00,  'Big supermarket run',      2, 2, 'expense', 'pix',      '2026-03-04'),
    (27.90,   'iFood delivery',           2, 2, 'expense', 'credit',   '2026-03-05'),
    -- Transport
    (150.00,  'Gas station',              3, 4, 'expense', 'debit',    '2026-03-02'),
    (6.50,    'Bus ticket',               3, 1, 'expense', 'cash',     '2026-03-03'),
    -- Health
    (85.00,   'Pharmacy',                 4, 2, 'expense', 'pix',      '2026-03-04'),
    -- Entertainment
    (65.00,   'Cinema + popcorn',         6, 2, 'expense', 'credit',   '2026-03-01'),
    -- Subscriptions
    (55.90,   'Spotify + Netflix',        9, 2, 'expense', 'credit',   '2026-03-05'),
    -- Freelance income
    (800.00,  'Freelance project',       17, 3, 'income',  'pix',      '2026-03-03');

-- Transfers
INSERT INTO transfers (from_account_id, to_account_id, amount, description, date) VALUES
    (2, 3, 200.00, 'Top up PicPay',   '2026-03-01'),
    (2, 1, 100.00, 'Cash withdrawal', '2026-03-02');

-- Credit card payment (Nubank)
INSERT INTO credit_card_payments (account_id, amount, date, description) VALUES
    (2, 500.00, '2026-03-05', 'February card bill');

-- Budgets
INSERT INTO budgets (category_id, amount, period) VALUES
    (2, 500.00,  'monthly'),   -- Food: R$500/month
    (3, 300.00,  'monthly'),   -- Transport: R$300/month
    (6, 200.00,  'monthly'),   -- Entertainment: R$200/month
    (9, 100.00,  'monthly');   -- Subscriptions: R$100/month

-- Recurring transactions
INSERT INTO recurring_transactions (amount, description, category_id, account_id, transaction_type, payment_method, frequency, next_due) VALUES
    (55.90,   'Spotify + Netflix',    9, 2, 'expense', 'credit', 'monthly', '2026-04-05'),
    (150.00,  'Electricity bill',     8, 2, 'expense', 'boleto', 'monthly', '2026-04-10'),
    (89.90,   'Internet',             8, 2, 'expense', 'boleto', 'monthly', '2026-04-15');

-- Installment purchase (Nubank credit card, 3x)
INSERT INTO installment_purchases (total_amount, installment_count, description, category_id, account_id, first_installment_date)
VALUES (900.00, 3, 'New headphones', 6, 2, '2026-03-01');

INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date, installment_purchase_id, installment_number)
VALUES
    (300.00, 'New headphones (1/3)', 6, 2, 'expense', 'credit', '2026-03-01', 1, 1),
    (300.00, 'New headphones (2/3)', 6, 2, 'expense', 'credit', '2026-04-01', 1, 2),
    (300.00, 'New headphones (3/3)', 6, 2, 'expense', 'credit', '2026-05-01', 1, 3);
