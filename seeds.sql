-- Seed data for manual testing of all TUI features and notification thresholds.
-- Assumes a freshly migrated DB (Cash account id=1, default categories 1-21).
--
-- Run with:
--   docker exec -i finances-db psql -U finances -d finances < seeds.sql

-- ============================================================
-- Accounts (Cash id=1 from migration)
-- ============================================================
-- id=2: checking + credit card + debit card
-- id=3: checking + debit card only (no credit card)
-- id=4: checking + credit card + debit card
-- id=5: checking + credit card only (no debit card)
INSERT INTO accounts (name, account_type, has_credit_card, credit_limit, billing_day, due_day, has_debit_card)
VALUES
    ('Nubank',   'checking', TRUE,  5000.00, 3,  10, TRUE),
    ('PicPay',   'checking', FALSE, NULL,    NULL, NULL, TRUE),
    ('Inter',    'checking', TRUE,  3000.00, 5,  15, TRUE),
    ('Bradesco', 'checking', TRUE,  2000.00, 10, 20, FALSE);

-- ============================================================
-- Transactions — all dates in March 2026, NONE on 2026-03-17.
-- Designed to hit specific budget thresholds.
-- ============================================================

-- === Income ===
-- Salary (cat 16) — transfer payment method
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (4500.00, 'March salary',       16, 2, 'income', 'transfer', '2026-03-01'),
    (800.00,  'Freelance project',  17, 3, 'income', 'pix',      '2026-03-05'),
    (250.00,  'Weekend gig',        17, 4, 'income', 'pix',      '2026-03-10');

-- === Food (cat 2): budget R$500, target ~R$275 = 55% → Budget50 ===
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (85.00,  'Weekly groceries',      2, 2, 'expense', 'debit',  '2026-03-01'),
    (32.50,  'Restaurant lunch',      2, 2, 'expense', 'credit', '2026-03-03'),
    (18.00,  'Bakery',                2, 1, 'expense', 'cash',   '2026-03-05'),
    (67.00,  'Supermarket run',       2, 2, 'expense', 'pix',    '2026-03-08'),
    (27.90,  'iFood delivery',        2, 2, 'expense', 'credit', '2026-03-10'),
    (44.60,  'Fair produce',          2, 3, 'expense', 'debit',  '2026-03-14');
-- Total: 85 + 32.50 + 18 + 67 + 27.90 + 44.60 = 275.00

-- === Transport (cat 3): budget R$300, target ~R$235 = 78% → Budget75 ===
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (150.00, 'Gas station',           3, 4, 'expense', 'debit',  '2026-03-02'),
    (6.50,   'Bus ticket',            3, 1, 'expense', 'cash',   '2026-03-04'),
    (6.50,   'Bus ticket',            3, 1, 'expense', 'cash',   '2026-03-06'),
    (45.00,  'Uber rides',            3, 2, 'expense', 'pix',    '2026-03-09'),
    (27.00,  'Parking fees',          3, 4, 'expense', 'debit',  '2026-03-12');
-- Total: 150 + 6.50 + 6.50 + 45 + 27 = 235.00

-- === Health (cat 4): budget R$150, target ~R$195 = 130% → BudgetExceeded ===
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (85.00,  'Pharmacy',              4, 2, 'expense', 'pix',    '2026-03-03'),
    (60.00,  'Doctor copay',          4, 2, 'expense', 'debit',  '2026-03-07'),
    (50.00,  'Lab tests',             4, 4, 'expense', 'boleto', '2026-03-11');
-- Total: 85 + 60 + 50 = 195.00

-- === Entertainment (cat 6): budget R$200, target ~R$185 = 92% → Budget90 ===
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (65.00,  'Cinema + popcorn',      6, 2, 'expense', 'credit', '2026-03-01'),
    (45.00,  'Board game cafe',       6, 3, 'expense', 'debit',  '2026-03-06'),
    (75.00,  'Concert ticket',        6, 5, 'expense', 'credit', '2026-03-13');
-- Total: 65 + 45 + 75 = 185.00

-- === Subscriptions (cat 9): budget R$100, target ~R$100 = 100% → Budget100 ===
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (44.90,  'Spotify + Netflix',     9, 2, 'expense', 'credit', '2026-03-05'),
    (55.10,  'YouTube Premium + iCloud', 9, 4, 'expense', 'credit', '2026-03-05');
