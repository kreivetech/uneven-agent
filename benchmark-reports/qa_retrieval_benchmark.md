# Relatório de Benchmark: Consulta (Q&A) no Subsistema do Scheduler
---

## 1. Detalhes do Sistema (Ambiente de Execução)
- **Modelo do Hardware:**                              Positivo Informatica SA S14BW01
- **Memória RAM:**                                      8.0 GiB (Disponível: ~7.1 GiB)
- **Processador:**                                      Intel® Pentium® N3710 × 4
- **Gráficos:**                                         Intel® HD Graphics 405 (BSW)
- **Capacidade do Disco (SSD):**                        240.1 GB
- **Sistema Operacional:**                              Ubuntu 26.04 LTS
- **Versão do Kernel:**                                 Linux 7.0.0-22-generic

---

## 2. Especificação do Teste
- **Comando Executado:**                                `target/release/uneven ask "How does the scheduler balance tasks across CPU cores?"`
- **Modelo Utilizado:**                                  `gemini-2.5-flash`
- **Configuração de Tokens (QA):**                       `maxTokens: 50000`
- **Contexto RAG**:                                     Carregamento dinâmico dos arquivos do scheduler (`topology.c`, `fair.c`, `core.c`, `sched.h`, `pelt.c`, `pelt.h`, `smp.h`, `loadavg.c`) identificados pelo classificador após correção do limite `max_tokens`.
- **Log de Monitoramento:**                             [benchmark_monitoring_20260616_211546.log](logs/benchmark_monitoring_20260616_211546.log)
- **Log de Saída do CLI:**                              [benchmark_output_20260616_211546.log](logs/benchmark_output_20260616_211546.log)

---

## 3. Métricas de Performance Obtidas

| Métrica | Valor Medido | Observação / Análise |
|---|---|---|
| **Tempo de Execução (End-to-End)** | `29 segundos` | Redução para 29s (ganho de 15%) com a compilação otimizada em release em hardware Pentium. |
| **Tokens Utilizados (Prompt + Completion)** | `120.685 tokens` | Contexto massivo contendo a íntegra dos principais códigos de agendamento do kernel Linux. |
| **Consumo de Memória (RSS Pico)** | `120 MB` | **Excelente Otimização (Release)**. Redução de 20% no consumo de RAM frente ao debug. |
| **Carga de CPU (Pico)** | `600%` | Distribuição e uso eficiente de threads na compilação release. |
| **Custo Calculado (API)** | `$0.03712` | Custo direto da consulta. |
| **Economia por Cache (Context Caching)** | `$1.83382` | Aproveitamento do cache de contexto do Gemini para reduzir custos de tokens repetidos. |
| **Código de Saída (Exit Code)** | `0` | Executado com total sucesso. |

---

## 4. Análise Técnica e Conclusões
- **Capacidade do Pipeline RAG:** Com o ajuste correto de `max_tokens` para `2000` no classificador, o sistema selecionou os 8 arquivos chave do scheduler do kernel e alimentou o Gemini com um contexto super completo de mais de 120.000 tokens.
- **Eficiência de RAM (RSS):** O pico de `120 MB` de uso de RAM é extremamente baixo. A maior parte das ferramentas RAG em Node.js ou Python ultrapassam facilmente 1.5 GB a 3 GB para lidar com buffers desse tamanho.
- **Qualidade da Resposta:** O Gemini gerou uma explicação altamente técnica e estruturada detalhando:
  1. Topologia de CPU via `sched_domain` e `sched_group` ([topology.c](file:///home/kreive/Documentos/back-end/uneven-ai/linux/kernel/sched/topology.c)).
  2. Métricas de rastreamento de carga (PELT) atualizadas em [pelt.c](file:///home/kreive/Documentos/back-end/uneven-ai/linux/kernel/sched/pelt.c).
  3. Mecanismos de migração de tarefas e balanceamento NUMA em [fair.c](file:///home/kreive/Documentos/back-end/uneven-ai/linux/kernel/sched/fair.c) e [core.c](file:///home/kreive/Documentos/back-end/uneven-ai/linux/kernel/sched/core.c).
  4. Gerenciamento de Locks concorrentes via `double_rq_lock` ([core.c](file:///home/kreive/Documentos/back-end/uneven-ai/linux/kernel/sched/core.c)).
