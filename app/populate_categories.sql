BEGIN;

INSERT INTO expense_categories (name)
VALUES
('Habitação'), -- 1
('Transporte'), -- 2
('Saúde'), -- 3
('Educação'), -- 4
('Alimentação'), -- 5
('Vestuário'), -- 6
('Cuidado Pessoal'), -- 7
('Lazer'), -- 8
('Dívida'), -- 9
('Doação'), -- 10
('Investimento'), -- 11
('Depósito de Segurança'), -- 12
('Outro'); -- 13

INSERT INTO expense_sub_categories (category_id, name)
VALUES
-- HABITAÇÃO
(1, 'Aluguel'),
(1, 'Condomínio'),
(1, 'Prestação'),
(1, 'Diarista'),
(1, 'Secretária'),
(1, 'Luz'),
(1, 'Telefone Fixo'),
(1, 'Gás'),
(1, 'Internet'),
(1, 'IPTU'),
(1, 'Compra de Utensílio'),
(1, 'Supermercado (Limpeza)'),
(1, 'Seguro'),
(1, 'Conserto'),
(1, 'Piscineiro'),
(1, 'Jardineiro'),
(1, 'TV Assinatura'),
(1, 'Celular'),
(1, 'Outro'),

-- TRANSPORTE
(2, 'Prestação de Veículo'),
(2, 'Seguro'),
(2, 'Multa'),
(2, 'Licenciamento'),
(2, 'Combustível'),
(2, 'Estacionamento'),
(2, 'IPVA'),
(2, 'Troca de Óleo'),
(2, 'Troca de Pneu'),
(2, 'Lavagem de Veículo'),
(2, 'Manutenção de Veículo'),
(2, 'Transporte Público'),
(2, 'Uber/99/Táxi'),
(2, 'Outro'),

-- SAÚDE
(3, 'Plano de Saúde'),
(3, 'Seguro de Vida'),
(3, 'Medicamento'),
(3, 'Médico'),
(3, 'Dentista'),
(3, 'Academia'),
(3, 'Psicólogo'),
(3, 'Fisioterapia'),
(3, 'Óculos/Lente'),
(3, 'Outro'),

-- EDUCAÇÃO
(4, 'Mensalidade'),
(4, 'Material Escolar'),
(4, 'Fardamento'),
(4, 'Curso'),
(4, 'Faculdade'),
(4, 'Livro'),
(4, 'Transporte Escolar'),
(4, 'Passeio Escolar'),
(4, 'Outro'),

-- ALIMENTAÇÃO
(5, 'Supermercado'),
(5, 'Feira'),
(5, 'Açougue'),
(5, 'Restaurante'),
(5, 'Lanchonete'),
(5, 'Padaria'),
(5, 'Delivery'),
(5, 'Outro'),

-- VESTUÁRIO
(6, 'Roupa'),
(6, 'Calçado'),
(6, 'Acessório'),
(6, 'Outro'),

-- CUIDADO PESSOAL
(7, 'Supermercado (Higiene Pessoal)'),
(7, 'Cabeleireiro'),
(7, 'Barbeiro'),
(7, 'Manicure'),
(7, 'Pedicure'),
(7, 'Estética'),
(7, 'Cosmético'),
(7, 'Depilação'),
(7, 'Massagem'),
(7, 'Spa'),
(7, 'Produto de Beleza'),
(7, 'Outro'),

-- LAZER
(8, 'Cinema'),
(8, 'Teatro'),
(8, 'Show'),
(8, 'Balada'),
(8, 'Viagem'),
(8, 'Passeio'),
(8, 'Evento'),
(8, 'Hobby'),
(8, 'Jogo'),
(8, 'Atividade Recreativa'),
(8, 'Outro'),

-- DÍVIDA
(9, 'Cheque'),
(9, 'Empréstimo'),
(9, 'Financiamento'),
(9, 'Outro'),

-- DOAÇÃO
(10, 'Instituição de Caridade'),
(10, 'Dízimo'),
(10, 'Oferta'),
(10, 'Família'),
(10, 'Apoio a Amigo/Parente'),
(10, 'Campanha Social'),
(10, 'Outro'),

-- INVESTIMENTO
(11, 'Ações'),
(11, 'Fundo de Investimento'),
(11, 'Poupança'),
(11, 'Criptomoeda'),
(11, 'Imóvel'),
(11, 'Compra de Ativos'),
(11, 'Previdência'),
(11, 'Outro'),

-- DEPÓSITO DE SEGURANÇA
(12, 'Aluguel'),
(12, 'Contrato'),
(12, 'Outro'),

-- OUTRO
(13, 'Taxa Conta Corrente'),
(13, 'Taxa Limite Cheque Especial'),
(13, 'Anuidade Cartão'),
(13, 'Animal Doméstico'),
(13, 'Plano Funerário'),
(13, 'Presentes'),
(13, 'Taxas'),
(13, 'Mesadas'),
(13, 'Pensão Alimentícia');


--------------------------------------------------------------------------------------------------------------------------

INSERT INTO income_categories (name)
VALUES
('Salário'),
('Pró Labore'),
('Aluguel Recebido'),
('Freelancer'),
('Pensão'),
('Empréstimo Recebido'),
('Mesada'),
('Seguro'),
('Comissão'),
('Bônus'),
('Herança'),
('13º Salário'),
('Rendimento de Investimento'),
('Venda de Bens'),
('Venda de Ações'),
('Venda de Imóvel'),
('Venda de Veículo'),
('Doação Recebida'),
('Outro');

COMMIT;