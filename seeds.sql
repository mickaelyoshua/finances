-- Demo seed data for the Finances TUI.
-- A believable Brazilian April-2026 month-in-progress (today ≈ 2026-04-14):
-- four accounts, ~45 transactions across March/April, active installments,
-- recurring bills, transfers, credit-card payments, and budget thresholds
-- tuned so the dashboard shows one exceeded budget, one approaching the
-- limit, and a couple of overdue bills.
--
-- Idempotent: TRUNCATEs every seed-owned table and resets identity
-- sequences so re-running reproduces the exact same dataset.
--
-- Run with:
--   make seed
-- or directly:
--   docker exec -i finances-tui-db-1 psql -U finances -d finances < seeds.sql

BEGIN;

-- Wipe all seed-owned data. `accounts` is pruned selectively because
-- the initial migration inserts the 'Cash' row (id=1) we want to keep.
TRUNCATE TABLE
    transactions,
    transfers,
    credit_card_payments,
    installment_purchases,
    budgets,
    recurring_transactions,
    notifications
RESTART IDENTITY CASCADE;

DELETE FROM accounts WHERE id > 1;
ALTER SEQUENCE accounts_id_seq RESTART WITH 2;

-- ============================================================
-- Accounts  (Cash id=1 preserved from migration)
-- ============================================================
INSERT INTO accounts (name, account_type, has_credit_card, credit_limit, billing_day, due_day, has_debit_card) VALUES
    ('Nubank',   'checking', TRUE,  5000.00, 3,  10, TRUE),   -- id=2
    ('PicPay',   'checking', FALSE, NULL,    NULL, NULL, TRUE), -- id=3
    ('Inter',    'checking', TRUE,  3000.00, 5,  15, TRUE),   -- id=4
    ('Bradesco', 'checking', TRUE,  2000.00, 10, 20, FALSE);  -- id=5

-- ============================================================
-- Budgets  (monthly — tuned for the April dashboard snapshot)
--   Moradia       R$2500 / R$1500 spent = 60%  → Budget50 notif
--   Alimentação   R$ 800 / R$ 300 spent = 37%  → safe
--   Transporte    R$ 400 / R$ 310 spent = 77%  → Budget75 notif
--   Saúde         R$ 250 / R$  80 spent = 32%  → safe
--   Lazer         R$ 300 / R$ 320 spent =106%  → BudgetExceeded
--   Assinaturas   R$ 120 / R$  45 spent = 37%  → safe
--   Vestuário     R$ 400 / R$ 280 spent = 70%  → safe
-- ============================================================
INSERT INTO budgets (category_id, amount, period) VALUES
    (1, 2500.00, 'monthly'),  -- Housing
    (2,  800.00, 'monthly'),  -- Food
    (3,  400.00, 'monthly'),  -- Transport
    (4,  250.00, 'monthly'),  -- Health
    (6,  300.00, 'monthly'),  -- Entertainment
    (9,  120.00, 'monthly'),  -- Subscriptions
    (7,  400.00, 'monthly');  -- Clothing

-- ============================================================
-- March 2026  — previous month, for history depth
-- ============================================================
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    -- Income
    (5200.00, 'Salário Março',            16, 2, 'income',  'transfer', '2026-03-01'),
    ( 600.00, 'Freelance design gig',     17, 4, 'income',  'pix',      '2026-03-22'),
    ( 150.00, 'Rendimento CDB',           18, 5, 'income',  'transfer', '2026-03-31'),
    -- Fixed
    (1500.00, 'Aluguel Março',             1, 2, 'expense', 'boleto',   '2026-03-01'),
    ( 135.00, 'Conta de luz',              8, 2, 'expense', 'boleto',   '2026-03-10'),
    ( 110.00, 'Internet Vivo Fibra',       8, 2, 'expense', 'boleto',   '2026-03-12'),
    -- Variable
    ( 180.00, 'Supermercado Pão de Açúcar',2, 2, 'expense', 'debit',    '2026-03-03'),
    (  95.00, 'Jantar restaurante',        2, 2, 'expense', 'credit',   '2026-03-14'),
    (  32.50, 'iFood delivery',            2, 2, 'expense', 'credit',   '2026-03-28'),
    (  55.00, 'Farmácia Drogasil',         4, 2, 'expense', 'pix',      '2026-03-08'),
    (  44.90, 'Spotify + Netflix',         9, 2, 'expense', 'credit',   '2026-03-05'),
    ( 170.00, 'Gasolina Shell',            3, 4, 'expense', 'debit',    '2026-03-20'),
    (  48.00, 'Cinema + pipoca',           6, 2, 'expense', 'credit',   '2026-03-25'),
    (  89.00, 'Online Rust course',        5, 2, 'expense', 'credit',   '2026-03-06'),
    (  85.00, 'Ração Golden cachorro',    13, 3, 'expense', 'debit',    '2026-03-15'),
    ( 120.00, 'Presente aniversário mãe', 14, 5, 'expense', 'credit',   '2026-03-11'),
    (  65.00, 'Corte de cabelo',          12, 1, 'expense', 'cash',     '2026-03-18');

