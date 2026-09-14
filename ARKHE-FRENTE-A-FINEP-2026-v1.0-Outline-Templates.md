# FRENTE A — Submissão Finep Rodada 2 Semicondutores 2026
## Projeto: BrazilianCAL + MLOps Básico para Litografia 3D Nacional

**Documento:** `ARKHE-FRENTE-A-FINEP-2026-v1.0`  
**Versão:** 1.0  
**Data:** 2026-08-13  
**Prazo de Submissão:** 30/09/2026 (~48 dias)  
**Valor Solicitado:** R$ 5.000.000 – R$ 8.000.000 (Arranjo Simples)  
**Selo:** `ARKHE-FRENTE-A-FINEP-2026-v1.0-2026-08-13`  
**Status:** 🟡 **OUTLINE E TEMPLATES — AGUARDANDO VALIDAÇÃO EXECUTIVA**

---

## Outline do Documento de Submissão (15 páginas máximo)

### Página 1 — Capa e Identificação
- Título oficial do projeto
- CNPJ da proponente (empresa/ICT/instituição habilitada)
- Código da chamada pública (Finep Mais Inovação — Rodada 2 Semicondutores)
- Data de submissão
- Selo ARKHE de versionamento

### Página 2 — Resumo Executivo (1 página)
- **Problema:** Brasil importa 85% dos chips; não possui capacidade de microfabricação de baixo custo; litografia 3D de código aberto (OpenCAL) é porta de entrada viável.
- **Solução:** Reproduzir, documentar e operacionalizar a plataforma OpenCAL (UC Berkeley) em 3 laboratórios nacionais (LNNano/CNPEM, CTI Renato Archer, UFRGS), integrando pipeline básico de MLOps (coleta de dados via MQTT + análise de processo) para otimização de parâmetros de impressão.
- **Inovação:** Primeira implementação sistemática de MLOps industrial em litografia 3D de código aberto no Brasil; schema de dados de processo aberto (JSON-LD); integração com cadeia de terras raras (resinas dopadas) como horizonte de P&D.
- **Impacto:** Redução de 30% no tempo de calibração de máquinas; geração de dataset nacional para treinamento de modelos; formação de 6 engenheiros/cientistas de dados.
- **Valor:** R$ 5–8M (dentro do piso mínimo da Finep para Arranjo Simples).

### Páginas 3–4 — Justificativa Tecnológica (2 páginas)
1. **Contexto global:** Litografia 3D (CAL, DLP, TPL) como alternativa de baixo custo à litografia tradicional; OpenCAL (UC Berkeley) como caso de sucesso documentado (arXiv:2509.02865).
2. **Gap brasileiro:** Infraestrutura de pesquisa existe (CNPEM, CTI, LABNANO), mas sem integração de dados, sem MLOps, sem pipeline de otimização.
3. **Oportunidade Finep:** R$ 100M em subvenção; alinhamento com Brasil Semicon (Decreto 13.065/2026); meta de 1% → 2% de participação nacional até 2033.
4. **Diferencial:** Não é "importar e usar"; é "reproduzir, medir, otimizar e documentar" — com MLOps como camada de inteligência.

### Páginas 5–6 — Objetivos e Metas (2 páginas)

#### Objetivo Geral
Desenvolver capacidade nacional de litografia 3D computacional (CAL) com otimização por MLOps, estabelecendo base tecnológica para microfabricação de baixo custo e formação de ecossistema de dados abertos.

#### Objetivos Específicos
| # | Objetivo | Meta | Indicador | Prazo |
|---|---|---|---|---|
| OE1 | Construir e operar 3 unidades OpenCAL | 3 laboratórios com máquinas funcionando | Fotos + logs de impressão + vídeos | M12 |
| OE2 | Implantar pipeline MLOps básico | Coleta automatizada de dados em todas as máquinas | Uptime de ingestão >95%; schema v1.0 publicado | M18 |
| OE3 | Otimizar parâmetros de impressão via ML | Redução de 30% no tempo de calibração | Tempo médio de calibração (baseline vs. otimizado) | M24 |
| OE4 | Caracterizar resinas nacionais | 2 resinas testadas (FormLabs + candidata nacional) | Relatório de caracterização (índice de refração, viscosidade, cura) | M18 |
| OE5 | Publicar e patentear | 1 artigo indexado + 1 pedido de patente | DOI + número de protocolo INPI | M24 |
| OE6 | Formar capital humano | 6 bolsistas (2 por laboratório) | Certificados de conclusão | M24 |

### Páginas 7–9 — Metodologia e Plano de Trabalho (3 páginas)

