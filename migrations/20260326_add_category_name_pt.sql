-- Add Portuguese name column for bilingual category support
ALTER TABLE categories ADD COLUMN IF NOT EXISTS name_pt VARCHAR(100);

-- Seed Portuguese names for default categories.
-- WHERE name_pt IS NULL makes this idempotent: won't overwrite user-set values.
UPDATE categories SET name_pt = CASE name
    -- Expense categories
    WHEN 'Housing' THEN 'Moradia'
    WHEN 'Food' THEN 'Alimentação'
    WHEN 'Transportation' THEN 'Transporte'
    WHEN 'Health' THEN 'Saúde'
    WHEN 'Education' THEN 'Educação'
    WHEN 'Entertainment' THEN 'Lazer'
    WHEN 'Clothing' THEN 'Vestuário'
    WHEN 'Utilities' THEN 'Utilidades'
    WHEN 'Subscriptions' THEN 'Assinaturas'
    WHEN 'Taxes' THEN 'Impostos'
    WHEN 'Debt' THEN 'Dívidas'
    WHEN 'Personal Care' THEN 'Cuidados Pessoais'
    WHEN 'Pets' THEN 'Animais'
    WHEN 'Gifts' THEN 'Presentes'
    WHEN 'Other' THEN 'Outros'
    -- Income categories
    WHEN 'Salary' THEN 'Salário'
    WHEN 'Freelance' THEN 'Freelance'
    WHEN 'Investments' THEN 'Investimentos'
    WHEN 'Sales' THEN 'Vendas'
    ELSE NULL
END
WHERE name_pt IS NULL;