-- ============================================================
-- April 2026  — current month (today = 2026-04-14)
-- ============================================================
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date) VALUES
    -- Income
    (5200.00, 'Salário Abril',            16, 2, 'income',  'transfer', '2026-04-01'),
    (1200.00, 'Projeto freelance React',  17, 4, 'income',  'pix',      '2026-04-04'),
    ( 180.00, 'Dividendos BBAS3',         18, 5, 'income',  'transfer', '2026-04-10'),
    -- Housing — Budget50 (60%)
    (1500.00, 'Aluguel Abril',             1, 2, 'expense', 'boleto',   '2026-04-01'),
    -- Food — safe (37%)  total 300
    ( 180.00, 'Supermercado Pão de Açúcar',2, 2, 'expense', 'debit',    '2026-04-02'),
    (  42.00, 'Almoço restaurante',        2, 2, 'expense', 'credit',   '2026-04-05'),
    (  28.00, 'iFood pizza noite',         2, 2, 'expense', 'credit',   '2026-04-08'),
    (  35.00, 'Feira de domingo',          2, 1, 'expense', 'cash',     '2026-04-12'),
    (  15.00, 'Padaria café da manhã',     2, 1, 'expense', 'cash',     '2026-04-13'),
    -- Transport — Budget75 (77%)  total 310
    ( 180.00, 'Gasolina Shell',            3, 4, 'expense', 'debit',    '2026-04-02'),
    (  45.00, 'Uber para o aeroporto',     3, 2, 'expense', 'pix',      '2026-04-06'),
    (  50.00, 'Recarga Bilhete Único',     3, 2, 'expense', 'debit',    '2026-04-09'),
    (  35.00, 'Estacionamento shopping',   3, 4, 'expense', 'debit',    '2026-04-12'),
    -- Health — safe (32%)  total 80
    (  45.00, 'Farmácia Drogasil',         4, 2, 'expense', 'pix',      '2026-04-03'),
    (  35.00, 'Consulta dentista copay',   4, 4, 'expense', 'debit',    '2026-04-10'),
    -- Entertainment — EXCEEDED (106%)  total 320
    (  65.00, 'Cinema + pipoca',           6, 2, 'expense', 'credit',   '2026-04-03'),
    ( 120.00, 'Show ingresso Pitty',       6, 5, 'expense', 'credit',   '2026-04-07'),
    (  85.00, 'Steam Spring Sale',         6, 2, 'expense', 'credit',   '2026-04-11'),
    (  50.00, 'Bar com amigos',            6, 2, 'expense', 'credit',   '2026-04-14'),
    -- Subscriptions — safe (37%)
    (  44.90, 'Spotify + Netflix',         9, 2, 'expense', 'credit',   '2026-04-05'),
    -- Clothing — safe (70%)
    ( 280.00, 'Tênis Adidas Ultraboost',   7, 2, 'expense', 'credit',   '2026-04-08'),
    -- Utilities (no budget)
    ( 145.00, 'Conta de luz',              8, 2, 'expense', 'boleto',   '2026-04-05'),
    -- Education (no budget)
    (  35.00, 'Udemy SQL masterclass',     5, 2, 'expense', 'credit',   '2026-04-06'),
    -- Personal care (no budget)
    (  50.00, 'Barbeiro',                 12, 1, 'expense', 'cash',     '2026-04-04');

-- ============================================================
-- Installments  (3 active, mid-flight — dates span multiple statements)
-- ============================================================
INSERT INTO installment_purchases (total_amount, installment_count, description, category_id, account_id, first_installment_date) VALUES
    ( 900.00, 3, 'Headphones Sony WH-1000XM5', 15, 2, '2026-03-01'),
    (1200.00, 6, 'Bicicleta ergométrica',      15, 4, '2026-02-10'),
    (1000.00, 4, 'Curso Full-Stack 2026',       5, 5, '2026-03-05');