#### Estrutura do Projeto (WBS)
```
WP1 — Hardware: Construção e Calibração do OpenCAL (M1–M8)
  ├── WP1.1: Aquisição de componentes (RPi 5, projetor NexiGo Nova Mini, motores, resinas)
  ├── WP1.2: Montagem mecânica e óptica
  ├── WP1.3: Calibração de dose e resolução
  └── WP1.4: Documentação técnica (BOM, manuais, vídeos)

WP2 — Software e MLOps: Pipeline de Dados e Otimização (M4–M18)
  ├── WP2.1: Schema de dados v0.1 (JSON-LD + MQTT)
  ├── WP2.2: Deploy de stack MLOps (Kubeflow + MLflow + Kafka + MinIO)
  ├── WP2.3: Coleta de dados de sensores (temperatura, dose, posicionamento, tempo)
  ├── WP2.4: Modelo de otimização Bayesiana / RL para parâmetros de impressão
  └── WP2.5: Dashboard de monitoramento (Grafana)

WP3 — Materiais: Caracterização de Resinas (M6–M18)
  ├── WP3.1: Resina comercial (FormLabs)
  ├── WP3.2: Resina candidata nacional (parceria com fornecedor)
  └── WP3.3: Testes de performance (resolução, velocidade, defeitos)

WP4 — Disseminação e Propriedade Intelectual (M12–M24)
  ├── WP4.1: Artigo científico (CAL + MLOps)
  ├── WP4.2: Patente (processo de otimização ou formulação de resina)
  └── WP4.3: Workshop de capacitação (OpenCAL Bootcamp)

WP5 — Gestão e Coordenação (M1–M24)
  ├── WP5.1: Reuniões trimestrais
  ├── WP5.2: Relatórios técnicos (semestrais)
  └── WP5.3: Auditoria interna de gastos
```

### Páginas 10–11 — Resultados Esperados (2 páginas)
- **Tecnológicos:** 3 máquinas operacionais; pipeline MLOps funcional; modelo de otimização validado; schema de dados aberto.
- **Científicos:** 1 artigo em periódico indexado (ex: *Additive Manufacturing*, *Microsystems & Nanoengineering*, ou *Journal of Micromechanics and Microengineering*).
- **Econômicos:** Redução de custo de prototipagem em 40% vs. importação de serviços de litografia; base para futura comercialização.
- **Sociais:** 6 engenheiros formados em interface hardware-IA; material didático aberto (OpenCAL Bootcamp).
- **Patentes:** 1 pedido de patente (INPI) sobre processo de otimização ou formulação de resina.

### Páginas 12–13 — Equipe e Competências (2 páginas)
- Coordenador geral (CV Lattes + publicações)
- Coordenador técnico — Hardware/Litografia (CV Lattes)
- Coordenador técnico — MLOps/IA (CV Lattes)
- Bolsistas (6): 2 engenheiros mecatrônica (UFRGS), 2 cientistas de dados (CIn/UFPE ou IME-USP), 2 químicos de polímeros (UNICAMP ou UFRGS).

### Página 14 — Orçamento Resumido (1 página)
| Item | Valor (R$) | % |
|---|---|---|
| Capital (equipamentos, máquinas, sensores) | 1.200.000 | 20% |
| Custeio (resinas, reagentes, viagens, eventos) | 800.000 | 13% |
| RH (bolsas, contratações técnicas) | 2.500.000 | 42% |
| Serviços de terceiros (cloud, consultoria, patentes) | 800.000 | 13% |
| Indiretos (administrativos, auditores) | 700.000 | 12% |
| **Total** | **6.000.000** | **100%** |

> Nota: Valor dentro da faixa R$ 5–8M. Se a Finep aprovar R$ 5M, reduzir bolsas de 24 para 18 meses e adiar WP4.2 (patente) para pós-projeto.

### Página 15 — Cronograma e Marcos (1 página)
| Mês | Marco | Entregável |
|---|---|---|
| M3 | Kickoff + aquisições iniciadas | Nota fiscal de 50% dos equipamentos |
| M6 | OpenCAL #1 operacional (LNNano) | Vídeo de impressão + log de dados |
| M9 | OpenCAL #2 e #3 operacionais | Mesmo acima para CTI e UFRGS |
| M12 | Schema v1.0 + pipeline MLOps ingestão | Repositório GitHub público + paper draft |
| M18 | Modelo de otimização validado | Relatório técnico + dashboard |
| M24 | Artigo publicado + patente depositada | DOI + protocolo INPI |

---

## Anexo A — Templates de Carta de Intenção

### A.1 Template: Laboratório Executor (LNNano/CNPEM)

