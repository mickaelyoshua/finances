-- Performance indexes for foreign-key lookups and filtered queries.

CREATE INDEX IF NOT EXISTS idx_transfers_from_account ON transfers (from_account_id);
CREATE INDEX IF NOT EXISTS idx_transfers_to_account ON transfers (to_account_id);
CREATE INDEX IF NOT EXISTS idx_cc_payments_account ON credit_card_payments (account_id);
CREATE INDEX IF NOT EXISTS idx_transactions_type_method ON transactions (account_id, transaction_type, payment_method);
