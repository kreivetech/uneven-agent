# Benchmark de Análise e Síntese de IA — Uneven Core

Este relatório documenta a execução de teste ao vivo da infraestrutura de análise, blind shield security, parsing de resposta e compilação de dashboards HTML interativos utilizando a API do Google Gemini.

## 1. Telemetria da Execução

* **Data e Hora:** 2026-11-13 18:17:20 (UTC-03:00)
* **Modelo Utilizado:** `gemini-2.5-flash`
* **Provedor:** Google Gemini via `llmcraft`
* **Tempo de Resposta (Latência):** 6.83 segundos
* **Tokens de Entrada (Prompt):** 719 tokens
* **Tokens de Saída (Completion):** 365 tokens
* **Total de Tokens:** 1.084 tokens
* **Custo Estimado (USD):** $0.000163 USD
* **Custo Estimado (BRL @ 5.80):** R$ 0,000945

## 2. Cenário de Teste e Blind Security

O teste preparou um schema relacional com tabelas operacionais e colunas protegidas:

* **Tabela `customers`:**
  * Colunas: `customer_id` (INTEGER, PK), `company_name` (TEXT), `segment` (TEXT), `country` (TEXT)
  * Amostras celulares mascaradas: `[{"customer_id": 1, "company_name": "Acme Corp", "segment": "Enterprise", "country": "BR"}]`
* **Tabela `orders`:**
  * Colunas: `order_id` (INTEGER, PK), `customer_id` (INTEGER, FK), `region` (TEXT), `total_amount` (REAL), `status` (TEXT), `created_at` (TEXT), `credit_card_token` (TEXT)
* **Políticas do Blind Shield (`shieldcraft`):**
  * Colunas bloqueadas: `credit_card_token`, `password`
  * Tabelas bloqueadas: `secret_audit_log`
  * Colunas mascaradas: `email`
  * Validação pré-envio: A coluna `credit_card_token` foi estritamente omitida da visualização de schema entregue à IA.

## 3. Pergunta Analítica Submetida

> *"Quais são as principais regiões comerciais por faturamento total e número de pedidos concluídos?"*

## 4. Resposta Retornada pela IA

```markdown
### Explanation
This query identifies the top commercial regions by calculating the total revenue (`total_amount`) and the count of completed orders for each region. It filters for 'completed' orders and then aggregates these metrics per `region`, ordering the results to highlight regions with higher revenue first.

### Report
This report will reveal the commercial regions that generate the most revenue and process the highest number of completed orders. It provides key insights into regional performance, allowing for strategic decisions on resource allocation, marketing efforts, or operational improvements based on where the business is thriving or where there's potential for growth.

```sql:dataset_main
SELECT
  region,
  SUM(total_amount) AS total_revenue,
  COUNT(order_id) AS completed_orders_count
FROM orders
WHERE
  status = 'completed'
GROUP BY
  region
ORDER BY
  total_revenue DESC,
  completed_orders_count DESC;
```

```dashboard
title: "Análise de Faturamento e Pedidos por Região"
theme: dark
palette: ["#10b981", "#06b6d4", "#8b5cf6"]
layout:
- type: kpi
  dataset: dataset_main
  metric: total_revenue
  label: "Faturamento Total (Concluído)"
  format: currency
  span: 4
- type: chart
  dataset: dataset_main
  chartType: bar
  title: "Faturamento por Região (Concluído)"
  x: region
  y: total_revenue
  span: 8
- type: table
  dataset: dataset_main
  title: "Detalhes por Região"
  span: 12
```
```

## 5. Validações e Resultados do Pipeline

* **Segurança da Query (`shieldcraft::is_safe_query`):** Aprovada (estritamente read-only `SELECT`, sem mutações DDL/DML).
* **Decomposição Estruturada (`ResponseParser`):**
  * Extração limpa do bloco SQL (`dataset_main`).
  * Extração do sumário técnico (`explanation`) e relatório executivo (`executive_report`).
  * Extração da especificação do painel (`dashboard_spec` / `visual_suggestions`).
* **Compilação do Dashboard Bento (`generate_dashboard`):**
  * Arquivo gerado: `target/reports/test_analyze_dashboard.html`
  * Tamanho do arquivo: 39.43 KB
  * Componentes renderizados: KPI cards, gráfico de barras por região e tabela de dados paginada.

## 6. Como Reproduzir este Teste

O teste está isolado em `examples/` para não consumir cotas de API durante execuções de `cargo test`:

```bash
cargo run -p uneven-core --example test_analyze_live
```