-- Expand installments into concrete transactions (past + future).
INSERT INTO transactions (amount, description, category_id, account_id, transaction_type, payment_method, date, installment_purchase_id, installment_number) VALUES
    -- Headphones 3x R$300 on Nubank credit
    (300.00, 'Headphones Sony WH-1000XM5 (1/3)', 15, 2, 'expense', 'credit', '2026-03-01', 1, 1),
    (300.00, 'Headphones Sony WH-1000XM5 (2/3)', 15, 2, 'expense', 'credit', '2026-04-01', 1, 2),
    (300.00, 'Headphones Sony WH-1000XM5 (3/3)', 15, 2, 'expense', 'credit', '2026-05-01', 1, 3),
    -- Bicicleta 6x R$200 on Inter credit
    (200.00, 'Bicicleta ergométrica (1/6)', 15, 4, 'expense', 'credit', '2026-02-10', 2, 1),
    (200.00, 'Bicicleta ergométrica (2/6)', 15, 4, 'expense', 'credit', '2026-03-10', 2, 2),
    (200.00, 'Bicicleta ergométrica (3/6)', 15, 4, 'expense', 'credit', '2026-04-10', 2, 3),
    (200.00, 'Bicicleta ergométrica (4/6)', 15, 4, 'expense', 'credit', '2026-05-10', 2, 4),
    (200.00, 'Bicicleta ergométrica (5/6)', 15, 4, 'expense', 'credit', '2026-06-10', 2, 5),
    (200.00, 'Bicicleta ergométrica (6/6)', 15, 4, 'expense', 'credit', '2026-07-10', 2, 6),
    -- Curso 4x R$250 on Bradesco credit
    (250.00, 'Curso Full-Stack 2026 (1/4)',  5, 5, 'expense', 'credit', '2026-03-05', 3, 1),
    (250.00, 'Curso Full-Stack 2026 (2/4)',  5, 5, 'expense', 'credit', '2026-04-05', 3, 2),
    (250.00, 'Curso Full-Stack 2026 (3/4)',  5, 5, 'expense', 'credit', '2026-05-05', 3, 3),
    (250.00, 'Curso Full-Stack 2026 (4/4)',  5, 5, 'expense', 'credit', '2026-06-05', 3, 4);

-- ============================================================
-- Transfers  (between own accounts — don't count as income/expense)
-- ============================================================
INSERT INTO transfers (from_account_id, to_account_id, amount, description, date) VALUES
    (2, 3,  250.00, 'Top-up PicPay',              '2026-03-03'),
    (4, 5,  400.00, 'Reserva de emergência',      '2026-03-15'),
    (2, 3,  300.00, 'Mesada para gastos',         '2026-04-01'),
    (2, 1,  200.00, 'Saque para a semana',        '2026-04-02'),
    (4, 5,  500.00, 'Aporte reserva',             '2026-04-05'),
    (2, 5, 1000.00, 'Investir em CDB Bradesco',   '2026-04-08');

-- ============================================================
-- Credit-card payments  (past bills already settled)
-- ============================================================
INSERT INTO credit_card_payments (account_id, amount, date, description) VALUES
    (2,  890.00, '2026-03-10', 'Fatura Nubank Fev'),
    (4,  420.00, '2026-03-15', 'Fatura Inter Fev'),
    (5,  540.00, '2026-03-20', 'Fatura Bradesco Fev'),
    (2, 1050.00, '2026-04-10', 'Fatura Nubank Mar');

-- ============================================================
-- Recurring transactions  (2 overdue, 4 upcoming, mixed frequencies)
-- ============================================================
INSERT INTO recurring_transactions (amount, description, category_id, account_id, transaction_type, payment_method, frequency, next_due) VALUES
    ( 145.00, 'Conta de luz',         8, 2, 'expense', 'boleto',  'monthly', '2026-04-10'),  -- overdue
    ( 110.00, 'Internet Vivo Fibra',  8, 2, 'expense', 'boleto',  'monthly', '2026-04-12'),  -- overdue
    (  89.00, 'Academia mensalidade', 4, 3, 'expense', 'pix',     'monthly', '2026-04-20'),
    (  44.90, 'Spotify + Netflix',    9, 2, 'expense', 'credit',  'monthly', '2026-05-05'),
    (  35.00, 'IPTV streaming',       6, 2, 'expense', 'credit',  'monthly', '2026-04-25'),
    ( 120.00, 'Domínio anual .com',   9, 2, 'expense', 'pix',     'yearly',  '2027-01-15');

COMMIT;
