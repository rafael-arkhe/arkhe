"""
Testes honestos do IterativeRefiner (Bloco 966, v354.0).

Usa um generator FALSO scriptado (test double EXPLÍCITO, emmemória).
Não é um stub oculto no produto: aqui o texto do prompt não importa,
apenas a sequência de saídas — comportamento 100% determinístico.

Invariantes tocados:
- Ghost-1 (round-trip verificável): asserts verificam o comportamento real.
- Runtime-3: executável localmente sem serviços externos.
"""

import unittest

from orchestration.prompt_refinement import IterativeRefiner


class _ScriptedGenerator:
    """Devolve a próxima saída da lista a cada chamada. Zero rede."""

    def __init__(self, script) -> None:
        self.script = list(script)
        self.call_count = 0

    def generate(self, prompt: str) -> str:
        del prompt  # Prompt é irrelevante para o determinismo do teste.
        if self.call_count >= len(self.script):
            raise AssertionError("Mais chamadas do que o script prevê.")
        output = self.script[self.call_count]
        self.call_count += 1
        return output


class IterativeRefinerTest(unittest.TestCase):
    def test_converges_immediately_on_ok(self) -> None:
        fake = _ScriptedGenerator(["R1", "OK"])
        refiner = IterativeRefiner(generate=fake.generate, max_iterations=3)
        result = refiner.refine("Gere uma firma válida.")

        self.assertEqual(result, "R1")
        self.assertEqual(len(refiner.history), 1)
        self.assertEqual(refiner.history[0]["feedback"].strip().upper(), "OK")
        self.assertEqual(fake.call_count, 2)

    def test_revises_then_converges(self) -> None:
        fake = _ScriptedGenerator(["R1", "BUG", "R2", "OK"])
        refiner = IterativeRefiner(generate=fake.generate, max_iterations=3)
        result = refiner.refine("Gere uma firma válida.")

        self.assertEqual(result, "R2")
        self.assertEqual(len(refiner.history), 2)
        self.assertEqual(refiner.history[0]["feedback"], "BUG")
        self.assertEqual(refiner.history[1]["feedback"].strip().upper(), "OK")
        self.assertEqual(fake.call_count, 4)

    def test_exhausts_iterations_without_converging(self) -> None:
        fake = _ScriptedGenerator(["R1", "BUG", "R2", "BUG", "R3", "BUG", "R4"])
        refiner = IterativeRefiner(generate=fake.generate, max_iterations=3)
        result = refiner.refine("Gere uma firma válida.")

        # max_iterations=3 → 3 feedbacks, nenhum OK → não converge (I491).
        self.assertEqual(result, "R4")
        self.assertEqual(len(refiner.history), 3)
        self.assertEqual(fake.call_count, 7)  # 1 inicial + 3×(feedback+revisão)

    def test_zero_iterations_returns_initial(self) -> None:
        fake = _ScriptedGenerator(["R1"])
        refiner = IterativeRefiner(generate=fake.generate, max_iterations=0)
        result = refiner.refine("Nada.")

        self.assertEqual(result, "R1")
        self.assertEqual(refiner.history, [])


if __name__ == "__main__":
    unittest.main()