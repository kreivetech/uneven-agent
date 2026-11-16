# Benchmark de Classificação de Intenção do Shell — Uneven Core

Este relatório documenta a execução de teste ao vivo da classificação semântica de intenções da IA interativa (`interactive.rs` / `shell_classifier.rs`) utilizando as regras do shell (`laws::get_context("shell")`), temperatura `0.1` e a API do Google Gemini.

## 1. Telemetria da Execução

* **Data e Hora:** 2026-11-13 19:04:05 (UTC-03:00)
* **Modelo Utilizado:** `gemini-2.5-flash`
* **Temperatura:** `0.1` (idêntica ao classificador de produção)
* **Tempo de Resposta (Latência):** 4.00 segundos
* **Tokens de Entrada (Prompt + Contexto):** 2.766 tokens
* **Tokens de Saída (Completion):** 161 tokens
* **Total de Tokens:** 2.927 tokens
* **Custo Estimado (USD):** $0.000256 USD
* **Custo Estimado (BRL @ 5.80):** R$ 0,00148

## 2. Prompt Complexo Submetido

> *"Estamos tendo um problema crítico de performance no gateway de autenticação: sob alta carga de requisições concorrentes, as conexões com o PostgreSQL se esgotam e causam timeout nas requisições. Analise a arquitetura atual em `src/infrastructure/db.rs` e `src/services/auth.rs`, verifique se o pool de conexões está com leak ou sem limite definido, e proponha a refatoração necessária para implementar pooling assíncrono seguro com backoff exponencial e métricas de monitoramento."*

## 3. Resposta Bruta da IA (Sem Parser)

```shell-action
command: think
args: Analyze the authentication gateway's database connection pooling in `src/infrastructure/db.rs` and `src/services/auth.rs` for leaks or undefined limits. Propose a refactoring to implement secure asynchronous pooling with exponential backoff and monitoring metrics to address PostgreSQL connection exhaustion under high concurrent load.
paths: src/infrastructure/db.rs,src/services/auth.rs
```
Entendido. Estou analisando a arquitetura atual em `src/infrastructure/db.rs` e `src/services/auth.rs` para identificar a causa do esgotamento das conexões PostgreSQL e propor uma refatoração para implementar pooling assíncrono seguro com backoff exponencial e métricas de monitoramento.

## 4. Análise da Decisão da IA

* **Comando Selecionado (`command: think`):**
  * **Precisão Perfeita:** A IA obedeceu estritamente à hierarquia de roteamento. Diante de um problema de concorrência, investigação de código e proposta de refatoração, ela não tentou aplicar alterações diretamente (`dde`) e nem executar ferramentas cegas. Ela acionou o motor `think` (Deep Reasoning e planejamento de implementação).
* **Tradução da Tarefa (`args`):**
  * Converteu com fidelidade técnica a tarefa para o inglês, capturando a essência do problema (leaks, limites de pooling, backoff exponencial e métricas).
* **Extração de Arquivos (`paths`):**
  * Extraiu com exatidão os dois arquivos impactados: `src/infrastructure/db.rs,src/services/auth.rs`.
* **Preservação da Linguagem Natural:**
  * O feedback conversacional foi mantido em português, respeitando a regra de espelhar o idioma de interação do desenvolvedor.

## 5. Como Reproduzir este Teste

O teste pode ser executado a qualquer momento com:

```bash
# Execução com o prompt padrão complexo:
cargo run -p uneven-core --example test_classify_live

# Ou com qualquer prompt customizado que desejar avaliar:
cargo run -p uneven-core --example test_classify_live -- "seu prompt aqui"
```