-- Total: 44.90 + 55.10 = 100.00

-- === Housing (cat 1): budget R$2000, target ~R$800 = 40% → no notification ===
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (800.00, 'Rent',                  1, 2, 'expense', 'boleto', '2026-03-01');
-- Total: 800.00

-- === Clothing (cat 7): budget R$300, spend R$0 → no notification ===
-- (no transactions)

-- === Extra: Utilities (cat 8), no budget — just for variety ===
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    (150.00, 'Electricity bill',      8, 2, 'expense', 'boleto', '2026-03-10'),
    (89.90,  'Internet bill',         8, 2, 'expense', 'boleto', '2026-03-15');

-- ============================================================
-- Budgets — 7 budgets covering all threshold bands
-- ============================================================
INSERT INTO budgets (category_id, amount, period) VALUES
    (2,  500.00,  'monthly'),   -- Food:           55% → Budget50
    (3,  300.00,  'monthly'),   -- Transport:      78% → Budget75
    (4,  150.00,  'monthly'),   -- Health:        130% → BudgetExceeded
    (6,  200.00,  'monthly'),   -- Entertainment:  92% → Budget90
    (9,  100.00,  'monthly'),   -- Subscriptions: 100% → Budget100
    (1, 2000.00,  'monthly'),   -- Housing:        40% → none
    (7,  300.00,  'monthly');   -- Clothing:        0% → none

-- ============================================================
-- Transfers — 3 between different accounts
-- ============================================================
INSERT INTO transfers (from_account_id, to_account_id, amount, description, date) VALUES
    (2, 3, 200.00, 'Top up PicPay',       '2026-03-01'),
    (2, 1, 100.00, 'Cash withdrawal',     '2026-03-04'),
    (4, 5, 150.00, 'Move to Bradesco',    '2026-03-09');

-- ============================================================
-- Credit card payments — 2 on different accounts
-- ============================================================
INSERT INTO credit_card_payments (account_id, amount, date, description) VALUES
    (2, 500.00, '2026-03-10', 'Nubank Feb card bill'),
    (4, 300.00, '2026-03-15', 'Inter Feb card bill');

-- ============================================================
-- Installment purchase — Nubank credit card, 3x
-- ============================================================
INSERT INTO installment_purchases (total_amount, installment_count, description, category_id, account_id, first_installment_date)
VALUES (900.00, 3, 'New headphones', 6, 2, '2026-03-01');

-- Note: installment transactions count toward Entertainment budget.
-- 300 * 1 in March = 300, but only the March one is in-period.
-- This pushes Entertainment from 185 to 485... which is 242% of R$200.
-- We want 92%, so we need to adjust. Let's use a non-budgeted category instead.
-- Using Gifts (cat 14, expense) which has no budget.
UPDATE installment_purchases SET category_id = 14 WHERE description = 'New headphones';

INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date, installment_purchase_id, installment_number) VALUES
    (300.00, 'New headphones (1/3)', 14, 2, 'expense', 'credit', '2026-03-01', 1, 1),
    (300.00, 'New headphones (2/3)', 14, 2, 'expense', 'credit', '2026-04-01', 1, 2),
    (300.00, 'New headphones (3/3)', 14, 2, 'expense', 'credit', '2026-05-01', 1, 3);

-- ============================================================
-- Recurring transactions — 2 overdue, 3 future, all 4 frequencies
-- ============================================================
INSERT INTO recurring_transactions (amount, description, category_id, account_id, transaction_type, payment_method, frequency, next_due) VALUES
    -- Overdue (next_due <= 2026-03-17)
    (150.00, 'Electricity bill',  8, 2, 'expense', 'boleto',   'monthly',  '2026-03-10'),
    (89.90,  'Internet bill',     8, 2, 'expense', 'boleto',   'monthly',  '2026-03-15'),
    -- Future
    (44.90,  'Spotify + Netflix', 9, 2, 'expense', 'credit',   'monthly',  '2026-04-05'),
    (20.00,  'Weekly gym',        4, 3, 'expense', 'debit',    'weekly',   '2026-03-24'),
    (120.00, 'Annual domain',     9, 2, 'expense', 'pix',      'yearly',   '2027-01-15');