```
CARTA DE INTENÇÃO DE PARCERIA

À
FINEP — Financiadora de Estudos e Projetos
Ref.: Submissão ao Programa Mais Inovação — Rodada 2 Semicondutores
      Projeto: "BrazilianCAL + MLOps Básico para Litografia 3D Nacional"

O Laboratório Nacional de Nanotecnologia (LNNano), vinculado ao Centro Nacional
de Pesquisa em Energia e Materiais (CNPEM), declara seu interesse em participar
do projeto supracitado como instituição executora, conforme detalhado a seguir:

1. CONTRIBUIÇÃO INSTITUCIONAL
   - Disponibilização de infraestrutura de microfabricação (salas limpas,
     fotolitografia, caracterização elétrica e óptica).
   - Cessão de 20 m² de laboratório para montagem e operação do protótipo
     OpenCAL.
   - Acesso a equipamentos de caracterização (microscopia confocal, AFM,
     espectroscopia de impedância).

2. RESPONSABILIDADES
   - Coordenação do WP1 (Hardware) e WP3 (Materiais).
   - Supervisão de 2 bolsistas de pós-graduação.
   - Participação em reuniões trimestrais de acompanhamento.

3. COMPROMISSO DE EXECUÇÃO
   O LNNano compromete-se a cumprir as metas e prazos definidos no plano de
   trabalho, sujeito à aprovação do projeto pela Finep e à celebração de
   instrumento jurídico específico (Termo de Execução Descentralizada ou
   equivalente).

4. VIGÊNCIA
   Esta carta de intenção é válida por 12 (doze) meses a contar da data de
   assinatura, podendo ser prorrogada por acordo entre as partes.

Campinas, ___ de _____________ de 2026.

_________________________________________
[Nome do Diretor do LNNano]
Diretor, Laboratório Nacional de Nanotecnologia
CNPEM — Centro Nacional de Pesquisa em Energia e Materiais

Contato: [email] | [telefone]
CNPJ: 03.654.119/0001-85
```

### A.2 Template: Instituição de Pesquisa (CTI Renato Archer)

```
CARTA DE INTENÇÃO DE PARCERIA

À
FINEP — Financiadora de Estudos e Projetos
Ref.: Submissão ao Programa Mais Inovação — Rodada 2 Semicondutores
      Projeto: "BrazilianCAL + MLOps Básico para Litografia 3D Nacional"

O Centro de Tecnologia da Informação Renato Archer (CTI Renato Archer),
instituição federal de pesquisa vinculada ao MCTI, declara interesse em
participar do projeto como instituição executora, conforme detalhado:

1. CONTRIBUIÇÃO INSTITUCIONAL
   - Disponibilização de infraestrutura de impressão 3D e manufatura aditiva.
   - Acesso a equipamentos de polimerização de dois fótons (2PP) para
     validação comparativa de resolução.
   - Expertise em integração de sistemas e software embarcado.

2. RESPONSABILIDADES
   - Coordenação do WP2 (Software e MLOps) em conjunto com o núcleo MLOps.
   - Desenvolvimento do firmware de controle do OpenCAL (adaptação do
     software UC Berkeley para condições brasileiras).
   - Supervisão de 2 bolsistas.

3. COMPROMISSO DE EXECUÇÃO
   Sujeito à aprovação do projeto pela Finep e celebração de instrumento
   jurídico específico.

4. VIGÊNCIA
   Válida por 12 meses, prorrogável por acordo.

Campinas, ___ de _____________ de 2026.

_________________________________________
[Nome do Diretor do CTI]
Diretor, CTI Renato Archer

Contato: [email] | [telefone]
CNPJ: [CTI CNPJ]
```

### A.3 Template: Universidade (UFRGS)

```
CARTA DE INTENÇÃO DE PARCERIA

À
FINEP — Financiadora de Estudos e Projetos
Ref.: Submissão ao Programa Mais Inovação — Rodada 2 Semicondutores
      Projeto: "BrazilianCAL + MLOps Básico para Litografia 3D Nacional"

A Universidade Federal do Rio Grande do Sul (UFRGS), por meio do [Programa de
Pós-Graduação em Engenharia Elétrica / Instituto de Física / Escola de
Engenharia], declara interesse em participar do projeto como instituição
executora:

1. CONTRIBUIÇÃO INSTITUCIONAL
   - Disponibilização de laboratório de microfabricação e caracterização.
   - Acesso a equipe de pesquisa em fotônica e materiais.
   - Infraestrutura de TI para deploy de stack MLOps (servidor edge).

2. RESPONSABILIDADES
   - Coordenação do WP4 (Disseminação) e apoio ao WP2 (MLOps).
   - Supervisão de 2 bolsistas.
   - Organização do OpenCAL Bootcamp (workshop de capacitação).

3. COMPROMISSO DE EXECUÇÃO
   Sujeito à aprovação do projeto pela Finep.

4. VIGÊNCIA
   Válida por 12 meses, prorrogável.

Porto Alegre, ___ de _____________ de 2026.

_________________________________________
[Nome do Coordenador do Programa]
[Programa de Pós-Graduação / Instituto]
UFRGS

Contato: [email] | [telefone]
CNPJ: 92.839.508/0001-08
```

