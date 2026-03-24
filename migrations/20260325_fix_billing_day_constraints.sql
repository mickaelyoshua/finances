-- Fix billing_day and due_day constraints to allow 1-31 (was 1-28).

ALTER TABLE accounts DROP CONSTRAINT IF EXISTS accounts_billing_day_check;
ALTER TABLE accounts ADD CONSTRAINT accounts_billing_day_check CHECK (billing_day BETWEEN 1 AND 31);

ALTER TABLE accounts DROP CONSTRAINT IF EXISTS accounts_due_day_check;
ALTER TABLE accounts ADD CONSTRAINT accounts_due_day_check CHECK (due_day BETWEEN 1 AND 31);
