"""
Refinamento iterativo de prompt — Catedral OS v354.0 (Bloco 966).

Implementa Self-Refine (Madaan et al., 2023, arXiv:2303.17651).
NÃO é retrocausalidade: é controle cibernético forward-causal
(saída em t é entrada em t+1; Wiener, 1948).

Regras de honestidade (regra da casa, precedente dos blocos 483-557):
- Sem stubs ocultos (measure_coherence, extract_gaps, etc. NÃO existem aqui).
- A "avaliação" é delegada a um SEGUNDO prompt ao próprio LLM.
- Nenhum "Φ" nem "lim = 1" é declarado. Convergência NÃO é garantida
  (ver I491 no PromptRefinement.lean); o critério de parada "OK" é heurístico.

Invariantes tocados:
- Simplicity-1/2: uma classe, zero dependências externas.
- Ethics-2 (Data Minimization): `self.history` permanece em memória do
  processo; o chamador decide se persiste. Nada é enviado à rede além
  dos prompts de geração/feedback.
- Loopseal-2 (auditabilidade): cada iteração fica registrada em
  `self.history` (append-only em memória).
"""

from typing import Any, Callable, List


class IterativeRefiner:
    """
    Self-Refine honesto: o modelo avalia a própria saída e a corrige.

    A qualidade do feedback depende inteiramente do modelo subjacente
    fornecido em `generate`. Não há métricas mágicas: se o modelo não
    detectar problemas, o loop para com a resposta atual.
    """

    def __init__(self, generate: Callable[[str], str], max_iterations: int = 3) -> None:
        self.generate = generate  # Função REAL de geração (ex.: API OpenAI).
        self.max_iterations = max_iterations
        self.history: List[dict] = []

    def refine(self, task_prompt: str) -> str:
        """Gera, avalia e refina iterativamente.

        Retorna a última resposta gerada. Nunca fabrica convergência:
        se max_iterations esgotar sem feedback "OK", o loop termina
        com o estado atual (possivelmente não convergido — I491).
        """
        response = self.generate(task_prompt)
        for i in range(self.max_iterations):
            feedback_prompt = (
                f"Tarefa original: {task_prompt}\n\n"
                f"Sua resposta: {response}\n\n"
                f"Identifique lacunas, ambiguidades ou erros na sua resposta. "
                f"Seja específico. Se não houver problemas, responda 'OK'."
            )
            feedback = self.generate(feedback_prompt)

            self.history.append({
                "iteration": i,
                "response": response,
                "feedback": feedback,
            })

            if feedback.strip().upper() == "OK":
                break  # Convergência declarada pelo modelo (heurística).

            revise_prompt = (
                f"Tarefa: {task_prompt}\n\n"
                f"Resposta anterior: {response}\n\n"
                f"Problemas identificados: {feedback}\n\n"
                f"Forneça uma resposta revisada."
            )
            response = self.generate(revise_prompt)

        return response