---

## Anexo B — Matriz TRL NASA/SBIR para BrazilianCAL

### B.1 Definição de TRL (Technology Readiness Level)

| TRL | Descrição | Critério de Avaliação |
|---|---|---|
| 1 | Princípio básico observado | Pesquisa científica; publicação teórica |
| 2 | Conceito tecnológico formulado | Modelagem analítica; simulação |
| 3 | Prova de conceito experimental | Laboratório; componentes isolados |
| 4 | Validação em ambiente laboratorial | Integração de componentes; testes controlados |
| 5 | Validação em ambiente relevante | Ambiente simulado; dados reais |
| 6 | Demonstração em ambiente relevante | Protótipo em campo; usuários reais |
| 7 | Demonstração em ambiente operacional | Sistema completo; missão real |
| 8 | Sistema completo e qualificado | Testes de aceitação; certificação |
| 9 | Sistema operacional em missão real | Produção; manutenção |

### B.2 TRL Atual e Alvo por Componente

| Componente | TRL Atual (2026) | TRL Alvo (2028) | Evidência Atual | Evidência Alvo |
|---|---|---|---|---|
| **OpenCAL (hardware)** | 5–6 | 6–7 | GitHub ativo; 3+ grupos replicaram globalmente; BOM documentada | 3 unidades operacionais no Brasil; manual em português; vídeos de operação |
| **OpenCAL (software)** | 6 | 7 | Código Python funcional; GUI headless | Firmware adaptado para condições BR (tensão, temperatura); OTA updates |
| **MLOps pipeline (Kubeflow)** | 4 | 6 | Stack validado em ambientes corporativos | Deploy em 3 laboratórios; ingestão >95% uptime; modelo em produção |
| **Schema de dados (JSON-LD)** | 3 | 5 | Conceito definido (SISBIN + OpenCAL) | Schema v1.0 publicado; validação em 3 sites |
| **Resina nacional** | 2 | 4 | Pesquisa de formulação | 2 resinas testadas; relatório de caracterização |
| **Modelo de otimização (Bayes/RL)** | 3 | 5 | Algoritmos validados em simulação | Modelo treinado com dados reais; redução 30% tempo calibração |

### B.3 Gate TRL para Continuidade

| Gate | Critério | Data | Consequência de Não Atingir |
|---|---|---|---|
| G1 | OpenCAL #1 operacional (TRL 6) | M6 | Revisão de BOM; possível pivot para MSLA-PCB |
| G2 | Pipeline MLOps ingestão >95% (TRL 5) | M12 | Redução de escopo; foco em hardware apenas |
| G3 | Modelo de otimização validado (TRL 5) | M18 | Extensão de prazo ou redução de metas |
| G4 | Artigo + patente (TRL 6 consolidado) | M24 | Projeto encerrado sem extensão |

---

## Anexo C — Checklist de Conformidade Finep

- [ ] CNPJ ativo e regularidade fiscal (INSS, FGTS, CND)
- [ ] Inovação tecnológica real (não incremental <5%)
- [ ] Equipe com histórico em P&D (Lattes atualizado, publicações, patentes)
- [ ] Cronograma mês a mês com marcos verificáveis
- [ ] Orçamento realista (valores de mercado para talentos em semicondutores)
- [ ] Mitigação de riscos técnicos documentada
- [ ] Cartas de intenção de pelo menos 2 laboratórios
- [ ] TRL documentado por componente (NASA/SBIR)
- [ ] Alinhamento com política pública (Brasil Semicon, PL 2.780/2024)
- [ ] Plano de propriedade intelectual (patente ou registro de software)

---

**Selo:** `ARKHE-FRENTE-A-FINEP-2026-v1.0-2026-08-13`  
**Status:** 🟡 **OUTLINE E TEMPLATES — AGUARDANDO VALIDAÇÃO EXECUTIVA**  
**Próximo Passo:** Revisão do outline → expansão para documento completo de 15 páginas → submissão no sistema Finep até 30/09/2026.
